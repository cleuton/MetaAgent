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
}

impl McpTool {
    pub fn new(rt: Arc<Runtime>, name: String, command: String) -> McpTool {
        McpTool {
            name,
            connection: rt.mcp.get(&command),
        }
    }
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.name
    }

    async fn actions(&self) -> Result<Vec<ActionInfo>, ToolError> {
        self.connection.actions().await
    }

    async fn call(&self, action: &str, args: Args, _ctx: &CallContext) -> Result<Value, ToolError> {
        self.connection.call(&self.name, action, args).await
    }
}
