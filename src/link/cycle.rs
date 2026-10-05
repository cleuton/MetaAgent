//! Agents calling each other in a circle must stop with a clear message.

use crate::diagnostics::Diagnostic;

/// `chain` is the agents currently running, outermost first; `target` is who is about to be called.
pub fn check(chain: &[String], target: &str) -> Result<(), Diagnostic> {
    let Some(start) = chain.iter().position(|a| a == target) else {
        return Ok(());
    };
    let mut path: Vec<&str> = chain[start..].iter().map(String::as_str).collect();
    path.push(target);
    let shown = path.join(" -> ");
    let message = if path.len() == 3 {
        format!(
            "{} asked {}, and {} asked {} again ({})",
            path[0], path[1], path[1], path[0], shown
        )
    } else {
        format!(
            "these agents are calling each other in a circle ({})",
            shown
        )
    };
    Err(Diagnostic::new(message).fix("make one of them answer without calling the other"))
}
