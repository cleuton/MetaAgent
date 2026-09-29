//! The language model behind `think`. Providers are chosen in configuration, never in agent code.

pub mod fake;
pub mod providers;

use async_trait::async_trait;

/// A tool the model may ask for. The name has the form `target__action`.
#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON schema of the arguments.
    pub schema: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCallReq {
    pub id: String,
    pub name: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChatMsg {
    User(String),
    Assistant {
        text: Option<String>,
        tool_call: Option<ToolCallReq>,
    },
    ToolResult {
        id: String,
        name: String,
        content: String,
    },
}

#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub system: String,
    pub messages: Vec<ChatMsg>,
    pub tools: Vec<ToolSpec>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LlmReply {
    Text(String),
    ToolCall(ToolCallReq),
}

#[async_trait]
pub trait Llm: Send + Sync {
    async fn complete(&self, request: &LlmRequest) -> Result<LlmReply, String>;
}
