//! Loads agent files on demand and notices when they change on disk.

use crate::diagnostics::{Diagnostic, MgResult};
use crate::lang::{AgentDef, Params, SourceFile, parse_file_with}; // 0.1.2: parse with parameters
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

struct Cached {
    modified: Option<SystemTime>,
    // 0.1.2: the parameters the file was parsed with; another set means parsing it again
    params: Params,
    agents: Vec<Arc<AgentDef>>,
}

#[derive(Default)]
pub struct Linker {
    files: Mutex<HashMap<PathBuf, Cached>>,
    /// Agents of files that are already loaded, so `link` can find siblings in the same file.
    registered: Mutex<HashMap<String, Vec<Arc<AgentDef>>>>,
    // 0.1.2: the parameters of the agent that started everything; linked agents use these
    params: Mutex<Params>,
}

pub fn file_key(source: &SourceFile) -> String {
    match &source.path {
        Some(p) => std::fs::canonicalize(p)
            .unwrap_or_else(|_| p.clone())
            .display()
            .to_string(),
        None => source.name.clone(),
    }
}

impl Linker {
    /// Remembers the agents of a file so other agents in that file can be linked by name.
    pub fn register(&self, agents: &[Arc<AgentDef>]) {
        if let (Some(first), Ok(mut map)) = (agents.first(), self.registered.lock()) {
            map.insert(file_key(&first.source), agents.to_vec());
        }
    }

    pub fn siblings(&self, source: &SourceFile) -> Vec<Arc<AgentDef>> {
        self.registered
            .lock()
            .ok()
            .and_then(|m| m.get(&file_key(source)).cloned())
            .unwrap_or_default()
    }

    /// 0.1.2: sets the parameters every loaded file is parsed with.
    pub fn set_parameters(&self, params: Params) {
        if let Ok(mut current) = self.params.lock() {
            *current = params;
        }
    }

    /// 0.1.2: the parameters every loaded file is parsed with.
    pub fn parameters(&self) -> Params {
        self.params.lock().map(|p| p.clone()).unwrap_or_default()
    }

    /// Loads a file, reading it again only when it changed on disk.
    // 0.1.2: it is also read again when the parameters are not the ones it was parsed with
    pub fn load(&self, path: &Path) -> MgResult<Vec<Arc<AgentDef>>> {
        let params = self.parameters(); // 0.1.2
        let real = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let modified = std::fs::metadata(&real).and_then(|m| m.modified()).ok();
        if let Ok(files) = self.files.lock() {
            if let Some(cached) = files.get(&real) {
                if cached.modified == modified && modified.is_some() && cached.params == params {
                    return Ok(cached.agents.clone());
                }
            }
        }
        let text = std::fs::read_to_string(&real).map_err(|e| {
            Diagnostic::new(format!("I could not open {}: {}", path.display(), e))
                .fix("check that the file is there")
        })?;
        let name = real
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let agents: Vec<Arc<AgentDef>> =
            parse_file_with(&name, Some(real.clone()), &text, &params)?
                .into_iter()
                .map(Arc::new)
                .collect();
        self.register(&agents);
        if let Ok(mut files) = self.files.lock() {
            files.insert(
                real,
                Cached {
                    modified,
                    params, // 0.1.2
                    agents: agents.clone(),
                },
            );
        }
        Ok(agents)
    }
}
