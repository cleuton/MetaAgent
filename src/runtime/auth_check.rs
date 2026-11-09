//! 0.1.4: checks `[auth]` against the agents of the project: names in `protect` and `forward` that match nothing, and
//! `forward` names that can never receive a token (D17, FR-012, FR-014).

use super::auth::AuthPolicy;
use crate::diagnostics::Diagnostic;
use crate::lang::{AgentDef, Params, ToolKind};
use crate::tools::closest_name;
use std::path::Path;

#[derive(Default)]
pub struct Report {
    pub errors: Vec<Diagnostic>,
    pub warnings: Vec<String>,
    pub notes: Vec<String>,
}

/// Every agent declared in a `.ag` file in the folder of metagente.toml or below it. A file that does not parse is
/// skipped here (the normal checks report it).
fn project_agents(root: &Path, params: &Params) -> Vec<AgentDef> {
    fn walk(dir: &Path, depth: usize, params: &Params, out: &mut Vec<AgentDef>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if path.is_dir() {
                if depth < 6 && !name.starts_with('.') && name != "target" && name != "node_modules"
                {
                    walk(&path, depth + 1, params, out);
                }
            } else if name.ends_with(".ag") {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if let Ok(defs) =
                        crate::lang::parse_file_with(&name, Some(path.clone()), &text, params)
                    {
                        out.extend(defs);
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(root, 0, params, &mut out);
    out
}

fn located(d: Diagnostic, policy: &AuthPolicy, line: usize) -> Diagnostic {
    if line > 0 {
        d.at(&policy.file, line, 1)
    } else {
        d
    }
}

/// True when a program named by the validator command can be found.
pub fn validator_program_exists(policy: &AuthPolicy) -> Option<bool> {
    let command = policy.validator.as_ref()?;
    let words = crate::mcp::client::split_command(command);
    let first = words.first()?;
    if first.contains('/') || first.contains('\\') {
        let p = Path::new(first);
        let full = if p.is_absolute() {
            p.to_path_buf()
        } else {
            policy.base_dir.join(p)
        };
        return Some(full.exists());
    }
    let path = std::env::var_os("PATH")?;
    let extensions: Vec<&str> = if cfg!(windows) {
        vec!["", ".exe", ".cmd", ".bat"]
    } else {
        vec![""]
    };
    Some(std::env::split_paths(&path).any(|dir| {
        extensions
            .iter()
            .any(|ext| dir.join(format!("{}{}", first, ext)).is_file())
    }))
}

pub fn validate(policy: &AuthPolicy, params: &Params) -> Report {
    let agents = project_agents(&policy.base_dir, params);
    validate_with(policy, &agents)
}

pub fn validate_with(policy: &AuthPolicy, agents: &[AgentDef]) -> Report {
    let mut report = Report::default();
    let agent_names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
    // protect: every entry must match something declared
    for entry in &policy.protect {
        let owners: Vec<&AgentDef> = agents.iter().filter(|a| a.name == entry.agent).collect();
        if owners.is_empty() {
            let fix = match closest_name(&entry.agent, agent_names.iter().copied()) {
                Some(s) => format!(
                    "did you mean `{}.{}`?",
                    s,
                    entry.message.as_deref().unwrap_or("*")
                ),
                None => "check the agent name in your .ag files".to_string(),
            };
            report.errors.push(located(
                Diagnostic::new(format!(
                    "[auth] protect lists {}, but no such message is declared.",
                    entry.text
                ))
                .fix(fix),
                policy,
                entry.line,
            ));
            continue;
        }
        match &entry.message {
            None => {
                if owners.iter().all(|a| a.accepts.is_empty()) {
                    report.notes.push(format!(
                        "[auth] protect lists {}, but {} accepts no message, so there is nothing to protect.",
                        entry.text, entry.agent
                    ));
                }
            }
            Some(message) => {
                if !owners.iter().any(|a| a.accept(message).is_some()) {
                    let all: Vec<&str> = owners
                        .iter()
                        .flat_map(|a| a.accepts.iter().map(|x| x.message.as_str()))
                        .collect();
                    let fix = match closest_name(message, all.iter().copied()) {
                        Some(s) => format!("did you mean `{}.{}`?", entry.agent, s),
                        None => format!("{} accepts: {}", entry.agent, all.join(", ")),
                    };
                    report.errors.push(located(
                        Diagnostic::new(format!(
                            "[auth] protect lists {}, but no such message is declared.",
                            entry.text
                        ))
                        .fix(fix),
                        policy,
                        entry.line,
                    ));
                }
            }
        }
    }
    if policy.protect.is_empty() {
        report.notes.push(
            "[auth] protects no message, so every message is served without a token.".to_string(),
        );
    }
    // forward: names that can receive a token
    let mut receivers: Vec<&str> = Vec::new();
    for a in agents {
        for r in &a.remotes {
            receivers.push(r.name.as_str());
        }
        for t in &a.tools {
            if let ToolKind::Mcp { command } = &t.kind {
                if command.starts_with("http://") || command.starts_with("https://") {
                    receivers.push(t.name.as_str());
                }
            }
        }
    }
    for name in &policy.forward {
        if name == "llm" || name == "model" {
            report.errors.push(located(
                Diagnostic::new(format!(
                    "{} is the language model, so it cannot receive a token.",
                    name
                ))
                .fix("remove it from [auth] forward"),
                policy,
                policy.forward_line,
            ));
            continue;
        }
        let local_tool = agents
            .iter()
            .flat_map(|a| a.tools.iter())
            .find(|t| &t.name == name && !receivers.contains(&name.as_str()));
        if let Some(tool) = local_tool {
            let what = match tool.kind {
                ToolKind::Mcp { .. } => "runs as a local program",
                _ => "is a built in tool",
            };
            report.errors.push(located(
                Diagnostic::new(format!("{} {}, so it cannot receive a token.", name, what))
                    .fix("remove it from [auth] forward"),
                policy,
                policy.forward_line,
            ));
            continue;
        }
        if !receivers.contains(&name.as_str()) {
            let fix = match closest_name(name, receivers.iter().copied()) {
                Some(s) => format!("did you mean `{}`?", s),
                None => "list a remote agent or a tool reached by an http:// or https:// address"
                    .to_string(),
            };
            report.warnings.push(format!(
                "[auth] forward lists {}, but nothing with that name is declared ({})",
                name, fix
            ));
        }
    }
    // a forwarded remote at a plain http address that is not on this computer can never be called
    for a in agents {
        for r in &a.remotes {
            if policy.forwards_to(&r.name) && is_open_plain_http(&r.url) {
                report.warnings.push(format!(
                    "{} is in [auth] forward, but its address is plain http and not on this computer: the token would travel in the open. Use an https:// address.",
                    r.name
                ));
            }
        }
    }
    report
}

/// An `http://` address whose host is not on this computer.
pub fn is_open_plain_http(url: &str) -> bool {
    match reqwest::Url::parse(url) {
        Ok(u) if u.scheme() == "http" => !matches!(
            u.host_str()
                .map(|h| h.trim_matches(|c| c == '[' || c == ']')),
            Some("127.0.0.1") | Some("localhost") | Some("::1")
        ),
        _ => false,
    }
}
