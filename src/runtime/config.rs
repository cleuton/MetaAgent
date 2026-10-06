//! Settings that live outside agent code (metagente.toml and environment variables).

use crate::diagnostics::Diagnostic;
use crate::lang::Params; // 0.1.2: the [parameters] section
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct LlmConfig {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub api_key_env: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub llm: LlmConfig,
    pub timeout_seconds: u64,
    pub think_max_steps: usize,
    pub bind: String,
    pub a2a_port: u16,
    /// The project folder: where `metagente.toml` was found, or where Metagente was started.
    pub root: PathBuf,
    /// Plain-language warnings, for example unknown keys.
    pub warnings: Vec<String>,
    // 0.1.2: the [parameters] section, read by agents as @parameters.name
    pub parameters: Params,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            llm: LlmConfig::default(),
            timeout_seconds: 30,
            think_max_steps: 10,
            bind: "127.0.0.1".to_string(),
            a2a_port: 8080,
            root: PathBuf::from("."),
            warnings: Vec::new(),
            parameters: Params::default(), // 0.1.2: none unless metagente.toml has [parameters]
        }
    }
}

const KNOWN_TOP: &[&str] = &["llm", "runtime", "serve", "parameters"]; // 0.1.2: parameters is a known section
const KNOWN_LLM: &[&str] = &["provider", "model", "api_key_env", "base_url"];
const KNOWN_RUNTIME: &[&str] = &["timeout_seconds", "think_max_steps"];
const KNOWN_SERVE: &[&str] = &["a2a_port", "bind"];

impl Config {
    /// Loads the configuration found by walking up from `start`, then applies environment overrides.
    pub fn load(start: &Path) -> Result<Config, Diagnostic> {
        Self::load_with_env(start, &|name| std::env::var(name).ok())
    }

    pub fn load_with_env(
        start: &Path,
        env: &dyn Fn(&str) -> Option<String>,
    ) -> Result<Config, Diagnostic> {
        let start = std::fs::canonicalize(start).unwrap_or_else(|_| start.to_path_buf());
        let mut config = Config {
            root: start.clone(),
            ..Config::default()
        };
        let mut dir = Some(start.as_path());
        while let Some(d) = dir {
            let candidate = d.join("metagente.toml");
            if candidate.is_file() {
                config.root = d.to_path_buf();
                config.read_file(&candidate)?;
                break;
            }
            dir = d.parent();
        }
        if let Some(v) = env("METAGENTE_LLM_PROVIDER") {
            config.llm.provider = Some(v);
        }
        if let Some(v) = env("METAGENTE_LLM_MODEL") {
            config.llm.model = Some(v);
        }
        if let Some(v) = env("METAGENTE_TIMEOUT_SECONDS") {
            config.timeout_seconds = v.trim().parse().map_err(|_| {
                Diagnostic::new(format!(
                    "METAGENTE_TIMEOUT_SECONDS is `{}`, which is not a whole number",
                    v
                ))
                .fix("set it to a number of seconds, for example 30")
            })?;
        }
        if let Some(v) = env("METAGENTE_THINK_MAX_STEPS") {
            config.think_max_steps = v.trim().parse().map_err(|_| {
                Diagnostic::new(format!(
                    "METAGENTE_THINK_MAX_STEPS is `{}`, which is not a whole number",
                    v
                ))
                .fix("set it to a number, for example 10")
            })?;
        }
        Ok(config)
    }

    /// 0.1.2: only the parameters, found like the rest of the settings. Environment variables play no part.
    pub fn load_parameters(start: &Path) -> Result<Params, Diagnostic> {
        Ok(Self::load_with_env(start, &|_| None)?.parameters)
    }

    fn read_file(&mut self, path: &Path) -> Result<(), Diagnostic> {
        let text = std::fs::read_to_string(path).map_err(|e| {
            Diagnostic::new(format!("I could not read {}: {}", path.display(), e))
                .fix("check that the file exists and you are allowed to read it")
        })?;
        let value: toml::Table = text.parse().map_err(|e: toml::de::Error| {
            let mut d =
                Diagnostic::new(format!("{} is not valid: {}", path.display(), e.message()))
                    .fix("check the quotes and the [section] names in metagente.toml");
            if let Some(span) = e.span() {
                let line = text[..span.start.min(text.len())].matches('\n').count() + 1;
                d = d
                    .at(&path.display().to_string(), line, 1)
                    .with_source(text.clone().into());
            }
            d
        })?;
        self.check_keys(&value);
        let get_str = |table: &toml::Table, key: &str| {
            table
                .get(key)
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from)
        };
        if let Some(llm) = value.get("llm").and_then(|v| v.as_table()) {
            self.llm.provider = get_str(llm, "provider");
            self.llm.model = get_str(llm, "model");
            self.llm.api_key_env = get_str(llm, "api_key_env");
            self.llm.base_url = get_str(llm, "base_url");
        }
        if let Some(rt) = value.get("runtime").and_then(|v| v.as_table()) {
            if let Some(n) = rt.get("timeout_seconds").and_then(|v| v.as_integer()) {
                self.timeout_seconds = n.max(1) as u64;
            }
            if let Some(n) = rt.get("think_max_steps").and_then(|v| v.as_integer()) {
                self.think_max_steps = n.max(1) as usize;
            }
        }
        // 0.1.2: every entry of [parameters] is a parameter; only text can be used by agents
        if let Some(table) = value.get("parameters").and_then(|v| v.as_table()) {
            for (name, v) in table {
                match v.as_str() {
                    Some(text) => {
                        self.parameters
                            .values
                            .insert(name.clone(), text.to_string());
                    }
                    None => {
                        self.parameters.not_text.insert(name.clone());
                    }
                }
            }
        }
        if let Some(sv) = value.get("serve").and_then(|v| v.as_table()) {
            if let Some(n) = sv.get("a2a_port").and_then(|v| v.as_integer()) {
                self.a2a_port = n.clamp(1, 65535) as u16;
            }
            if let Some(b) = get_str(sv, "bind") {
                self.bind = b;
            }
        }
        Ok(())
    }

    fn check_keys(&mut self, value: &toml::Table) {
        for (key, v) in value {
            if !KNOWN_TOP.contains(&key.as_str()) {
                self.warnings.push(format!(
                    "metagente.toml has an unknown section `[{}]`; I ignored it",
                    key
                ));
                continue;
            }
            // 0.1.2: parameters have names of their own, so none of them is "unknown"
            if key == "parameters" {
                continue;
            }
            let known = match key.as_str() {
                "llm" => KNOWN_LLM,
                "runtime" => KNOWN_RUNTIME,
                _ => KNOWN_SERVE,
            };
            if let Some(table) = v.as_table() {
                for inner in table.keys() {
                    if !known.contains(&inner.as_str()) {
                        self.warnings.push(format!(
                            "metagente.toml has an unknown setting `{}` in [{}]; I ignored it",
                            inner, key
                        ));
                    }
                }
            }
        }
    }
}
