//! An external MCP server used as a tool: `tool weather from mcp "npx -y weather-mcp"`.

use super::client::McpConnection;
use crate::runtime::Runtime;
use crate::runtime::task::CallContext;
use crate::runtime::value::Value;
use crate::tools::{ActionInfo, Args, Tool, ToolError};
use async_trait::async_trait;
use std::sync::Arc;

pub struct McpTool {
    name: String,
    connection: Arc<McpConnection>,
    // 0.1.3: for the secure rule
    rt: Arc<Runtime>,
    command: String,
    caller: String,
}

impl McpTool {
    pub fn new(rt: Arc<Runtime>, name: String, command: String, caller: String) -> McpTool {
        McpTool {
            name,
            // 0.1.3: the connection shares the network settings
            connection: rt.mcp.get(&command, &rt.net),
            rt,
            command,
            caller,
        }
    }

    /// 0.1.3: an `http://` tool server is refused for a secure tool or a secure agent, before any connection.
    fn guard(&self, inherited: bool) -> Result<(), ToolError> {
        crate::runtime::secure::decide(
            &self.rt.net.secure(),
            &crate::runtime::secure::Caller::new(&self.caller, inherited),
            crate::runtime::secure::Target::Tool(&self.name),
            &self.command,
        )
    }
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.name
    }

    async fn actions(&self) -> Result<Vec<ActionInfo>, ToolError> {
        self.guard(false)?; // 0.1.3
        self.connection.actions().await
    }

    async fn call(&self, action: &str, args: Args, ctx: &CallContext) -> Result<Value, ToolError> {
        self.guard(ctx.secure)?; // 0.1.3
        self.connection.call(&self.name, action, args).await
    }
}
