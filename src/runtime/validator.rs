//! 0.1.4: runs the validator program named in `[auth] validator`: the token goes in on standard input, the exit code
//! comes back. Directly, with no shell, in a clean environment (D5, D6).

use super::auth::{AuthPolicy, Token};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;

/// 0.1.4: at most 8 validators run at the same time (D6); the others wait for a place.
static SLOTS: Semaphore = Semaphore::const_new(8);

/// What the validator said.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Accepted,
    /// Any other result: an exit code, or `None` when the program was stopped by a signal.
    Refused(Option<i32>),
    /// It could not be asked: not found, cannot start, too slow. The text finishes "I could not ask the validator (..): ..".
    Unavailable(String),
}

/// 0.1.4: the program to start: a first word with a path separator is relative to the folder of metagente.toml,
/// anything else is looked up in `PATH`.
fn program_path(word: &str, base: &Path) -> PathBuf {
    if word.contains('/') || word.contains('\\') {
        let p = Path::new(word);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            base.join(p)
        }
    } else {
        PathBuf::from(word)
    }
}

fn plain_io(e: &std::io::Error) -> String {
    let text = e.to_string();
    match text.find(" (os error") {
        Some(i) => text[..i].to_lowercase(),
        None => text.to_lowercase(),
    }
}

pub async fn run(policy: &AuthPolicy, token: &Token) -> Outcome {
    let Some(command) = &policy.validator else {
        return Outcome::Unavailable("no validator is set in [auth]".to_string());
    };
    let words = crate::mcp::client::split_command(command);
    let Some((first, rest)) = words.split_first() else {
        return Outcome::Unavailable("the validator command is empty".to_string());
    };
    let limit = Duration::from_secs(policy.timeout_seconds.max(1));
    let work = async {
        // the wait for a free place counts against the time limit (FR-026)
        let _slot = SLOTS.acquire().await;
        let mut cmd = tokio::process::Command::new(program_path(first, &policy.base_dir));
        cmd.args(rest)
            .current_dir(&policy.base_dir)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        // only PATH, and SystemRoot on Windows: the validator never sees the model key or any other variable
        if let Some(path) = std::env::var_os("PATH") {
            cmd.env("PATH", path);
        }
        #[cfg(windows)]
        if let Some(root) = std::env::var_os("SystemRoot") {
            cmd.env("SystemRoot", root);
        }
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Outcome::Unavailable(format!("the program {} was not found", first));
            }
            Err(e) => {
                return Outcome::Unavailable(format!(
                    "the program {} could not be started ({})",
                    first,
                    plain_io(&e)
                ));
            }
        };
        if let Some(mut stdin) = child.stdin.take() {
            // the token and one line break, then the input is closed; a validator that never reads is not an error here
            let mut line = Vec::with_capacity(token.expose().len() + 1);
            line.extend_from_slice(token.expose().as_bytes());
            line.push(b'\n');
            let _ = stdin.write_all(&line).await;
            let _ = stdin.shutdown().await;
            drop(stdin);
        }
        match child.wait().await {
            Ok(status) => match status.code() {
                Some(0) => Outcome::Accepted,
                other => Outcome::Refused(other),
            },
            Err(e) => {
                Outcome::Unavailable(format!("it stopped without an answer ({})", plain_io(&e)))
            }
        }
    };
    match tokio::time::timeout(limit, work).await {
        Ok(outcome) => outcome,
        Err(_) => Outcome::Unavailable(format!(
            "it did not answer in {} second{}",
            limit.as_secs(),
            if limit.as_secs() == 1 { "" } else { "s" }
        )),
    }
}
