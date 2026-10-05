//! `tool state`: the agent's short term memory.

use super::{ActionInfo, Args, Tool, ToolError, need_text, unknown_action};
use crate::runtime::task::CallContext;
use crate::runtime::value::Value;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub type StateMap = Arc<Mutex<HashMap<String, Value>>>;

pub struct StateTool {
    store: StateMap,
}

impl StateTool {
    pub fn new(store: StateMap) -> StateTool {
        StateTool { store }
    }
}

#[async_trait]
impl Tool for StateTool {
    fn name(&self) -> &str {
        "state"
    }

    async fn actions(&self) -> Result<Vec<ActionInfo>, ToolError> {
        Ok(describe())
    }

    async fn call(&self, action: &str, args: Args, _ctx: &CallContext) -> Result<Value, ToolError> {
        let lock_failed = || ToolError::new("the agent's memory is not available right now");
        match action {
            "set" => {
                let key = need_text("state", "set", &args, "key")?;
                let value = args.get("value").cloned().unwrap_or(Value::Nothing);
                self.store
                    .lock()
                    .map_err(|_| lock_failed())?
                    .insert(key, value);
                Ok(Value::Nothing)
            }
            "get" => {
                let key = need_text("state", "get", &args, "key")?;
                Ok(self
                    .store
                    .lock()
                    .map_err(|_| lock_failed())?
                    .get(&key)
                    .cloned()
                    .unwrap_or(Value::Nothing))
            }
            other => Err(unknown_action("state", other, &["set", "get"])),
        }
    }
}

/// What this tool can do, also used by `metagente check`.
pub fn describe() -> Vec<ActionInfo> {
    vec![
        ActionInfo::simple("set", "Remember a value", &[("key", true), ("value", true)]),
        ActionInfo::simple("get", "Recall a value", &[("key", true)]),
    ]
}
