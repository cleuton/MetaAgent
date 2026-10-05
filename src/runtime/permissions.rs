//! What an agent may use is exactly what it declared.

use crate::diagnostics::Diagnostic;
use crate::lang::{AgentDef, ToolKind};
use std::collections::HashMap;

pub const BUILTIN_NAMES: &[&str] = &["file", "http", "env", "state", "clock"];

#[derive(Debug, Clone, PartialEq)]
pub enum Granted {
    Builtin,
    Mcp,
    Link,
    Remote,
}

#[derive(Debug, Clone)]
pub struct Capabilities {
    agent: String,
    granted: HashMap<String, Granted>,
}

impl Capabilities {
    pub fn from_agent(def: &AgentDef) -> Capabilities {
        let mut granted = HashMap::new();
        for tool in &def.tools {
            let kind = match tool.kind {
                ToolKind::Mcp { .. } => Granted::Mcp,
                _ => Granted::Builtin,
            };
            granted.insert(tool.name.clone(), kind);
        }
        for link in &def.links {
            granted.insert(link.name.clone(), Granted::Link);
        }
        for remote in &def.remotes {
            granted.insert(remote.name.clone(), Granted::Remote);
        }
        Capabilities {
            agent: def.name.clone(),
            granted,
        }
    }

    pub fn allows(&self, target: &str) -> bool {
        self.granted.contains_key(target)
    }

    pub fn declared_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.granted.keys().cloned().collect();
        names.sort();
        names
    }

    /// Returns a plain language refusal when the agent did not declare `target`.
    pub fn check(&self, target: &str) -> Result<(), Diagnostic> {
        if self.allows(target) {
            return Ok(());
        }
        if BUILTIN_NAMES.contains(&target) {
            return Err(Diagnostic::new(format!(
                "agent {} uses `{}` but never declared it",
                self.agent, target
            ))
            .fix(format!(
                "add the line `tool {}` under `agent {}`",
                target, self.agent
            )));
        }
        let declared = self.declared_names();
        let mut d = Diagnostic::new(format!(
            "agent {} does not know anything called `{}`",
            self.agent, target
        ));
        if declared.is_empty() {
            d = d.fix(format!(
                "declare it under `agent {}` with `tool {} from mcp \"command\"`, `link {}`, or `remote {} at \"address\"`",
                self.agent, target, target, target
            ));
        } else {
            d = d.fix(format!(
                "use one of the names this agent declared ({}), or declare `{}` under `agent {}`",
                declared.join(", "),
                target,
                self.agent
            ));
        }
        Err(d)
    }
}
