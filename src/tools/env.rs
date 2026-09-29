//! `tool env "NAME" ...`: read only the named environment variables.

use super::{ActionInfo, Args, Tool, ToolError, need_text, unknown_action};
use crate::runtime::task::CallContext;
use crate::runtime::value::Value;
use async_trait::async_trait;

pub struct EnvTool {
    allowed: Vec<String>,
    /// Variables agents may never read, such as the language model key.
    hidden: Vec<String>,
}

impl EnvTool {
    pub fn new(allowed: Vec<String>, hidden: Vec<String>) -> EnvTool {
        EnvTool { allowed, hidden }
    }
}

#[async_trait]
impl Tool for EnvTool {
    fn name(&self) -> &str {
        "env"
    }

    async fn actions(&self) -> Result<Vec<ActionInfo>, ToolError> {
        Ok(describe())
    }

    async fn call(&self, action: &str, args: Args, _ctx: &CallContext) -> Result<Value, ToolError> {
        if action != "get" {
            return Err(unknown_action("env", action, &["get"]));
        }
        let name = need_text("env", "get", &args, "name")?;
        if self.hidden.iter().any(|h| h == &name) {
            return Err(ToolError::new(format!(
                "`{}` holds the key for the language model, and agents can never read it",
                name
            ))
            .fix("use a different variable for your own values"));
        }
        if !self.allowed.iter().any(|a| a == &name) {
            let mut all = self.allowed.clone();
            all.push(name.clone());
            let quoted: Vec<String> = all.iter().map(|n| format!("\"{}\"", n)).collect();
            return Err(ToolError::new(format!(
                "this agent did not declare the variable `{}`",
                name
            ))
            .fix(format!(
                "change the declaration to: tool env {}",
                quoted.join(" ")
            )));
        }
        Ok(std::env::var(&name)
            .map(Value::Text)
            .unwrap_or(Value::Nothing))
    }
}

/// What this tool can do, also used by `metagente check`.
pub fn describe() -> Vec<ActionInfo> {
    vec![ActionInfo::simple(
        "get",
        "Read an environment variable",
        &[("name", true)],
    )]
}
