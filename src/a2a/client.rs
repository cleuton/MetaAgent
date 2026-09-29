//! Sends tasks to remote A2A agents: `remote Bob at "http://host:8080"`, called as `Bob.ask city: "x"`.

use super::{PROTOCOL_VERSION, new_id};
use crate::lang::RemoteDecl;
use crate::runtime::Runtime;
use crate::runtime::task::CallContext;
use crate::runtime::value::Value;
use crate::tools::{ActionInfo, Args, Tool, ToolError, closest_name};
use async_trait::async_trait;
use serde_json::{Value as Json, json};
use std::sync::Arc;
use tokio::sync::OnceCell;

struct Card {
    endpoint: String,
    skills: Vec<(String, String)>,
}

pub struct RemoteTool {
    decl: RemoteDecl,
    rt: Arc<Runtime>,
    card: OnceCell<Card>,
}

impl RemoteTool {
    pub fn new(rt: Arc<Runtime>, decl: RemoteDecl) -> RemoteTool {
        RemoteTool {
            decl,
            rt,
            card: OnceCell::new(),
        }
    }

    fn unreachable(&self, url: &str, e: &reqwest::Error) -> ToolError {
        let why = if e.is_connect() {
            "the connection failed"
        } else if e.is_timeout() {
            "it took too long"
        } else {
            "the request did not complete"
        };
        ToolError::new(format!(
            "I could not reach {} (remote agent {}): {}",
            url, self.decl.name, why
        ))
        .fix("check the address, and that the other agent is being served (metagente serve)")
    }

    async fn card(&self) -> Result<&Card, ToolError> {
        self.card
            .get_or_try_init(|| async {
                let base = self.decl.url.trim_end_matches('/');
                let url = format!("{}/.well-known/agent-card.json", base);
                let response = self.rt.http.get(&url).send().await.map_err(|e| self.unreachable(&url, &e))?;
                if !response.status().is_success() {
                    return Err(ToolError::new(format!(
                        "{} answered {} when I asked for the agent card of {}",
                        url,
                        response.status().as_u16(),
                        self.decl.name
                    ))
                    .fix("the address should be where the agent is served, for example http://127.0.0.1:8080"));
                }
                let card: Json = response.json().await.map_err(|_| {
                    ToolError::new(format!("{} did not give me a readable agent card", url))
                        .fix("check that the address belongs to an A2A agent")
                })?;
                let interfaces = card.get("supportedInterfaces").and_then(|i| i.as_array()).cloned().unwrap_or_default();
                let chosen = interfaces
                    .iter()
                    .find(|i| i.get("protocolBinding").and_then(|b| b.as_str()) == Some("JSONRPC"))
                    .ok_or_else(|| {
                        ToolError::new(format!("agent {} does not offer the JSON-RPC way of talking, which is the one Metagente uses", self.decl.name))
                    })?;
                if let Some(version) = chosen.get("protocolVersion").and_then(|v| v.as_str()) {
                    if !(version == "1" || version.starts_with("1.")) {
                        return Err(ToolError::new(format!(
                            "agent {} speaks A2A {}, and Metagente speaks A2A {}",
                            self.decl.name, version, PROTOCOL_VERSION
                        ))
                        .fix("ask the other side to offer A2A 1.0"));
                    }
                }
                let endpoint = chosen
                    .get("url")
                    .and_then(|u| u.as_str())
                    .ok_or_else(|| ToolError::new(format!("the agent card of {} has no address to send tasks to", self.decl.name)))?
                    .to_string();
                let skills = card
                    .get("skills")
                    .and_then(|s| s.as_array())
                    .map(|list| {
                        list.iter()
                            .filter_map(|s| {
                                let id = s.get("id")?.as_str()?.to_string();
                                let description = s.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string();
                                Some((id, description))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Ok(Card { endpoint, skills })
            })
            .await
    }

    async fn rpc(&self, endpoint: &str, method: &str, params: Json) -> Result<Json, ToolError> {
        let body =
            json!({"jsonrpc": "2.0", "id": new_id("req"), "method": method, "params": params});
        let response = self
            .rt
            .http
            .post(endpoint)
            .header("A2A-Version", PROTOCOL_VERSION)
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .send()
            .await
            .map_err(|e| self.unreachable(endpoint, &e))?;
        let answer: Json = response.json().await.map_err(|_| {
            ToolError::new(format!("{} did not answer with readable data", endpoint))
                .fix("check that the address belongs to an A2A agent")
        })?;
        if let Some(error) = answer.get("error") {
            let text = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("no details were given");
            return Err(ToolError::new(format!(
                "agent {} refused the request: {}",
                self.decl.name, text
            ))
            .fix("check the message name and values; the agent's card lists what it handles"));
        }
        answer.get("result").cloned().ok_or_else(|| {
            ToolError::new(format!(
                "agent {} answered without a result",
                self.decl.name
            ))
        })
    }
}

fn text_of_message(message: &Json) -> String {
    message
        .get("parts")
        .and_then(|p| p.as_array())
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn value_of_task(task: &Json) -> Value {
    let parts: Vec<Json> = task
        .get("artifacts")
        .and_then(|a| a.as_array())
        .and_then(|a| a.first())
        .and_then(|a| a.get("parts"))
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default();
    if let Some(data) = parts.iter().find_map(|p| p.get("data")) {
        return Value::from_json(data);
    }
    let texts: Vec<&str> = parts
        .iter()
        .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
        .collect();
    if texts.is_empty() {
        Value::Nothing
    } else {
        Value::Text(texts.join("\n"))
    }
}

#[async_trait]
impl Tool for RemoteTool {
    fn name(&self) -> &str {
        &self.decl.name
    }

    async fn actions(&self) -> Result<Vec<ActionInfo>, ToolError> {
        let card = self.card().await?;
        Ok(card
            .skills
            .iter()
            .map(|(id, d)| ActionInfo::simple(id, d, &[]))
            .collect())
    }

    async fn call(&self, action: &str, args: Args, _ctx: &CallContext) -> Result<Value, ToolError> {
        let card = self.card().await?;
        if !card.skills.iter().any(|(id, _)| id == action) {
            let ids: Vec<&str> = card.skills.iter().map(|(id, _)| id.as_str()).collect();
            let fix = match closest_name(action, ids.iter().copied()) {
                Some(s) => format!(
                    "did you mean `{}.{}`? (it handles: {})",
                    self.decl.name,
                    s,
                    ids.join(", ")
                ),
                None => format!("it handles: {}", ids.join(", ")),
            };
            return Err(ToolError::new(format!(
                "agent {} does not handle `{}`",
                self.decl.name, action
            ))
            .fix(fix));
        }
        let arguments: serde_json::Map<String, Json> =
            args.iter().map(|(k, v)| (k.clone(), v.to_json())).collect();
        let params = json!({
            "message": {
                "messageId": new_id("msg"),
                "role": "ROLE_USER",
                "parts": [{"data": {"skill": action, "arguments": arguments}, "mediaType": "application/json"}],
                "metadata": {"skill": action},
            },
            "configuration": {"returnImmediately": false},
        });
        let result = self.rpc(&card.endpoint, "SendMessage", params).await?;
        if let Some(message) = result.get("message") {
            return Ok(Value::Text(text_of_message(message)));
        }
        let mut task = result.get("task").cloned().ok_or_else(|| {
            ToolError::new(format!(
                "agent {} answered with neither a task nor a message",
                self.decl.name
            ))
        })?;
        loop {
            let state = task
                .pointer("/status/state")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            match state.as_str() {
                "TASK_STATE_COMPLETED" => return Ok(value_of_task(&task)),
                "TASK_STATE_FAILED" | "TASK_STATE_REJECTED" | "TASK_STATE_CANCELED" => {
                    let why = task
                        .pointer("/status/message")
                        .map(text_of_message)
                        .unwrap_or_default();
                    let mut error = ToolError::new(format!(
                        "agent {} could not answer `{}`",
                        self.decl.name, action
                    ));
                    error =
                        error.related(why.lines().map(|l| format!("  {}", l)).collect::<Vec<_>>());
                    return Err(error);
                }
                "TASK_STATE_INPUT_REQUIRED" | "TASK_STATE_AUTH_REQUIRED" => {
                    return Err(ToolError::new(format!(
                        "agent {} needs more from you before it can answer, which Metagente does not support yet",
                        self.decl.name
                    )));
                }
                _ => {
                    let id = task
                        .get("id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("")
                        .to_string();
                    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                    let polled = self
                        .rpc(&card.endpoint, "GetTask", json!({"id": id}))
                        .await?;
                    task = polled;
                }
            }
        }
    }
}
