// 0.1.4: a fake A2A agent and a fake MCP address that are not Metagente. They record the Authorization header of every
// call (FR-044) and can answer 401 or a redirect.
use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    /// Answers every task with a completed task whose text is "fake-answer".
    Normal,
    /// Answers HTTP 401 to every task call.
    Unauthorized,
    /// Answers 302 to `redirect_to` on every task call.
    Redirect,
}

pub struct Fake {
    pub port: u16,
    /// (path, Authorization header) of every request, in order.
    pub seen: Arc<Mutex<Vec<(String, Option<String>)>>>,
}

impl Fake {
    pub fn authorizations(&self, path_contains: &str) -> Vec<Option<String>> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|(p, _)| p.contains(path_contains))
            .map(|(_, a)| a.clone())
            .collect()
    }
    pub fn count(&self) -> usize {
        self.seen.lock().unwrap().len()
    }
}

#[derive(Clone)]
struct St {
    port: u16,
    mode: Mode,
    redirect_to: String,
    seen: Arc<Mutex<Vec<(String, Option<String>)>>>,
}

fn note(st: &St, path: &str, headers: &HeaderMap) {
    let auth = headers
        .get(header::AUTHORIZATION)
        .map(|v| v.to_str().unwrap().to_string());
    st.seen.lock().unwrap().push((path.to_string(), auth));
}

async fn card(State(st): State<St>, headers: HeaderMap) -> Response {
    note(&st, "/card", &headers);
    axum::Json(serde_json::json!({
        "name": "Researcher", "description": "fake", "version": "1",
        "supportedInterfaces": [{"url": format!("http://127.0.0.1:{}/a2a", st.port), "protocolBinding": "JSONRPC", "protocolVersion": "1.0"}],
        "capabilities": {}, "defaultInputModes": ["text/plain"], "defaultOutputModes": ["text/plain"],
        "skills": [{"id": "research", "name": "research", "description": "x", "tags": []}]
    }))
    .into_response()
}

async fn rpc(State(st): State<St>, headers: HeaderMap, body: String) -> Response {
    note(&st, "/a2a", &headers);
    match st.mode {
        Mode::Unauthorized => (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer")],
            "{}",
        )
            .into_response(),
        Mode::Redirect => (
            StatusCode::FOUND,
            [(header::LOCATION, st.redirect_to.clone())],
            "",
        )
            .into_response(),
        Mode::Normal => {
            let id = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("id").cloned())
                .unwrap_or(serde_json::Value::Null);
            axum::Json(
                serde_json::json!({"jsonrpc": "2.0", "id": id, "result": {"task": {
                    "id": "t-1", "contextId": "c-1",
                    "status": {"state": "TASK_STATE_COMPLETED"},
                    "artifacts": [{"artifactId": "a", "parts": [{"text": "fake-answer"}]}]
                }}}),
            )
            .into_response()
        }
    }
}

async fn anything(State(st): State<St>, headers: HeaderMap) -> Response {
    note(&st, "/other", &headers);
    (StatusCode::UNAUTHORIZED, "no").into_response()
}

/// A fake A2A agent (and whatever else is asked of it: `/mcp` answers 401 like a protected tool server).
pub async fn start(mode: Mode, redirect_to: &str) -> Fake {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let st = St {
        port,
        mode,
        redirect_to: redirect_to.to_string(),
        seen: seen.clone(),
    };
    let app = Router::new()
        .route("/.well-known/agent-card.json", get(card))
        .route("/a2a", post(rpc))
        .fallback(anything)
        .with_state(st);
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Fake { port, seen }
}
