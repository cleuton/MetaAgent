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

# 0.1.2: texts that agents read as @parameters.name, so you change them here and not in the .ag files.
# Uncomment and use them in an agent, for example: reply think @parameters.prompt1
# [parameters]
# prompt1 = "You are a helpful assistant."
# a2a_leitor = "http://127.0.0.1:8080"

# 0.1.3: secure connections. https:// addresses just work; these settings are for special cases.
# [network]
# allow_self_signed = false            # true skips certificate checks for agents, tools and web addresses (development only)
# llm_allow_self_signed = false        # true skips certificate checks for the language model only (a private gateway you trust)
# self_signed_hosts = ["localhost", "127.0.0.1"]   # optional: limit allow_self_signed to these hosts
# ca_file = "certs/dev-ca.pem"         # trust one more authority and keep checking (better than skipping checks)
#
# [network.proxy]                      # HTTPS_PROXY, HTTP_PROXY and NO_PROXY are used when there is no url here
# url = "http://proxy.company.com:3128"   # no user name or password inside the address
# username_env = "PROXY_USER"          # the NAME of the variable that holds the user name
# password_env = "PROXY_PASSWORD"      # the NAME of the variable that holds the password
# no_proxy = ["localhost", "127.0.0.1", ".internal.company.com"]
# pass_to_tools = false                # true gives these settings to tool programs started over stdio
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
