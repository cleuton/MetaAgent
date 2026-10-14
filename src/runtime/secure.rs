// 0.1.3: new file. Which agents, tools and the model must be reached over https, and the one rule that refuses
// an `http://` address for them. Every network call site asks `decide` before it opens a connection.

use super::config::SecureConfig;
use super::net::redact;
use crate::diagnostics::Diagnostic;
use crate::lang::ast::{AgentDef, Expr, TextPart, ToolKind};
use crate::tools::{ToolError, closest_name};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// What is being reached.
#[derive(Debug, Clone, Copy)]
pub enum Target<'a> {
    /// A `remote Name at "..."` agent.
    Remote(&'a str),
    /// A tool by its declared name: an MCP server over http, or the built in `http` tool.
    Tool(&'a str),
    /// The language model provider.
    Model,
}

/// Who is asking.
#[derive(Debug, Clone, Copy)]
pub struct Caller<'a> {
    /// The name of the agent whose handler makes the call.
    pub agent: &'a str,
    /// True when a secure agent that loaded this one with `link` makes it secure too (D14).
    pub inherited: bool,
}

impl<'a> Caller<'a> {
    pub fn new(agent: &'a str, inherited: bool) -> Caller<'a> {
        Caller { agent, inherited }
    }
}

/// True for an address that starts with `http://` (any letter case, spaces around it ignored).
pub fn is_plain_http(address: &str) -> bool {
    let a = address.trim_start();
    a.len() >= 7 && a[..7].eq_ignore_ascii_case("http://")
}

/// True when this agent is bound by the secure rule: its own name is listed, or a secure loader passed it on.
pub fn agent_is_secure(list: &SecureConfig, caller: &Caller) -> bool {
    caller.inherited || list.lists_agent(caller.agent)
}

/// Refuses an `http://` address for something declared secure, or for a secure agent, before any connection.
/// Anything that is not an `http://` address (https, a command, a path) passes: only network addresses are checked.
pub fn decide(
    list: &SecureConfig,
    caller: &Caller,
    target: Target,
    address: &str,
) -> Result<(), ToolError> {
    if !is_plain_http(address) {
        return Ok(());
    }
    let shown = redact(address);
    let (listed, what, section) = match target {
        Target::Remote(name) => (list.lists_agent(name), name, "[secure] agents"),
        Target::Tool(name) => (list.lists_tool(name), name, "[secure] tools"),
        Target::Model => (list.llm, "the language model", "[secure] llm"),
    };
    match target {
        Target::Model if listed => Err(ToolError::new(
            "The model address in [llm] uses http, and [secure] llm is on. Nothing was sent.",
        )
        .fix("use an address that starts with https:// in [llm] (base_url), or remove llm from [secure]")),
        _ if listed => Err(ToolError::new(format!(
            "{} is declared secure in metagente.toml but this address uses http: {}",
            what, shown
        ))
        .fix(format!(
            "write https:// instead of http://, or remove {} from {}",
            what, section
        ))),
        _ if agent_is_secure(list, caller) => {
            let place = match target {
                Target::Model => "The model address in [llm]".to_string(),
                _ => "This address".to_string(),
            };
            Err(ToolError::new(format!(
                "{} is declared secure, so it may only call https addresses. {} uses http: {}",
                caller.agent, place, shown
            ))
            .fix(match target {
                Target::Model => "use an address that starts with https:// in [llm] (base_url)",
                _ => "change the address so that it starts with https://",
            }))
        }
        _ => Ok(()),
    }
}

/// The card of a remote agent says where tasks go. For something secure that must be an https address on the same
/// host as the address the person wrote: a card cannot move a secure agent to another host (US1 scenario 5).
pub fn endpoint_rule(
    list: &SecureConfig,
    caller: &Caller,
    name: &str,
    from: &str,
    to: &str,
) -> Result<(), ToolError> {
    decide(list, caller, Target::Remote(name), to)?;
    let listed = list.lists_agent(name);
    if !(listed || agent_is_secure(list, caller)) {
        return Ok(());
    }
    if let (Ok(a), Ok(b)) = (
        reqwest::Url::parse(from.trim()),
        reqwest::Url::parse(to.trim()),
    ) {
        if a.host_str() != b.host_str() {
            let who = if listed { name } else { caller.agent };
            return Err(ToolError::new(format!(
                "the agent card of {} points tasks to {}, a different host than {}; {} is declared secure, so I will not follow it",
                name,
                redact(to),
                redact(from),
                who
            ))
            .fix(format!(
                "serve the agent so that its card points to its own address, or remove {} from [secure] agents",
                who
            )));
        }
    }
    Ok(())
}

// ---------- `metagente check` ----------

/// What `metagente check` reports about the secure list.
#[derive(Default)]
pub struct SecureReport {
    pub problems: Vec<Diagnostic>,
    pub warnings: Vec<String>,
    pub notes: Vec<String>,
}

fn problem_at(file: &Path, line: usize, message: String, fix: &str) -> Diagnostic {
    let mut d = Diagnostic::new(message).fix(fix);
    if line > 0 {
        d = d.at(&file.display().to_string(), line, 1);
    }
    d
}

/// Every `.ag` file in `dir` and below it (not `target`, not hidden folders).
fn agent_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for path in paths {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if path.is_dir() {
            if !name.starts_with('.') && name != "target" {
                agent_files(&path, out);
            }
        } else if path.extension().map(|e| e == "ag").unwrap_or(false) {
            out.push(path);
        }
    }
}

/// What the `.ag` files under the folder of metagente.toml declare.
#[derive(Default)]
struct Declared {
    /// Agent names and `remote` names.
    agents: BTreeSet<String>,
    /// Tool names with their kind: true when the tool is reached over a network address.
    tools: Vec<(String, bool)>,
}

fn declared_under(root: &Path, params: &crate::lang::Params) -> Declared {
    let mut files = Vec::new();
    agent_files(root, &mut files);
    let mut out = Declared::default();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let name = file
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let Ok(defs) = crate::lang::parse_file_with(&name, Some(file.clone()), &text, params)
        else {
            continue;
        };
        for def in defs {
            out.agents.insert(def.name.clone());
            for r in &def.remotes {
                out.agents.insert(r.name.clone());
            }
            for t in &def.tools {
                let net = match &t.kind {
                    ToolKind::Http => true,
                    ToolKind::Mcp { command } => {
                        let c = command.trim_start().to_ascii_lowercase();
                        c.starts_with("http://") || c.starts_with("https://")
                    }
                    _ => false,
                };
                out.tools.push((t.name.clone(), net));
            }
        }
    }
    out
}

/// The text of an argument when it is written as one piece of text with no `{name}` in it.
fn literal_text(expr: &Expr) -> Option<String> {
    if let Expr::Text(parts, _) = expr {
        let mut out = String::new();
        for p in parts {
            match p {
                TextPart::Lit(s) => out.push_str(s),
                TextPart::Var(_) => return None,
            }
        }
        return Some(out);
    }
    None
}

/// The checks that need only the agents of the file: literal `http://` addresses of secure names and of secure agents.
pub fn check_agents(agents: &[Arc<AgentDef>], list: &SecureConfig) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    for agent in agents {
        let caller = Caller::new(&agent.name, false);
        let source = agent.source.text.clone();
        let mut add = |span: crate::lang::Span, e: ToolError| {
            let mut d = Diagnostic::new(e.message).at(&agent.source.name, span.line, span.col);
            if let Some(f) = e.fix {
                d = d.fix(f);
            }
            problems.push(d.with_source(source.clone()));
        };
        for remote in &agent.remotes {
            if let Err(e) = decide(list, &caller, Target::Remote(&remote.name), &remote.url) {
                add(remote.span, e);
            }
        }
        for tool in &agent.tools {
            if let ToolKind::Mcp { command } = &tool.kind {
                if let Err(e) = decide(list, &caller, Target::Tool(&tool.name), command) {
                    add(tool.span, e);
                }
            }
            // a tool that runs as a local program, or never uses the network, has no address to secure (D8)
            if list.lists_tool(&tool.name) {
                let local = match &tool.kind {
                    ToolKind::Http => None,
                    ToolKind::Mcp { command } => {
                        let c = command.trim_start().to_ascii_lowercase();
                        if c.starts_with("http://") || c.starts_with("https://") {
                            None
                        } else {
                            Some(format!(
                                "{} runs as a local program, so it has no address to secure.",
                                tool.name
                            ))
                        }
                    }
                    _ => Some(format!(
                        "{} is a built in tool that never uses the network, so it has no address to secure.",
                        tool.name
                    )),
                };
                if let Some(message) = local {
                    add(
                        tool.span,
                        ToolError::new(message).fix(
                            "remove it from [secure] tools, or give the tool an https:// address",
                        ),
                    );
                }
            }
        }
        let mut all = Vec::new();
        for h in agent.handlers.iter().chain(agent.start.iter()) {
            all.extend(crate::lang::check::collect_calls(&h.body));
        }
        for call in all {
            let is_http_tool = agent
                .tools
                .iter()
                .any(|t| t.name == call.target && t.kind == ToolKind::Http);
            if !is_http_tool {
                continue;
            }
            for (key, value) in &call.args {
                if key == "url" {
                    if let Some(url) = literal_text(value) {
                        if let Err(e) = decide(list, &caller, Target::Tool(&call.target), &url) {
                            add(call.span, e);
                        }
                    }
                }
            }
        }
    }
    problems
}

/// Everything `metagente check` says about `[secure]` and the serving certificate (FR-003, D8, D16).
pub fn check_project(agents: &[Arc<AgentDef>], start: &Path) -> SecureReport {
    let mut report = SecureReport::default();
    let found = match super::config::Config::secure_for_check(start) {
        Ok(f) => f,
        Err(d) => {
            report.problems.push(d);
            return report;
        }
    };
    let (list, tls, path, unknown, root) = match found {
        Some(f) => (f.secure, f.serve_tls, f.path, f.unknown_keys, f.root),
        None => (
            SecureConfig::default(),
            Default::default(),
            PathBuf::new(),
            Vec::new(),
            start.to_path_buf(),
        ),
    };
    for (key, line) in unknown {
        report.problems.push(problem_at(
            &path,
            line,
            format!(
                "metagente.toml has an unknown setting `{}` in [secure]",
                key
            ),
            "the settings of [secure] are agents, tools and llm",
        ));
    }
    let params = super::config::Config::load_parameters(&root).unwrap_or_default();
    if !list.agents.is_empty() || !list.tools.is_empty() {
        let declared = declared_under(&root, &params);
        for name in &list.agents {
            if !declared.agents.contains(name) {
                let near = closest_name(name, declared.agents.iter().map(|s| s.as_str()));
                report.warnings.push(format!(
                    "[secure] lists the agent {}, but no agent or remote called {} is declared here.{}",
                    name,
                    name,
                    near.map(|n| format!(" Did you mean {}?", n)).unwrap_or_default()
                ));
            }
        }
        for name in &list.tools {
            if !declared.tools.iter().any(|(n, _)| n == name) {
                let near = closest_name(name, declared.tools.iter().map(|(n, _)| n.as_str()));
                report.warnings.push(format!(
                    "[secure] lists the tool {}, but no tool called {} is declared here.{}",
                    name,
                    name,
                    near.map(|n| format!(" Did you mean {}?", n))
                        .unwrap_or_default()
                ));
            }
        }
    }
    // `[secure] llm`: the model address in [llm] must be https (FR-008)
    if list.llm {
        if let Ok(cfg) = super::config::Config::load_with_env(&root, &|_| None) {
            if let Some(address) = crate::llm::providers::model_address(&cfg) {
                let caller = Caller::new("", false);
                if let Err(e) = decide(&list, &caller, Target::Model, &address) {
                    report.problems.push(problem_at(
                        &path,
                        list.llm_line,
                        e.message,
                        e.fix.as_deref().unwrap_or("use an https:// address"),
                    ));
                }
            }
        }
    }
    // the serving certificate: `check` never needs it (D16), it only says what `serve` will need
    for (file, setting) in [(&tls.cert, "tls_cert"), (&tls.key, "tls_key")] {
        if let Some(f) = file {
            if !f.is_file() {
                report.warnings.push(format!(
                    "{} in [serve] points to {}, which does not exist yet; `metagente serve` will need it",
                    setting,
                    f.display()
                ));
            }
        }
    }
    report.problems.extend(check_agents(agents, &list));
    for agent in agents {
        if list.lists_agent(&agent.name) {
            let local: Vec<&str> = agent
                .tools
                .iter()
                .filter(|t| !matches!(t.kind, ToolKind::Http))
                .filter(|t| match &t.kind {
                    ToolKind::Mcp { command } => !command
                        .trim_start()
                        .to_ascii_lowercase()
                        .starts_with("http"),
                    _ => true,
                })
                .map(|t| t.name.as_str())
                .collect();
            if !local.is_empty() || !agent.links.is_empty() {
                report.notes.push(format!(
                    "{} is declared secure. Its local tools and links ({}) are not network traffic, so the https rule does not apply to them.",
                    agent.name,
                    local
                        .iter()
                        .copied()
                        .chain(agent.links.iter().map(|l| l.name.as_str()))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
    }
    report
}
