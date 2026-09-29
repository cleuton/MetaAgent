//! Checks links before running: the target must exist and must accept what is sent to it.

use super::resolve::resolve;
use super::{Linker, interface};
use crate::diagnostics::Diagnostic;
use crate::lang::AgentDef;
use crate::lang::check::collect_calls;
use crate::runtime::value::Value;
use crate::tools::Args;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

pub fn check_links(linker: &Linker, root: &Path, agents: &[Arc<AgentDef>]) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    for agent in agents {
        let mut targets: HashMap<String, Arc<AgentDef>> = HashMap::new();
        for decl in &agent.links {
            match resolve(linker, root, agent, &decl.name, decl.path.as_deref()) {
                Ok(target) => {
                    targets.insert(decl.name.clone(), target);
                }
                Err(d) => problems.push(d.located(
                    &agent.source.name,
                    decl.span.line,
                    decl.span.col,
                    &agent.source.text,
                )),
            }
        }
        for handler in agent.handlers.iter().chain(agent.start.iter()) {
            for call in collect_calls(&handler.body) {
                let Some(target) = targets.get(&call.target) else {
                    continue;
                };
                let args: Args = call
                    .args
                    .iter()
                    .map(|(k, _)| (k.clone(), Value::Nothing))
                    .collect();
                if let Err(d) = interface::check(target, &call.action, &args) {
                    problems.push(d.located(
                        &agent.source.name,
                        call.span.line,
                        call.span.col,
                        &agent.source.text,
                    ));
                }
            }
        }
    }
    problems.sort_by_key(|d| (d.line, d.column));
    problems
}
