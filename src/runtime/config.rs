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

/// 0.1.3: the `[network]` section. Only names of environment variables are kept here, never their values.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NetworkConfig {
    pub allow_self_signed: bool,
    pub llm_allow_self_signed: bool,
    pub self_signed_hosts: Vec<String>,
    pub ca_file: Option<PathBuf>,
    /// The line of `ca_file` in metagente.toml, for messages.
    pub ca_file_line: usize,
    pub proxy: ProxyConfig,
    /// The metagente.toml these settings came from, for messages ("" when there is none).
    pub file: String,
}

/// 0.1.3: the `[network.proxy]` section.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProxyConfig {
    pub url: Option<String>,
    pub username_env: Option<String>,
    pub password_env: Option<String>,
    pub no_proxy: Vec<String>,
    pub pass_to_tools: bool,
    pub url_line: usize,
    pub username_env_line: usize,
    pub password_env_line: usize,
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
    // 0.1.3: the [network] section
    pub network: NetworkConfig,
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
            network: NetworkConfig::default(), // 0.1.3: certificate checks on, no proxy unless the environment has one
        }
    }
}

const KNOWN_TOP: &[&str] = &["llm", "runtime", "serve", "parameters", "network"]; // 0.1.2: parameters, 0.1.3: network
const KNOWN_NETWORK: &[&str] = &[
    "allow_self_signed",
    "llm_allow_self_signed",
    "self_signed_hosts",
    "ca_file",
    "proxy",
]; // 0.1.3
const KNOWN_PROXY: &[&str] = &[
    "url",
    "username_env",
    "password_env",
    "no_proxy",
    "pass_to_tools",
]; // 0.1.3
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

    /// 0.1.3: for `metagente check`. A metagente.toml that is not valid TOML at all is ignored by `check`, as in 0.1.2;
    /// a problem in `[network]` is an error.
    pub fn network_for_check(start: &Path) -> Result<NetworkConfig, Diagnostic> {
        let start = std::fs::canonicalize(start).unwrap_or_else(|_| start.to_path_buf());
        let mut dir = Some(start.as_path());
        while let Some(d) = dir {
            let candidate = d.join("metagente.toml");
            if candidate.is_file() {
                let Ok(text) = std::fs::read_to_string(&candidate) else {
                    return Ok(NetworkConfig::default());
                };
                let Ok(table) = text.parse::<toml::Table>() else {
                    return Ok(NetworkConfig::default());
                };
                return match table.get("network") {
                    Some(net) => read_network(net, &text, &candidate),
                    None => Ok(NetworkConfig::default()),
                };
            }
            dir = d.parent();
        }
        Ok(NetworkConfig::default())
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
        // 0.1.3: certificates and proxies
        if let Some(net) = value.get("network") {
            self.network = read_network(net, &text, path)?;
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
            // 0.1.3: [network] checks its own keys (and [network.proxy]) while it is read
            if key == "network" {
                if let Some(table) = v.as_table() {
                    for inner in table.keys() {
                        if !KNOWN_NETWORK.contains(&inner.as_str()) {
                            self.warnings.push(format!(
                                "metagente.toml has an unknown setting `{}` in [network]; I ignored it",
                                inner
                            ));
                        }
                    }
                    if let Some(proxy) = table.get("proxy").and_then(|p| p.as_table()) {
                        for inner in proxy.keys() {
                            // literal logins are an error, reported by read_network
                            if !KNOWN_PROXY.contains(&inner.as_str())
                                && inner != "username"
                                && inner != "password"
                            {
                                self.warnings.push(format!(
                                    "metagente.toml has an unknown setting `{}` in [network.proxy]; I ignored it",
                                    inner
                                ));
                            }
                        }
                    }
                }
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

// ---------- 0.1.3: [network] ----------

/// The line (1-based) of `key` inside `[section]` of metagente.toml, or 0 when it cannot be found.
fn line_of(text: &str, section: &str, key: &str) -> usize {
    let mut current = String::new();
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            current = t.trim_matches(|c| c == '[' || c == ']').trim().to_string();
            continue;
        }
        if current == section {
            let name = t.split('=').next().unwrap_or("").trim();
            if name == key && t.contains('=') {
                return i + 1;
            }
        }
    }
    0
}

fn network_problem(path: &Path, line: usize, message: String, fix: &str) -> Diagnostic {
    let mut d = Diagnostic::new(message).fix(fix);
    if line > 0 {
        d = d.at(&path.display().to_string(), line, 1);
    }
    d
}

fn want_bool(
    table: &toml::Table,
    key: &str,
    section: &str,
    text: &str,
    path: &Path,
) -> Result<bool, Diagnostic> {
    match table.get(key) {
        None => Ok(false),
        Some(toml::Value::Boolean(b)) => Ok(*b),
        Some(_) => Err(network_problem(
            path,
            line_of(text, section, key),
            format!("`{}` in [{}] must be true or false", key, section),
            &format!("write {} = true or {} = false", key, key),
        )),
    }
}

fn want_text(
    table: &toml::Table,
    key: &str,
    section: &str,
    text: &str,
    path: &Path,
) -> Result<Option<String>, Diagnostic> {
    match table.get(key) {
        None => Ok(None),
        Some(toml::Value::String(s)) if !s.trim().is_empty() => Ok(Some(s.trim().to_string())),
        Some(toml::Value::String(_)) => Ok(None),
        Some(_) => Err(network_problem(
            path,
            line_of(text, section, key),
            format!("`{}` in [{}] must be text in quotes", key, section),
            &format!("write it like {} = \"...\"", key),
        )),
    }
}

fn want_list(
    table: &toml::Table,
    key: &str,
    section: &str,
    text: &str,
    path: &Path,
) -> Result<Vec<String>, Diagnostic> {
    match table.get(key) {
        None => Ok(Vec::new()),
        Some(toml::Value::Array(items)) => {
            let mut out = Vec::new();
            for item in items {
                match item.as_str() {
                    Some(s) => out.push(s.trim().to_string()),
                    None => {
                        return Err(network_problem(
                            path,
                            line_of(text, section, key),
                            format!("`{}` in [{}] must be a list of texts", key, section),
                            &format!("write it like {} = [\"a\", \"b\"]", key),
                        ));
                    }
                }
            }
            Ok(out)
        }
        Some(_) => Err(network_problem(
            path,
            line_of(text, section, key),
            format!("`{}` in [{}] must be a list of texts", key, section),
            &format!("write it like {} = [\"a\", \"b\"]", key),
        )),
    }
}

const LOGIN_FIX: &str = "write the NAME of an environment variable in username_env and password_env, for example username_env = \"PROXY_USER\", and put the value in that variable";

fn read_network(value: &toml::Value, text: &str, path: &Path) -> Result<NetworkConfig, Diagnostic> {
    let Some(table) = value.as_table() else {
        return Err(network_problem(
            path,
            line_of(text, "", "network"),
            "`network` must be a section, written [network]".to_string(),
            "write [network] on a line of its own, then the settings under it",
        ));
    };
    let mut net = NetworkConfig {
        file: path.display().to_string(),
        ..NetworkConfig::default()
    };
    net.allow_self_signed = want_bool(table, "allow_self_signed", "network", text, path)?;
    net.llm_allow_self_signed = want_bool(table, "llm_allow_self_signed", "network", text, path)?;
    net.self_signed_hosts = want_list(table, "self_signed_hosts", "network", text, path)?;
    if let Some(file) = want_text(table, "ca_file", "network", text, path)? {
        let line = line_of(text, "network", "ca_file");
        let given = PathBuf::from(&file);
        let full = if given.is_absolute() {
            given
        } else {
            path.parent().unwrap_or_else(|| Path::new(".")).join(given)
        };
        if let Err(e) = std::fs::read(&full) {
            return Err(network_problem(
                path,
                line,
                format!(
                    "I could not read the certificate file {}: {}",
                    full.display(),
                    e
                ),
                "check the path of ca_file (a relative path starts at the folder of metagente.toml)",
            ));
        }
        net.ca_file = Some(full);
        net.ca_file_line = line;
    }
    let Some(proxy_value) = table.get("proxy") else {
        return Ok(net);
    };
    let Some(proxy) = proxy_value.as_table() else {
        return Err(network_problem(
            path,
            line_of(text, "network", "proxy"),
            "`proxy` must be a section, written [network.proxy]".to_string(),
            "write [network.proxy] on a line of its own, then the settings under it",
        ));
    };
    for literal in ["username", "password"] {
        if proxy.contains_key(literal) {
            return Err(network_problem(
                path,
                line_of(text, "network.proxy", literal),
                format!(
                    "`{}` in [network.proxy] must not hold a real login: metagente.toml may be shared or committed",
                    literal
                ),
                LOGIN_FIX,
            ));
        }
    }
    let pr = &mut net.proxy;
    pr.url = want_text(proxy, "url", "network.proxy", text, path)?;
    pr.url_line = line_of(text, "network.proxy", "url");
    if let Some(url) = &pr.url {
        let authority = url.split("://").nth(1).unwrap_or(url);
        let authority = authority.split('/').next().unwrap_or("");
        if authority.contains('@') {
            return Err(network_problem(
                path,
                pr.url_line,
                "the proxy `url` must not contain a user name or password".to_string(),
                LOGIN_FIX,
            ));
        }
        match reqwest::Url::parse(url) {
            Ok(u) if (u.scheme() == "http" || u.scheme() == "https") && u.host_str().is_some() => {}
            _ => {
                return Err(network_problem(
                    path,
                    pr.url_line,
                    format!("`{}` is not a proxy address I can use", url),
                    "write it like url = \"http://proxy.company.com:3128\"",
                ));
            }
        }
    }
    pr.username_env = want_text(proxy, "username_env", "network.proxy", text, path)?;
    pr.password_env = want_text(proxy, "password_env", "network.proxy", text, path)?;
    pr.username_env_line = line_of(text, "network.proxy", "username_env");
    pr.password_env_line = line_of(text, "network.proxy", "password_env");
    match (&pr.username_env, &pr.password_env) {
        (Some(_), None) => {
            return Err(network_problem(
                path,
                pr.username_env_line,
                "[network.proxy] has username_env but no password_env".to_string(),
                "add password_env with the NAME of the variable that holds the password",
            ));
        }
        (None, Some(_)) => {
            return Err(network_problem(
                path,
                pr.password_env_line,
                "[network.proxy] has password_env but no username_env".to_string(),
                "add username_env with the NAME of the variable that holds the user name",
            ));
        }
        _ => {}
    }
    pr.no_proxy = want_list(proxy, "no_proxy", "network.proxy", text, path)?;
    pr.pass_to_tools = want_bool(proxy, "pass_to_tools", "network.proxy", text, path)?;
    Ok(net)
}
