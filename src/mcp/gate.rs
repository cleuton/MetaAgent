//! 0.1.4: the door in front of `/mcp` for a process that protects messages. The MCP library answers every error with
//! HTTP 200, so the token is judged here, where a real 401 or 503 can be sent (D9, FR-016). A request that gets past is
//! marked with what it carried, and the tool call handler picks the mark up.

use crate::runtime::auth::{Token, judge, parse_authorization};
use crate::runtime::serve::Served;
use axum::body::{Body, to_bytes};
use axum::extract::{Request, State};
use axum::http::{Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};
use std::sync::Arc;

/// D24: the most this door reads of one request.
pub const BODY_LIMIT: usize = 1024 * 1024;

/// What the request carried: put in the request extensions, read by `AgentsMcp::call_tool`.
#[derive(Clone, Debug)]
pub struct McpAuthMark {
    pub token: Option<Token>,
    /// True only if a protected tool in this request was judged and accepted.
    pub accepted: bool,
}

fn error_response(status: StatusCode, body: Value) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "application/json")],
        body.to_string(),
    )
        .into_response()
}

/// The `tools/call` requests of one JSON-RPC body (a single request or a batch) that name a protected tool of an
/// existing agent and message. Names that do not exist are left to the library, which answers "not supported".
fn protected_calls(served: &Served, body: &Value) -> Vec<(Value, String)> {
    let Some(policy) = served.rt.config.auth.as_ref() else {
        return Vec::new();
    };
    let items: Vec<&Value> = match body {
        Value::Array(list) => list.iter().collect(),
        other => vec![other],
    };
    let mut found = Vec::new();
    for item in items {
        if item.get("method").and_then(|m| m.as_str()) != Some("tools/call") {
            continue;
        }
        let Some(name) = item.pointer("/params/name").and_then(|n| n.as_str()) else {
            continue;
        };
        let Some((agent, message)) = name.split_once('.') else {
            continue;
        };
        let exists = served
            .find(agent)
            .map(|def| def.accept(message).is_some())
            .unwrap_or(false);
        if exists && policy.is_protected(agent, message) {
            found.push((
                item.get("id").cloned().unwrap_or(Value::Null),
                name.to_string(),
            ));
        }
    }
    found
}

pub async fn gate(State(served): State<Arc<Served>>, request: Request, next: Next) -> Response {
    let Some(policy) = served.rt.config.auth.clone() else {
        return next.run(request).await;
    };
    let (mut parts, body) = request.into_parts();
    let parsed = parse_authorization(&parts.headers);
    let mut accepted = false;
    let mut token_for_mark = parsed.as_ref().ok().and_then(|t| t.clone());
    if parts.method == Method::POST {
        let read = match to_bytes(body, BODY_LIMIT).await {
            Ok(b) => b,
            Err(_) => {
                return error_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    json!({"error": "This request is larger than 1 MiB, which is the most a server with protected messages reads on /mcp."}),
                );
            }
        };
        let calls = serde_json::from_slice::<Value>(&read)
            .map(|v| protected_calls(&served, &v))
            .unwrap_or_default();
        if let Some((id, what)) = calls.first() {
            let (verdict, token) = judge(&policy, parsed).await;
            if let Some(refusal) = verdict.refusal(what, &policy) {
                eprintln!("{}", refusal.log);
                let mut response = error_response(
                    StatusCode::from_u16(refusal.status).unwrap_or(StatusCode::UNAUTHORIZED),
                    crate::runtime::auth::refusal_json(id, &refusal),
                );
                if refusal.status == 401 {
                    response.headers_mut().insert(
                        header::WWW_AUTHENTICATE,
                        header::HeaderValue::from_static("Bearer"),
                    );
                }
                return response;
            }
            accepted = true;
            token_for_mark = token;
        }
        parts.extensions.insert(McpAuthMark {
            token: token_for_mark,
            accepted,
        });
        let body = Body::from(read);
        return next.run(Request::from_parts(parts, body)).await;
    }
    parts.extensions.insert(McpAuthMark {
        token: token_for_mark,
        accepted,
    });
    next.run(Request::from_parts(parts, body)).await
}
