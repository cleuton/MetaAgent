//! `metagente new`: a starter agent and a starter configuration.

use crate::diagnostics::{Diagnostic, MgResult};
use std::path::Path;

const CONFIG: &str = r#"# Settings for Metagente. Agents never mention these, so they stay simple.

# Uncomment this section to let agents use `think` (a language model).
# [llm]
# provider = "anthropic"               # or "openai-compatible"
# model = "claude-sonnet-5-5"          # the model name your provider gives you
# api_key_env = "ANTHROPIC_API_KEY"    # the NAME of the variable that holds your key

[runtime]
timeout_seconds = 30      # how long a tool call may take
think_max_steps = 10      # how many steps `think` may take

[serve]
a2a_port = 8080
bind = "127.0.0.1"        # only this computer; use --public to open up
"#;

fn agent_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    let mut chars = cleaned.chars();
    match chars.next() {
        Some(first) if first.is_alphabetic() => {
            first.to_uppercase().collect::<String>() + chars.as_str()
        }
        _ => "Helper".to_string(),
    }
}

/// Creates `<name>.ag` and, when missing, `metagente.toml` in `dir`. Returns the messages to show.
pub fn create(dir: &Path, name: &str) -> MgResult<Vec<String>> {
    let agent = agent_name(name);
    let file_name = format!("{}.ag", agent.to_lowercase());
    let target = dir.join(&file_name);
    if target.exists() {
        return Err(Diagnostic::new(format!("{} already exists", file_name))
            .fix("choose another name, or delete the old file first"));
    }
    let source = format!(
        "agent {agent}\n  goal \"Say hello to someone\"\n  accepts greet name\n  on greet\n    reply \"Hello, {{name}}!\"\n"
    );
    std::fs::write(&target, source).map_err(|e| {
        Diagnostic::new(format!("I could not create {}: {}", file_name, e))
            .fix("check that you may write in this folder")
    })?;
    let mut messages = vec![format!("Created {}", file_name)];
    let config = dir.join("metagente.toml");
    if !config.exists() {
        std::fs::write(&config, CONFIG).map_err(|e| {
            Diagnostic::new(format!("I could not create metagente.toml: {}", e))
                .fix("check that you may write in this folder")
        })?;
        messages.push("Created metagente.toml".to_string());
    }
    messages.push(format!(
        "Try it: metagente run {} greet name=World",
        file_name
    ));
    Ok(messages)
}
