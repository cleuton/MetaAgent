//! Tools an agent can call: built in ones here, MCP tools in `crate::mcp`,
//! other agents in `crate::link`, remote agents in `crate::a2a`.

pub mod clock;
pub mod env;
pub mod file;
pub mod http;
pub mod state;

use crate::diagnostics::Diagnostic;
use crate::lang::{AgentDef, ToolDecl, ToolKind};
use crate::runtime::Runtime;
use crate::runtime::task::CallContext;
use crate::runtime::value::Value;
use async_trait::async_trait;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

pub type Args = BTreeMap<String, Value>;

/// A failure inside a tool, in plain words. The interpreter adds the line it happened on.
#[derive(Debug, Clone)]
pub struct ToolError {
    pub message: String,
    pub fix: Option<String>,
    /// Extra lines of context, for example a problem that happened inside a linked agent.
    pub related: Vec<String>,
}

impl ToolError {
    pub fn new(message: impl Into<String>) -> ToolError {
        ToolError {
            message: message.into(),
            fix: None,
            related: Vec::new(),
        }
    }
    pub fn fix(mut self, fix: impl Into<String>) -> ToolError {
        self.fix = Some(fix.into());
        self
    }
    pub fn related(mut self, lines: impl IntoIterator<Item = String>) -> ToolError {
        self.related.extend(lines);
        self
    }
}

impl From<Diagnostic> for ToolError {
    fn from(d: Diagnostic) -> ToolError {
        ToolError {
            message: d.message,
            fix: d.suggestion,
            related: d.related,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParamInfo {
    pub name: String,
    pub required: bool,
}

#[derive(Debug, Clone)]
pub struct ActionInfo {
    pub name: String,
    pub description: String,
    pub params: Vec<ParamInfo>,
    /// The tool's own JSON schema when it has one (MCP tools do).
    pub schema: Option<serde_json::Value>,
}

impl ActionInfo {
    pub fn simple(name: &str, description: &str, params: &[(&str, bool)]) -> ActionInfo {
        ActionInfo {
            name: name.to_string(),
            description: description.to_string(),
            params: params
                .iter()
                .map(|(n, r)| ParamInfo {
                    name: n.to_string(),
                    required: *r,
                })
                .collect(),
            schema: None,
        }
    }

    pub fn json_schema(&self) -> serde_json::Value {
        if let Some(schema) = &self.schema {
            return schema.clone();
        }
        let mut props = serde_json::Map::new();
        let mut required = Vec::new();
        for p in &self.params {
            props.insert(p.name.clone(), serde_json::json!({ "type": "string" }));
            if p.required {
                required.push(serde_json::Value::String(p.name.clone()));
            }
        }
        serde_json::json!({ "type": "object", "properties": props, "required": required })
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    async fn actions(&self) -> Result<Vec<ActionInfo>, ToolError>;
    async fn call(&self, action: &str, args: Args, ctx: &CallContext) -> Result<Value, ToolError>;
}

pub type Registry = HashMap<String, Arc<dyn Tool>>;

/// Finds the closest name, for "did you mean" hints.
pub fn closest_name<'a>(word: &str, options: impl IntoIterator<Item = &'a str>) -> Option<String> {
    fn distance(a: &str, b: &str) -> usize {
        let a: Vec<char> = a.chars().collect();
        let b: Vec<char> = b.chars().collect();
        let mut prev: Vec<usize> = (0..=b.len()).collect();
        for i in 1..=a.len() {
            let mut cur = vec![i];
            for j in 1..=b.len() {
                let cost = usize::from(a[i - 1] != b[j - 1]);
                cur.push((prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost));
            }
            prev = cur;
        }
        prev[b.len()]
    }
    options
        .into_iter()
        .map(|o| (distance(word, o), o))
        .filter(|(d, _)| *d <= 2 && *d < word.chars().count())
        .min_by_key(|(d, _)| *d)
        .map(|(_, o)| o.to_string())
}

/// Error for an action the tool does not have, listing the ones it does.
pub fn unknown_action(tool: &str, action: &str, available: &[&str]) -> ToolError {
    let mut fix = format!("`{}` can do: {}", tool, available.join(", "));
    if let Some(s) = closest_name(action, available.iter().copied()) {
        fix = format!("did you mean `{}.{}`? ({})", tool, s, fix);
    }
    ToolError::new(format!("`{}` has no action called `{}`", tool, action)).fix(fix)
}

/// Reads a required argument as text.
pub fn need_text(tool: &str, action: &str, args: &Args, key: &str) -> Result<String, ToolError> {
    match args.get(key) {
        Some(Value::Text(s)) => Ok(s.clone()),
        Some(Value::Nothing) | None => Err(ToolError::new(format!(
            "`{}.{}` needs a value for `{}`",
            tool, action, key
        ))
        .fix(format!(
            "add it to the call, for example: {}.{} {}: \"...\"",
            tool, action, key
        ))),
        Some(other) => Ok(other.to_display()),
    }
}

/// Builds the tools an agent declared. Nothing else is reachable from the agent.
pub fn build_registry(rt: &Arc<Runtime>, def: &Arc<AgentDef>) -> Result<Registry, Diagnostic> {
    let mut registry: Registry = HashMap::new();
    for decl in &def.tools {
        let tool: Arc<dyn Tool> = build_tool(rt, def, decl)?;
        registry.insert(decl.name.clone(), tool);
    }
    for link in &def.links {
        registry.insert(
            link.name.clone(),
            Arc::new(crate::link::LinkTool::new(
                rt.clone(),
                def.clone(),
                link.clone(),
            )),
        );
    }
    for remote in &def.remotes {
        registry.insert(
            remote.name.clone(),
            Arc::new(crate::a2a::client::RemoteTool::new(
                rt.clone(),
                remote.clone(),
            )),
        );
    }
    Ok(registry)
}

fn build_tool(
    rt: &Arc<Runtime>,
    def: &Arc<AgentDef>,
    decl: &ToolDecl,
) -> Result<Arc<dyn Tool>, Diagnostic> {
    Ok(match &decl.kind {
        ToolKind::File { scope } => {
            Arc::new(file::FileTool::new(rt.config.root.clone(), scope.clone()))
        }
        // 0.1.3: certificates and proxies come from [network]
        ToolKind::Http => Arc::new(http::HttpTool::new(rt.net.clone())),
        ToolKind::State => Arc::new(state::StateTool::new(rt.state_for(&def.name))),
        ToolKind::Clock => Arc::new(clock::ClockTool),
        ToolKind::Env { names } => {
            let hidden = rt.hidden_env();
            Arc::new(env::EnvTool::new(names.clone(), hidden))
        }
        ToolKind::Mcp { command } => Arc::new(crate::mcp::tool::McpTool::new(
            rt.clone(),
            decl.name.clone(),
            command.clone(),
        )),
    })
}

/// The actions of a built in tool, by its name. Used to check agents before they run.
pub fn builtin_actions(name: &str) -> Option<Vec<ActionInfo>> {
    match name {
        "file" => Some(file::describe()),
        "http" => Some(http::describe()),
        "env" => Some(env::describe()),
        "state" => Some(state::describe()),
        "clock" => Some(clock::describe()),
        _ => None,
    }
}
