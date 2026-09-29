//! Real model providers, chosen only through configuration.

use super::{ChatMsg, Llm, LlmReply, LlmRequest, ToolCallReq};
use crate::diagnostics::Diagnostic;
use crate::runtime::config::Config;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Anthropic,
    OpenAiCompatible,
}

pub struct HttpLlm {
    kind: Kind,
    client: reqwest::Client,
    base_url: String,
    model: String,
    key_env: String,
    max_tokens: u32,
}

/// Builds the configured provider, or `None` when no model is set up.
pub fn from_config(config: &Config) -> Result<Option<Arc<dyn Llm>>, Diagnostic> {
    let Some(provider) = config.llm.provider.as_deref() else {
        return Ok(None);
    };
    let (kind, default_base, default_key, default_model) = match provider {
        "anthropic" => (
            Kind::Anthropic,
            "https://api.anthropic.com",
            "ANTHROPIC_API_KEY",
            Some("claude-sonnet-5-5"),
        ),
        "openai-compatible" | "openai" => (
            Kind::OpenAiCompatible,
            "https://api.openai.com",
            "OPENAI_API_KEY",
            None,
        ),
        other => {
            return Err(Diagnostic::new(format!(
                "I do not know the language model provider `{}`",
                other
            ))
            .fix("in metagente.toml set provider to \"anthropic\" or \"openai-compatible\""));
        }
    };
    let model = config
        .llm
        .model
        .clone()
        .or_else(|| default_model.map(String::from))
        .ok_or_else(|| {
            Diagnostic::new(format!("the provider `{}` needs a model name", provider))
                .fix("add model = \"...\" to the [llm] section of metagente.toml")
        })?;
    let client = crate::runtime::web::build_client();
    Ok(Some(Arc::new(HttpLlm {
        kind,
        client,
        base_url: config
            .llm
            .base_url
            .clone()
            .unwrap_or_else(|| default_base.to_string())
            .trim_end_matches('/')
            .to_string(),
        model,
        key_env: config
            .llm
            .api_key_env
            .clone()
            .unwrap_or_else(|| default_key.to_string()),
        max_tokens: 1024,
    })))
}

impl HttpLlm {
    fn anthropic_body(&self, req: &LlmRequest) -> Value {
        let mut messages = Vec::new();
        for m in &req.messages {
            match m {
                ChatMsg::User(t) => messages.push(json!({"role": "user", "content": t})),
                ChatMsg::Assistant { text, tool_call } => {
                    let mut blocks = Vec::new();
                    if let Some(t) = text.as_ref().filter(|t| !t.is_empty()) {
                        blocks.push(json!({"type": "text", "text": t}));
                    }
                    if let Some(c) = tool_call {
                        blocks.push(json!({"type": "tool_use", "id": c.id, "name": c.name, "input": c.args}));
                    }
                    messages.push(json!({"role": "assistant", "content": blocks}));
                }
                ChatMsg::ToolResult { id, content, .. } => messages.push(json!({
                    "role": "user",
                    "content": [{"type": "tool_result", "tool_use_id": id, "content": content}]
                })),
            }
        }
        let mut body = json!({
            "model": self.model,
            "max_tokens": self.max_tokens,
            "system": req.system,
            "messages": messages,
        });
        if !req.tools.is_empty() {
            body["tools"] = Value::Array(
                req.tools
                    .iter()
                    .map(|t| json!({"name": t.name, "description": t.description, "input_schema": t.schema}))
                    .collect(),
            );
        }
        body
    }

    fn openai_body(&self, req: &LlmRequest) -> Value {
        let mut messages = vec![json!({"role": "system", "content": req.system})];
        for m in &req.messages {
            match m {
                ChatMsg::User(t) => messages.push(json!({"role": "user", "content": t})),
                ChatMsg::Assistant { text, tool_call } => {
                    let mut msg =
                        json!({"role": "assistant", "content": text.clone().unwrap_or_default()});
                    if let Some(c) = tool_call {
                        msg["tool_calls"] = json!([{
                            "id": c.id, "type": "function",
                            "function": {"name": c.name, "arguments": c.args.to_string()}
                        }]);
                    }
                    messages.push(msg);
                }
                ChatMsg::ToolResult { id, content, .. } => {
                    messages.push(json!({"role": "tool", "tool_call_id": id, "content": content}))
                }
            }
        }
        let mut body = json!({"model": self.model, "messages": messages});
        if !req.tools.is_empty() {
            body["tools"] = Value::Array(
                req.tools
                    .iter()
                    .map(|t| json!({"type": "function", "function": {"name": t.name, "description": t.description, "parameters": t.schema}}))
                    .collect(),
            );
        }
        body
    }
}

fn parse_anthropic(response: &Value) -> Result<LlmReply, String> {
    let blocks = response
        .get("content")
        .and_then(|c| c.as_array())
        .ok_or("the answer had no content")?;
    let mut text = String::new();
    for block in blocks {
        match block.get("type").and_then(|t| t.as_str()) {
            Some("tool_use") => {
                return Ok(LlmReply::ToolCall(ToolCallReq {
                    id: block
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("call")
                        .to_string(),
                    name: block
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    args: block.get("input").cloned().unwrap_or(Value::Null),
                }));
            }
            Some("text") => text.push_str(block.get("text").and_then(|v| v.as_str()).unwrap_or("")),
            _ => {}
        }
    }
    Ok(LlmReply::Text(text))
}

fn parse_openai(response: &Value) -> Result<LlmReply, String> {
    let message = response
        .pointer("/choices/0/message")
        .ok_or("the answer had no message")?;
    if let Some(call) = message.pointer("/tool_calls/0") {
        let args_text = call
            .pointer("/function/arguments")
            .and_then(|v| v.as_str())
            .unwrap_or("{}");
        return Ok(LlmReply::ToolCall(ToolCallReq {
            id: call
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("call")
                .to_string(),
            name: call
                .pointer("/function/name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            args: serde_json::from_str(args_text).unwrap_or(Value::Null),
        }));
    }
    Ok(LlmReply::Text(
        message
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    ))
}

#[async_trait]
impl Llm for HttpLlm {
    async fn complete(&self, request: &LlmRequest) -> Result<LlmReply, String> {
        let key = std::env::var(&self.key_env)
            .map_err(|_| format!("the variable {} is not set", self.key_env))?;
        let (url, builder, body) = match self.kind {
            Kind::Anthropic => {
                let url = format!("{}/v1/messages", self.base_url);
                let b = self
                    .client
                    .post(&url)
                    .header("x-api-key", key)
                    .header("anthropic-version", "2023-06-01");
                (url, b, self.anthropic_body(request))
            }
            Kind::OpenAiCompatible => {
                let url = if self.base_url.ends_with("/v1") {
                    format!("{}/chat/completions", self.base_url)
                } else {
                    format!("{}/v1/chat/completions", self.base_url)
                };
                let b = self.client.post(&url).bearer_auth(key);
                (url, b, self.openai_body(request))
            }
        };
        let response = builder
            .json(&body)
            .send()
            .await
            .map_err(|_| format!("I could not reach {}", url))?;
        let status = response.status();
        let json: Value = response
            .json()
            .await
            .map_err(|_| format!("{} did not answer with readable data", url))?;
        if !status.is_success() {
            let why = json
                .pointer("/error/message")
                .and_then(|v| v.as_str())
                .unwrap_or("no details were given");
            return Err(format!("{} answered {}: {}", url, status.as_u16(), why));
        }
        match self.kind {
            Kind::Anthropic => parse_anthropic(&json),
            Kind::OpenAiCompatible => parse_openai(&json),
        }
    }
}
