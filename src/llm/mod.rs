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

/// 0.1.5: how a model call failed, for a model that writes its own plain sentences (a named model, `[llm.NAME]`).
/// `plain` is true when `message` is already a whole sentence for the person (and `fix` the fix), so `think` shows
/// them as they are; false keeps the 0.1.4 wrapping around `message`.
#[derive(Debug, Clone, PartialEq)]
pub struct LlmFailure {
    pub message: String,
    pub fix: Option<String>,
    pub plain: bool,
}

#[async_trait]
pub trait Llm: Send + Sync {
    async fn complete(&self, request: &LlmRequest) -> Result<LlmReply, String>;

    /// 0.1.5: like `complete`, with the failure in detail. The default calls `complete` and marks the failure as not
    /// plain, so the `[llm]` model and the scripted model behave exactly as in 0.1.4.
    async fn complete_detailed(&self, request: &LlmRequest) -> Result<LlmReply, LlmFailure> {
        self.complete(request).await.map_err(|message| LlmFailure {
            message,
            fix: None,
            plain: false,
        })
    }
}
