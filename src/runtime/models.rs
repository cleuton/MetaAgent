//! 0.1.5: named language models, `[llm.NAME]` in metagente.toml. Reading and checking them, and the plain messages about
//! them. A named model is Anthropic or OpenAI at its official address (never another address); its model name and its
//! key come from environment variables. Nothing is printed from metagente.toml except a SAFE NAME (D10), because a
//! person may paste a key where a name belongs.

use crate::diagnostics::Diagnostic;
use std::collections::BTreeMap;
use std::path::Path;

/// The services a named model can be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Anthropic,
    OpenAi,
}

impl ProviderKind {
    pub fn name(self) -> &'static str {
        match self {
            ProviderKind::Anthropic => "anthropic",
            ProviderKind::OpenAi => "openai",
        }
    }

    /// The usual key variable of the service.
    pub fn usual_key_env(self) -> &'static str {
        match self {
            ProviderKind::Anthropic => "ANTHROPIC_API_KEY",
            ProviderKind::OpenAi => "OPENAI_API_KEY",
        }
    }
}

/// Where the model name comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelName {
    /// `model = "..."` (or the usual model of Anthropic).
    Literal(String),
    /// `model_env = "NAME"`: the name of a variable that holds the model name, read at the first use.
    FromEnv(String),
}

/// One `[llm.NAME]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedModel {
    pub nickname: String,
    pub provider: ProviderKind,
    pub model: ModelName,
    /// The variable that holds the key: `api_key_env`, or the usual one of the provider.
    pub key_env: String,
    /// True when `api_key_env` was written (so it is a name this project chose, to be hidden from `tool env`).
    pub key_env_written: bool,
    /// The line of the `[llm.NAME]` header, for messages.
    pub line: usize,
    /// The lines of `api_key_env` and `model_env` (0 when not written), for a message that cannot show a long name.
    pub key_env_line: usize,
    pub model_env_line: usize,
}

/// Every named model of a project, ordered by nickname. Empty when `[llm]` has no sub-table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelSet {
    models: BTreeMap<String, NamedModel>,
}

impl ModelSet {
    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }

    pub fn len(&self) -> usize {
        self.models.len()
    }

    pub fn get(&self, nickname: &str) -> Option<&NamedModel> {
        self.models.get(nickname)
    }

    pub fn names(&self) -> Vec<&str> {
        self.models.keys().map(String::as_str).collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = &NamedModel> {
        self.models.values()
    }

    /// The key variables a person named with `api_key_env`; agents may never read them (FR-014).
    pub fn written_key_envs(&self) -> Vec<String> {
        self.models
            .values()
            .filter(|m| m.key_env_written)
            .map(|m| m.key_env.clone())
            .collect()
    }
}

impl ModelSet {
    /// The warning of D9: sub-tables without `[version]`. It names the first model (its nickname is always safe).
    pub fn no_version_warning(&self) -> String {
        let first = self
            .models
            .keys()
            .next()
            .map(String::as_str)
            .unwrap_or("NAME");
        format!(
            "[llm.{}] is set but [version] is not. Older programs do not know it. Add a [version] section with minimum = \"0.1.5\".",
            first
        )
    }
}

// ---------- the rule of what a message may print (D10) ----------

/// A SAFE NAME: 1 to 24 ASCII letters, digits, `_` or `-`. Only these are ever shown from metagente.toml or from the
/// `using` word of an `.ag` file, so a key pasted where a name belongs can never be printed.
pub fn shown(name: &str) -> Option<&str> {
    let ok = !name.is_empty()
        && name.chars().count() <= 24
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    ok.then_some(name)
}

/// 1 to 64 ASCII letters, digits or `_`, not starting with a digit: the name of an environment variable.
pub fn valid_variable_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.chars().next().is_some_and(|c| c.is_ascii_digit())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// An ASCII letter, then letters, digits, `_` or `-`; at most 24 characters.
pub fn valid_nickname(name: &str) -> bool {
    shown(name).is_some() && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
}

/// The name of a variable as a message may show it: itself when it is safe, or "the variable named on line N".
pub fn variable_phrase(name: &str, line: usize) -> String {
    match shown(name) {
        Some(n) => format!("the variable {}", n),
        None => format!("the variable named on line {}", line),
    }
}

// ---------- reading ----------

const SETTINGS: &[&str] = &["provider", "model", "model_env", "api_key_env"];
const KEY_LIKE: &[&str] = &["key", "api_key", "apikey", "token", "secret", "password"];
const SETTINGS_OF_LLM: &[&str] = &["provider", "model", "api_key_env", "base_url"];

/// The place is the file and the line, never the text of the line: a person may have pasted a key there (D10), and a
/// diagnostic that shows its source line would print it.
fn problem(path: &Path, _text: &str, line: usize, message: String, fix: String) -> Diagnostic {
    let mut d = Diagnostic::new(message).fix(fix);
    if line > 0 {
        d = d.at(&path.display().to_string(), line, 1);
    }
    d
}

/// The line of the header `[llm.NAME]` (1 when it cannot be found, for example with dotted keys).
fn header_line(text: &str, nickname: &str) -> usize {
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        if let Some(inner) = t.strip_prefix("[llm.").and_then(|r| r.strip_suffix(']')) {
            let inner = inner.trim();
            if inner == nickname || inner.trim_matches('"') == nickname {
                return i + 1;
            }
        }
    }
    text.lines()
        .position(|l| l.trim() == "[llm]")
        .map(|i| i + 1)
        .unwrap_or(1)
}

/// The line of `key` inside `[llm.NAME]`, or the header line.
fn setting_line(text: &str, nickname: &str, key: &str, header: usize) -> usize {
    let n = super::config::line_of(text, &format!("llm.{}", nickname), key);
    if n > 0 {
        return n;
    }
    // a table inside the sub-table has its own header: [llm.NAME.key]
    let wanted = format!("[llm.{}.{}]", nickname, key);
    text.lines()
        .position(|l| l.trim() == wanted)
        .map(|i| i + 1)
        .unwrap_or(header)
}

fn label(nickname: &str) -> String {
    format!("[llm.{}]", nickname)
}

/// Reads every sub-table of `[llm]` (a key of `[llm]` whose value is a table). Every mistake is an error with the
/// line (D11). Nothing is printed from the file except a safe name (D10).
pub fn read_models(llm: &toml::Table, text: &str, path: &Path) -> Result<ModelSet, Diagnostic> {
    let mut set = ModelSet::default();
    for (nickname, value) in llm {
        let Some(table) = value.as_table() else {
            continue; // a scalar of [llm]: the 0.1.4 rules (a note for an unknown one)
        };
        let header = header_line(text, nickname);
        let model = read_one(nickname, table, text, path, header)?;
        set.models.insert(nickname.clone(), model);
    }
    Ok(set)
}

fn read_one(
    nickname: &str,
    table: &toml::Table,
    text: &str,
    path: &Path,
    header: usize,
) -> Result<NamedModel, Diagnostic> {
    // 1. the nickname
    if !valid_nickname(nickname) {
        let message = match shown(nickname) {
            Some(n) => format!(
                "[llm.{}] is not a name I can use for a model: it must start with a letter and have at most 24 letters, digits, _ or -.",
                n
            ),
            None => format!(
                "[llm] has a name that is too long or has unusual characters (line {}).",
                header
            ),
        };
        return Err(problem(
            path,
            text,
            header,
            message,
            "use a short name made of letters, digits, _ and -, such as fast".to_string(),
        ));
    }
    if SETTINGS_OF_LLM.contains(&nickname) {
        return Err(problem(
            path,
            text,
            header,
            format!(
                "[llm.{}] cannot be a model name, because {} is a setting of [llm].",
                nickname, nickname
            ),
            "choose another name, such as fast".to_string(),
        ));
    }
    if crate::lang::RESERVED_WORDS.contains(&nickname) {
        return Err(problem(
            path,
            text,
            header,
            format!(
                "[llm.{}] cannot be a model name, because {} is a word of the language.",
                nickname, nickname
            ),
            "choose another name, such as fast".to_string(),
        ));
    }
    let here = label(nickname);

    // 2. the settings, in the order of the file
    let mut keys: Vec<(&String, usize)> = table
        .keys()
        .map(|k| (k, setting_line(text, nickname, k, header)))
        .collect();
    keys.sort_by_key(|(_, line)| *line);
    for (key, line) in &keys {
        let value = &table[*key];
        if value.is_table() || !(SETTINGS.contains(&key.as_str())) {
            let key_like = KEY_LIKE.contains(&key.as_str());
            let address = key.as_str() == "base_url" || key.as_str() == "base_url_env";
            if !value.is_table() && key_like {
                return Err(problem(
                    path,
                    text,
                    *line,
                    format!(
                        "{} has a key written in the file. Keys are never kept here.",
                        here
                    ),
                    "put the key in an environment variable and name it in api_key_env".to_string(),
                ));
            }
            if !value.is_table() && address {
                return Err(problem(
                    path,
                    text,
                    *line,
                    format!(
                        "{} has {}. A named model always uses the official address.",
                        here, key
                    ),
                    "for another address, use the [llm] section".to_string(),
                ));
            }
            let message = match shown(key) {
                Some(k) => format!("{} has the setting {}, which I do not know.", here, k),
                None => format!(
                    "{} has a setting with a name that is too long or has unusual characters (line {}).",
                    here, line
                ),
            };
            let fix = match shown(key)
                .and_then(|k| crate::tools::closest_name(k, SETTINGS.iter().copied()))
            {
                Some(near) => format!("did you mean {}?", near),
                None => {
                    "the settings of a named model are provider, model, model_env and api_key_env"
                        .to_string()
                }
            };
            return Err(problem(path, text, *line, message, fix));
        }
    }

    // 3. the values: text that is not empty
    let text_of = |key: &str| -> Result<Option<(String, usize)>, Diagnostic> {
        let Some(v) = table.get(key) else {
            return Ok(None);
        };
        let line = setting_line(text, nickname, key, header);
        match v.as_str().filter(|s| !s.trim().is_empty()) {
            Some(s) => Ok(Some((s.to_string(), line))),
            None => Err(problem(
                path,
                text,
                line,
                format!("{} setting {} must be text that is not empty.", here, key),
                format!("write it between quotes, like {} = \"...\"", key),
            )),
        }
    };
    let provider = text_of("provider")?;
    let model = text_of("model")?;
    let model_env = text_of("model_env")?;
    let api_key_env = text_of("api_key_env")?;

    // 4. the provider
    let Some((provider_text, provider_line)) = provider else {
        return Err(problem(
            path,
            text,
            header,
            format!("The provider is missing in {}.", here),
            "add provider = \"anthropic\" or \"openai\"".to_string(),
        ));
    };
    let kind = match provider_text.as_str() {
        "anthropic" => ProviderKind::Anthropic,
        "openai" => ProviderKind::OpenAi,
        other => {
            let message = match shown(other) {
                Some(p) => format!(
                    "The provider {} in {} is not one a named model accepts. Use anthropic or openai.",
                    p, here
                ),
                None => format!(
                    "The provider on line {} of {} is not one a named model accepts. Use anthropic or openai.",
                    provider_line, here
                ),
            };
            let fix = if other == "openai-compatible" {
                "a named model accepts anthropic or openai; for another address use the [llm] section"
                    .to_string()
            } else {
                match shown(other)
                    .and_then(|p| crate::tools::closest_name(p, ["anthropic", "openai"]))
                {
                    Some(near) => format!("did you mean {}?", near),
                    None => "use provider = \"anthropic\" or \"openai\"".to_string(),
                }
            };
            return Err(problem(path, text, provider_line, message, fix));
        }
    };

    // 5. the model name
    if let (Some(_), Some((_, line))) = (&model, &model_env) {
        return Err(problem(
            path,
            text,
            *line,
            format!("{} has both model and model_env.", here),
            "give only one".to_string(),
        ));
    }
    let model_env_line = model_env.as_ref().map(|(_, l)| *l).unwrap_or(0);
    let model_name = match (model, model_env) {
        (Some((m, _)), None) => ModelName::Literal(m),
        (None, Some((v, line))) => {
            if !valid_variable_name(&v) {
                return Err(wrong_variable_name(
                    path,
                    text,
                    line,
                    "model_env",
                    &here,
                    "FAST_MODEL",
                ));
            }
            ModelName::FromEnv(v)
        }
        (None, None) => match kind {
            ProviderKind::Anthropic => ModelName::Literal("claude-sonnet-5-5".to_string()),
            ProviderKind::OpenAi => {
                return Err(problem(
                    path,
                    text,
                    header,
                    format!("The provider openai needs a model name in {}.", here),
                    "add model = \"...\" (or model_env = \"NAME_OF_A_VARIABLE\")".to_string(),
                ));
            }
        },
        (Some(_), Some(_)) => unreachable!("handled above"),
    };

    // 6. the key variable
    let key_env_line = api_key_env.as_ref().map(|(_, l)| *l).unwrap_or(0);
    let (key_env, key_env_written) = match api_key_env {
        Some((v, line)) => {
            if !valid_variable_name(&v) {
                return Err(wrong_variable_name(
                    path,
                    text,
                    line,
                    "api_key_env",
                    &here,
                    "FAST_KEY",
                ));
            }
            (v, true)
        }
        None => (kind.usual_key_env().to_string(), false),
    };

    Ok(NamedModel {
        nickname: nickname.to_string(),
        provider: kind,
        model: model_name,
        key_env,
        key_env_written,
        line: header,
        key_env_line,
        model_env_line,
    })
}

fn wrong_variable_name(
    path: &Path,
    text: &str,
    line: usize,
    setting: &str,
    here: &str,
    example: &str,
) -> Diagnostic {
    problem(
        path,
        text,
        line,
        format!(
            "{} in {} must be the name of a variable, such as {}.",
            setting, here, example
        ),
        "write only the name of the variable; the key itself stays in the environment".to_string(),
    )
}

// ---------- checking `using NAME` against the models (FR-010) ----------

/// Every `using NAME` of the agents must name a model of `[llm.NAME]`. The message lists the models that exist and the
/// closest name; the nickname written in the `.ag` file is shown only when it is a safe name (D10).
pub fn check_uses(
    agents: &[std::sync::Arc<crate::lang::AgentDef>],
    models: &ModelSet,
) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    for agent in agents {
        for (name, span) in crate::lang::check::thinks_using(agent) {
            if models.get(&name).is_some() {
                continue;
            }
            let (message, fix) = if models.is_empty() {
                match shown(&name) {
                    Some(n) => (
                        format!("There is no model called {}. No model is defined under [llm].", n),
                        format!(
                            "add [llm.{}] with provider = \"anthropic\" or \"openai\"",
                            n
                        ),
                    ),
                    None => (
                        "There is no model with the name written after using. No model is defined under [llm]."
                            .to_string(),
                        "add [llm.fast] with provider = \"anthropic\" or \"openai\", and write using fast"
                            .to_string(),
                    ),
                }
            } else {
                let list = models.names().join(", ");
                let near = shown(&name).and_then(|n| crate::tools::closest_name(n, models.names()));
                let fix = match near {
                    Some(n) => format!("did you mean {}?", n),
                    None => format!("use one of: {}", list),
                };
                let message = match shown(&name) {
                    Some(n) => format!("There is no model called {}. The models are: {}.", n, list),
                    None => format!(
                        "There is no model with the name written after using. The models are: {}.",
                        list
                    ),
                };
                (message, fix)
            };
            problems.push(Diagnostic::new(message).fix(fix).located(
                &agent.source.name,
                span.line,
                span.col,
                &agent.source.text,
            ));
        }
    }
    problems
}

/// The nicknames that `think ... using NAME` uses in these agents that are models of the project, without repeats.
pub fn used_models(
    agents: &[std::sync::Arc<crate::lang::AgentDef>],
    models: &ModelSet,
) -> Vec<String> {
    let mut used: Vec<String> = Vec::new();
    for agent in agents {
        for (name, _) in crate::lang::check::thinks_using(agent) {
            if models.get(&name).is_some() && !used.contains(&name) {
                used.push(name);
            }
        }
    }
    used.sort();
    used
}

/// `metagente check` only: a warning for each model that the project uses whose key variable or `model_env` variable
/// is unset or empty here. Never for `[llm]`, for a model nothing uses, or for a project without sub-tables (D11, F15).
pub fn unset_warnings(
    used: &[String],
    models: &ModelSet,
    env: &dyn Fn(&str) -> Option<String>,
) -> Vec<String> {
    let missing = |name: &str| env(name).map(|v| v.trim().is_empty()).unwrap_or(true);
    let mut out = Vec::new();
    for nickname in used {
        let Some(m) = models.get(nickname) else {
            continue;
        };
        if missing(&m.key_env) {
            out.push(format!(
                "the model {} needs {}, and it is not set here.",
                m.nickname,
                variable_phrase(&m.key_env, m.key_env_line.max(m.line))
            ));
        }
        if let ModelName::FromEnv(var) = &m.model {
            if missing(var) {
                out.push(format!(
                    "the model {} needs {}, and it is empty or not set here.",
                    m.nickname,
                    variable_phrase(var, m.model_env_line.max(m.line))
                ));
            }
        }
    }
    out
}
