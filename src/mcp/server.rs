//! MCP server: every `accepts` entry of the served agents becomes a tool named `Agent.message`.

use crate::runtime::auth::RequestAuth; // 0.1.4
use crate::runtime::interpreter::Agent;
use crate::runtime::serve::Served;
use crate::runtime::task::{CallContext, new_task_id};
use crate::runtime::value::Value;
use crate::tools::Args;
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use std::sync::Arc;

#[derive(Clone)]
pub struct AgentsMcp {
    pub served: Arc<Served>,
}

impl AgentsMcp {
    pub fn new(served: Arc<Served>) -> AgentsMcp {
        AgentsMcp { served }
    }

    pub fn tools(&self) -> Vec<Tool> {
        let mut tools = Vec::new();
        for def in self.served.agents() {
            for accept in &def.accepts {
                let mut props = serde_json::Map::new();
                for p in &accept.params {
                    props.insert(
                        p.clone(),
                        serde_json::json!({"type": "string", "description": format!("the {}", p)}),
                    );
                }
                let schema = serde_json::json!({"type": "object", "properties": props, "required": accept.params});
                let description = accept.description.clone().unwrap_or_else(|| {
                    format!(
                        "{}: {}",
                        def.goal.as_ref().map(|g| g.0.as_str()).unwrap_or(&def.name),
                        accept.message
                    )
                });
                tools.push(Tool::new(
                    format!("{}.{}", def.name, accept.message),
                    description,
                    Arc::new(schema.as_object().cloned().unwrap_or_default()),
                ));
            }
        }
        tools
    }
}

impl ServerHandler for AgentsMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("Metagente agents. Each tool is a message an agent accepts.")
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(self.tools()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let name = request.name.to_string();
        let known: Vec<String> = self.tools().iter().map(|t| t.name.to_string()).collect();
        let Some((agent_name, message)) = name.split_once('.') else {
            return Err(McpError::invalid_params(
                format!(
                    "Not supported: `{}` is not a tool here; the tools are: {}",
                    name,
                    known.join(", ")
                ),
                None,
            ));
        };
        let Some(def) = self.served.find(agent_name) else {
            return Err(McpError::invalid_params(
                format!(
                    "Not supported: there is no agent called {}; the tools are: {}",
                    agent_name,
                    known.join(", ")
                ),
                None,
            ));
        };
        if def.accept(message).is_none() {
            return Err(McpError::invalid_params(
                format!(
                    "Not supported: {} does not accept `{}`; the tools are: {}",
                    agent_name,
                    message,
                    known.join(", ")
                ),
                None,
            ));
        }
        // 0.1.4: what the request carried, put there by the door in front of /mcp (stdio has no request: no token)
        let auth = self.served.rt.config.auth.clone().map(|policy| {
            let mark = context
                .extensions
                .get::<axum::http::request::Parts>()
                .and_then(|parts| parts.extensions.get::<super::gate::McpAuthMark>().cloned());
            match mark {
                Some(m) => RequestAuth::network(policy, m.token, m.accepted),
                None => RequestAuth::network(policy, None, false),
            }
        });
        // the second check: a protected tool never runs without an accepted token, even if the door was skipped
        if let Some(a) = &auth {
            if a.policy.is_protected(agent_name, message) && !a.accepted {
                return Err(McpError::invalid_params(
                    format!("{} needs a token, and this request had none.", name),
                    None,
                ));
            }
        }
        let args: Args = request
            .arguments
            .unwrap_or_default()
            .iter()
            .map(|(k, v)| (k.clone(), Value::from_json(v)))
            .collect();
        let outcome = match Agent::new(self.served.rt.clone(), def) {
            Ok(agent) => {
                let mut ctx = CallContext::new(new_task_id());
                ctx.auth = auth; // 0.1.4: the token travels with this request only
                agent.handle(message, args, &ctx).await
            }
            Err(d) => Err(d),
        };
        let result = match outcome {
            Ok(Value::Text(t)) => CallToolResult::success(vec![ContentBlock::text(t)]),
            Ok(Value::Nothing) => CallToolResult::success(vec![]),
            Ok(other) => {
                CallToolResult::success(vec![ContentBlock::text(other.to_json().to_string())])
            }
            Err(d) => CallToolResult::error(vec![ContentBlock::text(d.render())]),
        };
        Ok(result.into())
    }
}
