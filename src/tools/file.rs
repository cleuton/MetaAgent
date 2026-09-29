//! `tool file`: read and write text files inside the folder the agent may use.

use super::{ActionInfo, Args, Tool, ToolError, need_text, unknown_action};
use crate::runtime::task::CallContext;
use crate::runtime::value::Value;
use async_trait::async_trait;
use std::path::{Component, Path, PathBuf};

pub struct FileTool {
    /// Folder that relative paths start from and that nothing may leave.
    base: PathBuf,
    /// How the scope reads in messages.
    shown: String,
}

impl FileTool {
    pub fn new(root: PathBuf, scope: Option<String>) -> FileTool {
        let (base, shown) = match &scope {
            Some(s) => (normalize(&root.join(s)), s.clone()),
            None => (normalize(&root), "the project folder".to_string()),
        };
        FileTool { base, shown }
    }

    fn resolve(&self, given: &str) -> Result<PathBuf, ToolError> {
        let wanted = normalize(&self.base.join(given));
        let outside = || {
            ToolError::new(format!(
                "`{}` is outside the folder this agent may use ({})",
                given, self.shown
            ))
            .fix("keep the file inside that folder, or change the agent's declaration, for example: tool file \"other-folder/\"")
        };
        let base_real = canonical_or_self(&self.base);
        if !wanted.starts_with(&self.base) && !canonical_or_self(&wanted).starts_with(&base_real) {
            return Err(outside());
        }
        // Follow symlinks: the nearest existing part of the path must still be inside.
        let mut existing = wanted.clone();
        let mut rest = PathBuf::new();
        while !existing.exists() {
            match (
                existing.file_name().map(PathBuf::from),
                existing.parent().map(Path::to_path_buf),
            ) {
                (Some(name), Some(parent)) => {
                    rest = if rest.as_os_str().is_empty() {
                        name
                    } else {
                        name.join(&rest)
                    };
                    existing = parent;
                }
                _ => break,
            }
        }
        let real = canonical_or_self(&existing).join(rest);
        if !real.starts_with(&base_real) {
            return Err(outside());
        }
        Ok(wanted)
    }
}

/// Removes `.` and resolves `..` without touching the disk.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn canonical_or_self(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[async_trait]
impl Tool for FileTool {
    fn name(&self) -> &str {
        "file"
    }

    async fn actions(&self) -> Result<Vec<ActionInfo>, ToolError> {
        Ok(describe())
    }

    async fn call(&self, action: &str, args: Args, _ctx: &CallContext) -> Result<Value, ToolError> {
        match action {
            "read" => {
                let path = need_text("file", "read", &args, "path")?;
                let full = self.resolve(&path)?;
                match std::fs::read(&full) {
                    Ok(bytes) => String::from_utf8(bytes).map(Value::Text).map_err(|_| {
                        ToolError::new(format!("`{}` is not a text file I can read", path))
                            .fix("only plain text files can be read with file.read")
                    }),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(ToolError::new(
                        format!("the file `{}` does not exist", path),
                    )
                    .fix(format!(
                        "check the spelling; files are looked up from {}",
                        self.base.display()
                    ))),
                    Err(e) => Err(
                        ToolError::new(format!("I could not read `{}`: {}", path, e))
                            .fix("check that you are allowed to read that file"),
                    ),
                }
            }
            "write" => {
                let path = need_text("file", "write", &args, "path")?;
                let text = need_text("file", "write", &args, "text")?;
                let full = self.resolve(&path)?;
                if let Some(parent) = full.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&full, text).map_err(|e| {
                    ToolError::new(format!("I could not write `{}`: {}", path, e))
                        .fix("check that the folder exists and you are allowed to write there")
                })?;
                Ok(Value::Nothing)
            }
            other => Err(unknown_action("file", other, &["read", "write"])),
        }
    }
}

/// What this tool can do, also used by `metagente check`.
pub fn describe() -> Vec<ActionInfo> {
    vec![
        ActionInfo::simple(
            "read",
            "Read a text file and return its content",
            &[("path", true)],
        ),
        ActionInfo::simple(
            "write",
            "Write text to a file",
            &[("path", true), ("text", true)],
        ),
    ]
}
