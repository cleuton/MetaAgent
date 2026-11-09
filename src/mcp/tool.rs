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
        // 0.1.4: a listed tool gets the token of this request, on a session of its own (D12); anything else is as before
        let listed = self
            .rt
            .config
            .auth
            .as_ref()
            .is_some_and(|p| p.forwards_to(&self.name));
        if listed && crate::runtime::auth_check::is_open_plain_http(&self.command) {
            return Err(ToolError::new(format!(
                "{} is in [auth] forward, but its address is plain http and not on this computer: the token would travel in the open.",
                self.name
            ))
            .fix("use an https:// address"));
        }
        let token = crate::runtime::auth::forward_token(ctx.auth.as_ref(), &self.name).cloned();
        let connection = match &token {
            Some(t) => self.rt.mcp.get_for_token(&self.command, &self.rt.net, t),
            None => self.connection.clone(),
        };
        let outcome = connection.call(&self.name, action, args).await;
        match outcome {
            Err(e) if e.related.iter().any(|r| r == "auth-401") => {
                if let Some(t) = &token {
                    self.rt.mcp.forget_token(&self.command, t);
                }
                // FR-034: the tool server asked for a token
                Err(if token.is_some() {
                    ToolError::new(format!(
                        "{} refused the call: it did not accept the token this agent received.",
                        self.name
                    ))
                    .fix(
                        "check that it uses the same validator, and that the token has not expired",
                    )
                } else if listed {
                    ToolError::new(format!(
                        "{} refused the call: it needs a token, and this request had none to pass on.",
                        self.name
                    ))
                    .fix("call this agent with a token")
                } else {
                    ToolError::new(format!(
                        "{} refused the call: it needs a token. If it needs the one this agent received, add {} to forward in [auth].",
                        self.name, self.name
                    ))
                    .fix(format!("add {} to forward in [auth] of metagente.toml", self.name))
                })
            }
            other => other,
        }
    }
}
