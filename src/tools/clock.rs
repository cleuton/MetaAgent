//! `tool clock`: the current time, and waiting.

use super::{ActionInfo, Args, Tool, ToolError, unknown_action};
use crate::runtime::task::CallContext;
use crate::runtime::value::Value;
use async_trait::async_trait;

pub struct ClockTool;

#[async_trait]
impl Tool for ClockTool {
    fn name(&self) -> &str {
        "clock"
    }

    async fn actions(&self) -> Result<Vec<ActionInfo>, ToolError> {
        Ok(describe())
    }

    async fn call(&self, action: &str, args: Args, _ctx: &CallContext) -> Result<Value, ToolError> {
        match action {
            "now" => {
                let now = chrono::Utc::now();
                Ok(Value::record([
                    ("text".to_string(), Value::Text(now.to_rfc3339())),
                    ("unix".to_string(), Value::Number(now.timestamp() as f64)),
                ]))
            }
            "wait" => {
                let seconds = args
                    .get("seconds")
                    .and_then(|v| v.as_number())
                    .ok_or_else(|| {
                        ToolError::new("`clock.wait` needs a number of seconds")
                            .fix("write it like: clock.wait seconds: 5")
                    })?;
                if !(0.0..=86_400.0).contains(&seconds) {
                    return Err(
                        ToolError::new("`clock.wait` needs between 0 and 86400 seconds")
                            .fix("use a smaller number of seconds"),
                    );
                }
                tokio::time::sleep(std::time::Duration::from_secs_f64(seconds)).await;
                Ok(Value::Nothing)
            }
            other => Err(unknown_action("clock", other, &["now", "wait"])),
        }
    }
}

/// What this tool can do, also used by `metagente check`.
pub fn describe() -> Vec<ActionInfo> {
    vec![
        ActionInfo::simple("now", "The current date and time", &[]),
        ActionInfo::simple("wait", "Wait for a number of seconds", &[("seconds", true)]),
    ]
}
