//! Checks a message against the public interface (`accepts`) of an agent before it runs.

use crate::diagnostics::Diagnostic;
use crate::lang::AgentDef;
use crate::tools::{Args, closest_name};

pub fn check(def: &AgentDef, message: &str, args: &Args) -> Result<(), Diagnostic> {
    let Some(accept) = def.accept(message) else {
        let known: Vec<&str> = def.accepts.iter().map(|a| a.message.as_str()).collect();
        let mut d = Diagnostic::new(format!(
            "agent {} does not accept the message `{}`",
            def.name, message
        ));
        d = if known.is_empty() {
            d.fix(format!(
                "add a line like `accepts {}` to agent {}",
                message, def.name
            ))
        } else if let Some(s) = closest_name(message, known.iter().copied()) {
            d.fix(format!(
                "did you mean `{}`? {} accepts: {}",
                s,
                def.name,
                known.join(", ")
            ))
        } else {
            d.fix(format!("{} accepts: {}", def.name, known.join(", ")))
        };
        return Err(d);
    };
    for param in &accept.params {
        if !args.contains_key(param) {
            return Err(Diagnostic::new(format!(
                "the message `{}` of agent {} needs a value for `{}`",
                message, def.name, param
            ))
            .fix(format!("send it like: {} {}: \"...\"", message, param)));
        }
    }
    for key in args.keys() {
        if !accept.params.contains(key) {
            let mut d = Diagnostic::new(format!(
                "the message `{}` of agent {} does not take `{}`",
                message, def.name, key
            ));
            let params: Vec<&str> = accept.params.iter().map(String::as_str).collect();
            d = match closest_name(key, params.iter().copied()) {
                Some(s) => d.fix(format!("did you mean `{}`?", s)),
                None if params.is_empty() => d.fix(format!("`{}` takes no values", message)),
                None => d.fix(format!("`{}` takes: {}", message, params.join(", "))),
            };
            return Err(d);
        }
    }
    Ok(())
}
