//! A small local web server for the http tool and the model provider tests.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct Seen {
    pub bodies: Vec<Value>,
    pub headers: Vec<HeaderMap>,
    pub paths: Vec<String>,
}

pub struct Web {
    pub base: String,
    pub seen: Arc<Mutex<Seen>>,
}

/// `reply` is what the model endpoints answer with.
pub async fn start(reply: Value) -> Web {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let state = (seen.clone(), Arc::new(reply));
    async fn record(
        State((seen, reply)): State<(Arc<Mutex<Seen>>, Arc<Value>)>,
        path: &str,
        headers: HeaderMap,
        body: Value,
    ) -> Json<Value> {
        if let Ok(mut s) = seen.lock() {
            s.bodies.push(body);
            s.headers.push(headers);
            s.paths.push(path.to_string());
        }
        Json((*reply).clone())
    }
    let app =
        Router::new()
            .route(
                "/hello",
                get(|| async { Json(json!({"greeting": "hi", "count": 3})) }),
            )
            .route("/echo", post(|body: String| async move { body }))
            .route(
                "/v1/messages",
                post(
                    |s: State<(Arc<Mutex<Seen>>, Arc<Value>)>,
                     h: HeaderMap,
                     Json(b): Json<Value>| async move {
                        record(s, "/v1/messages", h, b).await
                    },
                ),
            )
            .route(
                "/v1/chat/completions",
                post(
                    |s: State<(Arc<Mutex<Seen>>, Arc<Value>)>,
                     h: HeaderMap,
                     Json(b): Json<Value>| async move {
                        record(s, "/v1/chat/completions", h, b).await
                    },
                ),
            )
            .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().expect("addr").port();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Web {
        base: format!("http://127.0.0.1:{}", port),
        seen,
    }
}

/// A mock Anthropic endpoint that answers with each reply in order, then repeats the last one.
/// Every request body and header is recorded in `seen`.
pub async fn start_scripted(replies: Vec<Value>) -> Web {
    use std::sync::atomic::{AtomicUsize, Ordering};
    type Shared = (Arc<Mutex<Seen>>, Arc<Vec<Value>>, Arc<AtomicUsize>);
    let seen = Arc::new(Mutex::new(Seen::default()));
    let state: Shared = (
        seen.clone(),
        Arc::new(replies),
        Arc::new(AtomicUsize::new(0)),
    );
    async fn answer(
        State((seen, replies, counter)): State<Shared>,
        headers: HeaderMap,
        Json(body): Json<Value>,
    ) -> Json<Value> {
        if let Ok(mut s) = seen.lock() {
            s.bodies.push(body);
            s.headers.push(headers);
            s.paths.push("/v1/messages".to_string());
        }
        let index = counter
            .fetch_add(1, Ordering::SeqCst)
            .min(replies.len().saturating_sub(1));
        Json(replies.get(index).cloned().unwrap_or(Value::Null))
    }
    let app = Router::new()
        .route("/v1/messages", post(answer))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().expect("addr").port();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Web {
        base: format!("http://127.0.0.1:{}", port),
        seen,
    }
}

/// The two kinds of answer the sample's model gives: use a tool, or write text.
pub fn tool_use(id: &str, name: &str, input: Value) -> Value {
    json!({"content": [{"type": "tool_use", "id": id, "name": name, "input": input}]})
}

pub fn text_reply(text: &str) -> Value {
    json!({"content": [{"type": "text", "text": text}]})
}
