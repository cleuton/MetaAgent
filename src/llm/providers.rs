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
    net: Arc<crate::runtime::net::ClientSet>, // 0.1.3
    base_url: String,
    model: String,
    key_env: String,
    max_tokens: u32,
    // 0.1.5: set for a named model ([llm.NAME]); None for the model of [llm], which is built and behaves as in 0.1.4
    named: Option<Named>,
    // 0.1.5: set after OpenAI refused tools together with its own reasoning effort (see `exchange`)
    reasoning_none: std::sync::atomic::AtomicBool,
}

/// 0.1.5: what only a named model has.
struct Named {
    nickname: String,
    /// `model_env`: read at the first use, then kept (D3).
    model_env: Option<String>,
    model_cache: std::sync::Mutex<Option<String>>,
    key_env_line: usize,
    model_env_line: usize,
}

/// 0.1.5: the official addresses of the two services. A named model never reads an address from anything.
pub const ANTHROPIC_ADDRESS: &str = "https://api.anthropic.com";
pub const OPENAI_ADDRESS: &str = "https://api.openai.com";

/// 0.1.3: the address the model is reached at (`base_url` or the provider's own), or `None` when no model is set up.
pub fn model_address(config: &Config) -> Option<String> {
    let provider = config.llm.provider.as_deref()?;
    let default_base = match provider {
        "anthropic" => "https://api.anthropic.com",
        "openai-compatible" | "openai" => "https://api.openai.com",
        _ => return None,
    };
    Some(
        config
            .llm
            .base_url
            .clone()
            .unwrap_or_else(|| default_base.to_string()),
    )
}

/// Builds the configured provider, or `None` when no model is set up.
pub fn from_config(config: &Config) -> Result<Option<Arc<dyn Llm>>, Diagnostic> {
    // 0.1.3: own network clients, made from the same settings
    from_config_with(
        config,
        Arc::new(crate::runtime::net::ClientSet::for_config(config)),
    )
}

/// 0.1.3: like `from_config`, sharing the network clients (and so a changed `[network]`) with the runtime.
pub fn from_config_with(
    config: &Config,
    net: Arc<crate::runtime::net::ClientSet>,
) -> Result<Option<Arc<dyn Llm>>, Diagnostic> {
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
    Ok(Some(Arc::new(HttpLlm {
        kind,
        net,
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
        named: None, // 0.1.5: the model of [llm] is not a named model
        reasoning_none: Default::default(),
    })))
}

/// 0.1.5: one model object for every `[llm.NAME]`, built at start next to the default model (F3). The request code is
/// the one of `[llm]`; only the address (always the official one), the model name and the messages differ.
pub fn named_from_config(
    config: &Config,
    net: Arc<crate::runtime::net::ClientSet>,
) -> std::collections::HashMap<String, Arc<dyn Llm>> {
    use crate::runtime::models::{ModelName, ProviderKind};
    let mut out: std::collections::HashMap<String, Arc<dyn Llm>> = Default::default();
    for m in config.models.iter() {
        let (kind, base) = match m.provider {
            ProviderKind::Anthropic => (Kind::Anthropic, ANTHROPIC_ADDRESS),
            ProviderKind::OpenAi => (Kind::OpenAiCompatible, OPENAI_ADDRESS),
        };
        let (model, model_env) = match &m.model {
            ModelName::Literal(name) => (name.clone(), None),
            ModelName::FromEnv(var) => (String::new(), Some(var.clone())),
        };
        out.insert(
            m.nickname.clone(),
            Arc::new(HttpLlm {
                kind,
                net: net.clone(),
                base_url: base.to_string(),
                model,
                key_env: m.key_env.clone(),
                max_tokens: 1024,
                named: Some(Named {
                    nickname: m.nickname.clone(),
                    model_env,
                    model_cache: std::sync::Mutex::new(None),
                    key_env_line: m.key_env_line,
                    model_env_line: m.model_env_line,
                }),
                reasoning_none: Default::default(),
            }),
        );
    }
    out
}

impl HttpLlm {
    fn anthropic_body(&self, req: &LlmRequest, model: &str) -> Value {
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
            "model": model,
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

    fn openai_body(&self, req: &LlmRequest, model: &str) -> Value {
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
        let mut body = json!({"model": model, "messages": messages});
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

impl HttpLlm {
    /// 0.1.5: the request, built in one place for `[llm]` and for a named model: the address, the key header and the
    /// body. (This is the code `complete` had inline in 0.1.4; nothing in it changed.)
    fn prepare(
        &self,
        client: &reqwest::Client,
        key: String,
        model: &str,
        request: &LlmRequest,
    ) -> (String, reqwest::RequestBuilder, Value) {
        match self.kind {
            Kind::Anthropic => {
                let url = format!("{}/v1/messages", self.base_url);
                let b = client
                    .post(&url)
                    .header("x-api-key", key)
                    .header("anthropic-version", "2023-06-01");
                (url, b, self.anthropic_body(request, model))
            }
            Kind::OpenAiCompatible => {
                let url = if self.base_url.ends_with("/v1") {
                    format!("{}/chat/completions", self.base_url)
                } else {
                    format!("{}/v1/chat/completions", self.base_url)
                };
                let b = client.post(&url).bearer_auth(key);
                let mut body = self.openai_body(request, model);
                // 0.1.5: only after OpenAI refused the request without it (see `exchange`); never in the first request
                if self
                    .reasoning_none
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
                    body["reasoning_effort"] = json!("none");
                }
                (url, b, body)
            }
        }
    }

    /// 0.1.5: one call, and at most one more. The first request is exactly what 0.1.4 sent. Some OpenAI models (the
    /// ones that reason by default) answer a request that has tools with a 400 that says tools cannot be used with their
    /// reasoning effort and that `reasoning_effort` must be `none`. Only then the request is sent once more with
    /// `reasoning_effort: "none"`, and the next calls of this model send it at once. Any other answer, and every other
    /// provider, is left alone. The body of the refusal is read only to decide this; it is never shown.
    async fn exchange(
        &self,
        client: &reqwest::Client,
        key: &str,
        model: &str,
        request: &LlmRequest,
    ) -> Result<(String, reqwest::StatusCode, Result<Value, ()>), (String, reqwest::Error)> {
        let mut attempt = 0;
        loop {
            let (url, builder, body) = self.prepare(client, key.to_string(), model, request);
            let response = builder
                .json(&body)
                .send()
                .await
                .map_err(|e| (url.clone(), e))?;
            let status = response.status();
            let json: Result<Value, ()> = response.json().await.map_err(|_| ());
            attempt += 1;
            let refused_reasoning = attempt == 1
                && self.kind == Kind::OpenAiCompatible
                && status.as_u16() == 400
                && !self
                    .reasoning_none
                    .load(std::sync::atomic::Ordering::Relaxed)
                && json
                    .as_ref()
                    .ok()
                    .and_then(|j| j.pointer("/error/message"))
                    .and_then(|m| m.as_str())
                    .is_some_and(|m| m.contains("reasoning_effort") && m.contains("none"));
            if refused_reasoning {
                self.reasoning_none
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                continue;
            }
            return Ok((url, status, json));
        }
    }

    /// 0.1.5: a call to a named model. Every failure is a whole plain sentence written here; nothing that came from the
    /// service, the library or a proxy is repeated (it can hold the model name, a masked key or an address) (FR-013).
    async fn call_named(&self, request: &LlmRequest) -> Result<LlmReply, crate::llm::LlmFailure> {
        use crate::runtime::models::variable_phrase;
        let named = self
            .named
            .as_ref()
            .expect("call_named is only used for a named model");
        let nick = named.nickname.as_str();
        let service = match self.kind {
            Kind::Anthropic => "anthropic",
            Kind::OpenAiCompatible => "openai",
        };
        let fail = |message: String, fix: &str| crate::llm::LlmFailure {
            message,
            fix: Some(fix.to_string()),
            plain: true,
        };
        let key = match std::env::var(&self.key_env) {
            Ok(k) if !k.trim().is_empty() => k,
            _ => {
                return Err(fail(
                    format!(
                        "The model {} needs {}, and it is not set.",
                        nick,
                        variable_phrase(&self.key_env, named.key_env_line)
                    ),
                    "set the variable",
                ));
            }
        };
        let model = match &named.model_env {
            None => self.model.clone(),
            Some(var) => {
                let cached = named.model_cache.lock().ok().and_then(|c| c.clone());
                match cached {
                    Some(m) => m,
                    None => match std::env::var(var) {
                        Ok(m) if !m.trim().is_empty() => {
                            if let Ok(mut c) = named.model_cache.lock() {
                                *c = Some(m.clone());
                            }
                            m
                        }
                        _ => {
                            return Err(fail(
                                format!(
                                    "The model {} needs {}, and it is empty or not set.",
                                    nick,
                                    variable_phrase(var, named.model_env_line)
                                ),
                                "set the variable, or change model_env",
                            ));
                        }
                    },
                }
            }
        };
        let client = self.net.model().map_err(|_| {
            fail(
                format!(
                    "The model {} ({}) could not be reached: the network client could not be set up.",
                    nick, service
                ),
                "check the [network] section of metagente.toml",
            )
        })?;
        let (_url, status, json) = match self.exchange(&client, &key, &model, request).await {
            Ok(r) => r,
            Err((url, e)) => {
                use crate::runtime::net::FailureKind;
                let (kind, fix) = match self.net.failure_kind(&e, &url) {
                    FailureKind::Timeout => (
                        "timeout",
                        "check your connection and the settings in [network]",
                    ),
                    FailureKind::NoConnection => (
                        "no connection",
                        "check your connection and the settings in [network]",
                    ),
                    FailureKind::Certificate => {
                        ("certificate", "add the authority with ca_file in [network]")
                    }
                    FailureKind::Proxy => ("proxy", "check the [network.proxy] settings"),
                };
                return Err(fail(
                    format!(
                        "The model {} ({}) could not be reached: {}.",
                        nick, service, kind
                    ),
                    fix,
                ));
            }
        };
        if !status.is_success() {
            let code = status.as_u16();
            let fix = match code {
                401 | 403 => "check the key and that it belongs to this provider",
                400 | 404 => "check the model name and that this model accepts tools",
                429 => "wait and try again, or check the plan of the service",
                500..=599 => "the service has a problem, try again later",
                _ => "check the settings of the model",
            };
            return Err(fail(
                format!(
                    "The model {} ({}) answered with status {}.",
                    nick, service, code
                ),
                fix,
            ));
        }
        let unreadable = || {
            fail(
                format!(
                    "The model {} ({}) answered with something I could not read.",
                    nick, service
                ),
                "try again later",
            )
        };
        let json: Value = json.map_err(|_| unreadable())?;
        match self.kind {
            Kind::Anthropic => parse_anthropic(&json),
            Kind::OpenAiCompatible => parse_openai(&json),
        }
        .map_err(|_| unreadable())
    }
}

#[async_trait]
impl Llm for HttpLlm {
    async fn complete_detailed(
        &self,
        request: &LlmRequest,
    ) -> Result<LlmReply, crate::llm::LlmFailure> {
        // 0.1.5: a named model has plain sentences of its own; the model of [llm] keeps the 0.1.4 ones
        if self.named.is_some() {
            return self.call_named(request).await;
        }
        self.complete(request)
            .await
            .map_err(|message| crate::llm::LlmFailure {
                message,
                fix: None,
                plain: false,
            })
    }

    async fn complete(&self, request: &LlmRequest) -> Result<LlmReply, String> {
        if self.named.is_some() {
            return self.call_named(request).await.map_err(|f| match f.fix {
                Some(fix) => format!("{} ({})", f.message, fix),
                None => f.message,
            });
        }
        let key = std::env::var(&self.key_env)
            .map_err(|_| format!("the variable {} is not set", self.key_env))?;
        // 0.1.3: the model has its own client, with its own certificate switch
        let client = self.net.model().map_err(|e| e.to_text())?;
        let (url, status, json) = match self.exchange(&client, &key, &self.model, request).await {
            Ok(r) => r,
            Err((url, e)) => {
                // 0.1.3: certificate and proxy problems get their own plain sentences
                let said = self
                    .net
                    .explain(
                        &e,
                        &url,
                        "language model",
                        "check the address of the model server and your connection",
                        crate::runtime::net::Purpose::Model,
                    )
                    .await;
                return Err(match said.fix {
                    Some(f) => format!("{} ({})", said.message, f),
                    None => said.message,
                });
            }
        };
        let json: Value = json.map_err(|_| format!("{} did not answer with readable data", url))?;
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
