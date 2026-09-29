//! Finds the agent a `link` refers to.
//! Order: same file, next to the calling file, the project's `agents/` folder.
//! A quoted path is used exactly as written.

use super::Linker;
use crate::diagnostics::{Diagnostic, MgResult};
use crate::lang::AgentDef;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn pick(
    agents: Vec<Arc<AgentDef>>,
    name: &str,
    path: &Path,
    explicit: bool,
) -> MgResult<Arc<AgentDef>> {
    if let Some(found) = agents.iter().find(|a| a.name == name) {
        return Ok(found.clone());
    }
    if explicit && agents.len() == 1 {
        return Ok(agents[0].clone());
    }
    let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
    Err(
        Diagnostic::new(format!("{} has no agent called {}", path.display(), name))
            .fix(format!("it has: {}", names.join(", "))),
    )
}

fn candidates(dir: &Path, name: &str) -> Vec<PathBuf> {
    let mut out = vec![dir.join(format!("{}.ag", name))];
    let lower = dir.join(format!("{}.ag", name.to_lowercase()));
    if lower != out[0] {
        out.push(lower);
    }
    out
}

pub fn resolve(
    linker: &Linker,
    root: &Path,
    caller: &AgentDef,
    name: &str,
    quoted: Option<&str>,
) -> MgResult<Arc<AgentDef>> {
    let caller_dir: PathBuf = caller
        .source
        .path
        .as_ref()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| root.to_path_buf());
    if let Some(quoted) = quoted {
        let path = if Path::new(quoted).is_absolute() {
            PathBuf::from(quoted)
        } else {
            caller_dir.join(quoted)
        };
        if !path.is_file() {
            return Err(Diagnostic::new(format!(
                "I could not find the agent file {}",
                path.display()
            ))
            .related(format!(
                "agent {} asked for it with: link {} from \"{}\"",
                caller.name, name, quoted
            ))
            .fix(
                "check the path, which starts from the folder of the file that has the link line",
            ));
        }
        return pick(linker.load(&path)?, name, &path, true);
    }
    let mut looked: Vec<String> = Vec::new();
    if let Some(found) = linker
        .siblings(&caller.source)
        .into_iter()
        .find(|a| a.name == name)
    {
        return Ok(found);
    }
    looked.push("in the same file".to_string());
    for dir in [caller_dir.clone(), root.join("agents")] {
        for path in candidates(&dir, name) {
            looked.push(path.display().to_string());
            if path.is_file() {
                return pick(linker.load(&path)?, name, &path, false);
            }
        }
    }
    let mut d = Diagnostic::new(format!("I could not find an agent called {}", name));
    d = d.related("I looked:".to_string());
    for place in &looked {
        d = d.related(format!("  {}", place));
    }
    Err(d.fix(format!(
        "create {}.ag next to {}, or write the path yourself: link {} from \"path/to/file.ag\"",
        name.to_lowercase(),
        caller.source.name,
        name
    )))
}
