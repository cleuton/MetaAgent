//! Receives A2A tasks: serves Agent Cards and answers `SendMessage` and `GetTask`.

use super::card::agent_card_for; // 0.1.4
use super::message::interpret;
use super::store::TaskStore;
use super::{new_id, now_text};
use crate::lang::AgentDef;
use crate::runtime::auth::{
    Fingerprint, Refusal, RequestAuth, Verdict, judge, parse_authorization,
};
use crate::runtime::interpreter::Agent;
use crate::runtime::serve::Served;
use crate::runtime::task::CallContext;
use crate::runtime::value::Value;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde_json::{Value as Json, json};
use std::sync::Arc;

pub struct A2aState {
    pub served: Arc<Served>,
    pub store: TaskStore,
    /// Where clients reach this server, for the Agent Card, e.g. `http://127.0.0.1:8080`.
    pub base: String,
}

pub fn router(state: Arc<A2aState>) -> Router {
    Router::new()
        .route("/.well-known/agent-card.json", get(default_card))
        .route(
            "/agents/{name}/.well-known/agent-card.json",
            get(named_card),
        )
        .route("/a2a", post(rpc_default))
        .route("/agents/{name}/a2a", post(rpc_named))
        .with_state(state)
}

fn json_response(status: StatusCode, body: Json) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "application/json")],
        body.to_string(),
    )
        .into_response()
}

/// 0.1.3: the host (and port) the caller used. HTTP/1.1 sends a `Host` header; HTTP/2 (curl, browsers) sends the
/// authority in the request address instead and no `Host` header at all.
fn host_of(headers: &HeaderMap, uri: &axum::http::Uri) -> Option<String> {
    headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .map(String::from)
        .or_else(|| uri.authority().map(|a| a.to_string()))
}

fn base_of(state: &A2aState, headers: &HeaderMap, uri: &axum::http::Uri) -> String {
    // 0.1.3: a listener that answers https always advertises https, whatever a header says
    if state.base.starts_with("https://") {
        return match host_of(headers, uri) {
            Some(h) => format!("https://{}", h),
            None => state.base.clone(),
        };
    }
    // 0.1.3: behind a reverse proxy that ends https, the card must advertise https, or clients would refuse to follow it
    let scheme = match headers
        .get("x-forwarded-proto")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.trim().to_ascii_lowercase())
    {
        Some(s) if s == "https" => "https",
        _ => "http",
    };
    host_of(headers, uri)
        .map(|h| format!("{}://{}", scheme, h))
        .unwrap_or_else(|| state.base.clone())
}

async fn default_card(
    State(state): State<Arc<A2aState>>,
    headers: HeaderMap,
    uri: axum::http::Uri,
) -> Response {
    let base = base_of(&state, &headers, &uri);
    match state.served.default() {
        Some(def) => json_response(
            StatusCode::OK,
            agent_card_for(
                &def,
                &format!("{}/a2a", base),
                state.served.rt.config.auth.as_deref(),
            ),
        ),
        None => json_response(
            StatusCode::NOT_FOUND,
            json!({"error": "this server has no agents"}),
        ),
    }
}

async fn named_card(
    State(state): State<Arc<A2aState>>,
    headers: HeaderMap,
    uri: axum::http::Uri,
    Path(name): Path<String>,
) -> Response {
    let base = base_of(&state, &headers, &uri);
    match state.served.find(&name) {
        Some(def) => json_response(
            StatusCode::OK,
            agent_card_for(
                &def,
                &format!("{}/agents/{}/a2a", base, name),
                state.served.rt.config.auth.as_deref(),
            ),
        ),
        None => json_response(
            StatusCode::NOT_FOUND,
            json!({"error": format!("there is no agent called {}", name)}),
        ),
    }
}

async fn rpc_default(
    State(state): State<Arc<A2aState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    handle_rpc(state, None, headers, body).await
}

async fn rpc_named(
    State(state): State<Arc<A2aState>>,
    Path(name): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    handle_rpc(state, Some(name), headers, body).await
}

struct RpcError {
    code: i64,
    message: String,
    reason: &'static str,
    /// 0.1.4: a refusal for a missing or bad token is a real HTTP 401 or 503, decided before the JSON-RPC answer
    refusal: Option<Refusal>,
}

impl RpcError {
    fn new(code: i64, reason: &'static str, message: impl Into<String>) -> RpcError {
        RpcError {
            code,
            message: message.into(),
            reason,
            refusal: None, // 0.1.4
        }
    }
    /// 0.1.4: print the line for the operator, and answer with the real HTTP status.
    fn refused(refusal: Refusal) -> RpcError {
        eprintln!("{}", refusal.log);
        RpcError {
            code: -32000,
            message: refusal.message.clone(),
            reason: refusal.reason,
            refusal: Some(refusal),
        }
    }
    fn to_json(&self, id: &Json) -> Json {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": self.code,
                "message": self.message,
                "data": [{
                    "@type": "type.googleapis.com/google.rpc.ErrorInfo",
                    "reason": self.reason,
                    "domain": "a2a-protocol.org",
                    "metadata": {"timestamp": now_text()}
                }]
            }
        })
    }
}

async fn handle_rpc(
    state: Arc<A2aState>,
    agent: Option<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request: Json = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => {
            let e = RpcError::new(
                -32700,
                "JSON_PARSE_ERROR",
                "Invalid JSON payload: the request is not valid JSON",
            );
            return json_response(StatusCode::OK, e.to_json(&Json::Null));
        }
    };
    let id = request.get("id").cloned().unwrap_or(Json::Null);
    let outcome = dispatch(&state, agent, &headers, &request).await;
    match outcome {
        Ok(result) => json_response(
            StatusCode::OK,
            json!({"jsonrpc": "2.0", "id": id, "result": result}),
        ),
        Err(e) => match &e.refusal {
            // 0.1.4: 401 asks for a bearer token; 503 means the validator is broken
            Some(r) => {
                let mut response = json_response(
                    StatusCode::from_u16(r.status).unwrap_or(StatusCode::UNAUTHORIZED),
                    crate::runtime::auth::refusal_json(&id, r),
                );
                if r.status == 401 {
                    response.headers_mut().insert(
                        header::WWW_AUTHENTICATE,
                        header::HeaderValue::from_static("Bearer"),
                    );
                }
                response
            }
            None => json_response(StatusCode::OK, e.to_json(&id)),
        },
    }
}

async fn dispatch(
    state: &Arc<A2aState>,
    agent: Option<String>,
    headers: &HeaderMap,
    request: &Json,
) -> Result<Json, RpcError> {
    if request.get("jsonrpc").and_then(|v| v.as_str()) != Some("2.0") {
        return Err(RpcError::new(
            -32600,
            "INVALID_REQUEST",
            "Request payload validation error: \"jsonrpc\" must be \"2.0\"",
        ));
    }
    if let Some(v) = headers.get("a2a-version").and_then(|v| v.to_str().ok()) {
        if !(v == "1" || v.starts_with("1.")) {
            return Err(RpcError::new(
                -32009,
                "VERSION_NOT_SUPPORTED",
                format!(
                    "Version not supported: this server speaks A2A 1.0, but the request asked for {}",
                    v
                ),
            ));
        }
    }
    let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let params = request.get("params").cloned().unwrap_or(Json::Null);
    match method {
        "SendMessage" => send_message(state, agent, params, headers).await, // 0.1.4: headers, for the token
        "GetTask" => get_task(state, params, headers).await,
        other => Err(RpcError::new(
            -32601,
            "METHOD_NOT_FOUND",
            format!(
                "Method not found: {}. This server supports SendMessage and GetTask",
                other
            ),
        )),
    }
}

async fn get_task(
    state: &Arc<A2aState>,
    params: Json,
    headers: &HeaderMap,
) -> Result<Json, RpcError> {
    let id = params.get("id").and_then(|v| v.as_str()).ok_or_else(|| {
        RpcError::new(
            -32602,
            "INVALID_PARAMS",
            "Invalid parameters: \"id\" is required",
        )
    })?;
    let task = state.store.get(id).ok_or_else(|| {
        RpcError::new(
            -32001,
            "TASK_NOT_FOUND",
            format!(
                "Task not found: {} (finished tasks are kept for 10 minutes)",
                id
            ),
        )
    })?;
    // 0.1.4: a task that came from a protected message is read only with the token that started it (D14)
    if let (Some(owner), Some(policy)) =
        (state.store.owner(id), state.served.rt.config.auth.clone())
    {
        let (verdict, token) = judge(&policy, parse_authorization(headers)).await;
        let same = token.as_ref().map(Fingerprint::of).as_ref() == Some(&owner);
        match verdict {
            Verdict::Accepted if same => {}
            Verdict::Unavailable(_) => {
                if let Some(r) = verdict.refusal("this task", &policy) {
                    return Err(RpcError::refused(r));
                }
            }
            other => {
                let why = match other {
                    Verdict::Accepted => {
                        "a different token than the one that started the task".to_string()
                    }
                    Verdict::NoToken => "no token".to_string(),
                    Verdict::Malformed => "malformed Authorization header".to_string(),
                    _ => "token not accepted".to_string(),
                };
                return Err(RpcError::refused(Refusal {
                    status: 401,
                    reason: "UNAUTHENTICATED",
                    message:
                        "This task needs the token that started it. Ask again with that token."
                            .to_string(),
                    log: format!("Refused GetTask for a protected task: {}.", why),
                }));
            }
        }
    }
    Ok(task)
}

fn part_for(value: &Value) -> Json {
    match value {
        Value::Text(t) => json!({"text": t}),
        other => json!({"data": other.to_json(), "mediaType": "application/json"}),
    }
}

fn task_json(
    id: &str,
    context: &str,
    state: &str,
    note: Option<String>,
    reply: Option<&Value>,
) -> Json {
    let mut status = json!({"state": state, "timestamp": now_text()});
    if let Some(text) = note {
        status["message"] = json!({
            "messageId": new_id("msg"),
            "role": "ROLE_AGENT",
            "parts": [{"text": text}],
        });
    }
    let mut task = json!({"id": id, "contextId": context, "status": status});
    if let Some(value) = reply {
        task["artifacts"] = json!([{ "artifactId": new_id("artifact"), "name": "reply", "parts": [part_for(value)] }]);
    }
    task
}

/// 0.1.4: judges the token of a request once the message is known (D8, FR-015). A public message is not judged; a
/// token it carried is still kept, because its handler may pass it on to a listed destination.
async fn judge_request(
    state: &Arc<A2aState>,
    agent: &str,
    message: &str,
    headers: &HeaderMap,
) -> Result<Option<std::sync::Arc<RequestAuth>>, RpcError> {
    let Some(policy) = state.served.rt.config.auth.clone() else {
        return Ok(None);
    };
    let parsed = parse_authorization(headers);
    if !policy.is_protected(agent, message) {
        let token = parsed.ok().flatten();
        return Ok(Some(RequestAuth::network(policy, token, false)));
    }
    let (verdict, token) = judge(&policy, parsed).await;
    match verdict.refusal(&format!("{}.{}", agent, message), &policy) {
        None => Ok(Some(RequestAuth::network(policy, token, true))),
        Some(r) => Err(RpcError::refused(r)),
    }
}

async fn send_message(
    state: &Arc<A2aState>,
    agent: Option<String>,
    params: Json,
    headers: &HeaderMap, // 0.1.4
) -> Result<Json, RpcError> {
    let message = params
        .get("message")
        .filter(|m| m.is_object())
        .ok_or_else(|| {
            RpcError::new(
                -32602,
                "INVALID_PARAMS",
                "Invalid parameters: \"message\" is required",
            )
        })?;
    if message
        .get("parts")
        .and_then(|p| p.as_array())
        .map(|p| p.is_empty())
        .unwrap_or(true)
    {
        return Err(RpcError::new(
            -32602,
            "INVALID_PARAMS",
            "Invalid parameters: the message needs at least one part",
        ));
    }
    let wanted = agent.or_else(|| {
        params
            .get("tenant")
            .and_then(|t| t.as_str())
            .filter(|t| !t.is_empty())
            .map(String::from)
    });
    let def: Arc<AgentDef> = match &wanted {
        Some(name) => state.served.find(name).ok_or_else(|| {
            RpcError::new(
                -32004,
                "UNSUPPORTED_OPERATION",
                format!("Not supported: there is no agent called {} here", name),
            )
        })?,
        None => state.served.default().ok_or_else(|| {
            RpcError::new(
                -32004,
                "UNSUPPORTED_OPERATION",
                "Not supported: this server has no agents",
            )
        })?,
    };
    let (name, args) = interpret(&def, message, params.get("metadata")).map_err(|why| {
        RpcError::new(
            -32004,
            "UNSUPPORTED_OPERATION",
            format!("Not supported: {}", why),
        )
    })?;
    // 0.1.4: the message is known, so the token is judged now: before the values are checked and before anything runs
    let auth = judge_request(state, &def.name, &name, headers).await?;
    if let Err(d) = crate::link::interface::check(&def, &name, &args) {
        let fix = d
            .suggestion
            .map(|f| format!(" ({})", f))
            .unwrap_or_default();
        return Err(RpcError::new(
            -32602,
            "INVALID_PARAMS",
            format!("Invalid parameters: {}{}", d.message, fix),
        ));
    }
    let task_id = new_id("task");
    let context = message
        .get("contextId")
        .and_then(|c| c.as_str())
        .filter(|c| !c.is_empty())
        .map(String::from)
        .unwrap_or_else(|| new_id("ctx"));
    let immediately = params
        .pointer("/configuration/returnImmediately")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    state.store.put(
        &task_id,
        task_json(&task_id, &context, "TASK_STATE_SUBMITTED", None, None),
        false,
    );
    // 0.1.4: a task from a protected message remembers the fingerprint of the token that started it (D14)
    if let Some(a) = &auth {
        if a.accepted {
            if let Some(fp) = &a.fingerprint {
                state.store.set_owner(&task_id, fp.clone());
            }
        }
    }
    let run = {
        let state = state.clone();
        let (task_id, context) = (task_id.clone(), context.clone());
        async move { run_task(state, def, name, args, task_id, context, auth).await }
    };
    if immediately {
        state.store.put(
            &task_id,
            task_json(&task_id, &context, "TASK_STATE_WORKING", None, None),
            false,
        );
        tokio::spawn(run);
        let task = state
            .store
            .get(&task_id)
            .unwrap_or_else(|| task_json(&task_id, &context, "TASK_STATE_WORKING", None, None));
        return Ok(json!({"task": task}));
    }
    Ok(json!({"task": run.await}))
}

/// Runs the agent and records the outcome as a completed or failed task.
async fn run_task(
    state: Arc<A2aState>,
    def: Arc<AgentDef>,
    message: String,
    args: crate::tools::Args,
    task_id: String,
    context: String,
    auth: Option<std::sync::Arc<RequestAuth>>, // 0.1.4
) -> Json {
    state.store.put(
        &task_id,
        task_json(&task_id, &context, "TASK_STATE_WORKING", None, None),
        false,
    );
    let outcome = match Agent::new(state.served.rt.clone(), def) {
        Ok(agent) => {
            let mut ctx = CallContext::new(task_id.clone());
            ctx.auth = auth; // 0.1.4: the token travels with this request only
            agent.handle(&message, args, &ctx).await
        }
        Err(d) => Err(d),
    };
    let task = match outcome {
        Ok(value) => task_json(
            &task_id,
            &context,
            "TASK_STATE_COMPLETED",
            None,
            Some(&value),
        ),
        Err(d) => task_json(
            &task_id,
            &context,
            "TASK_STATE_FAILED",
            Some(d.render()),
            None,
        ),
    };
    state.store.put(&task_id, task.clone(), true);
    task
}
