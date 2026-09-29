//! An A2A server running inside the test process.

use metagente::a2a::server::{A2aState, router};
use metagente::a2a::store::TaskStore;
use metagente::runtime::Runtime;
use metagente::runtime::config::Config;
use metagente::runtime::run::load_agents;
use metagente::runtime::serve::Served;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

#[allow(dead_code)]
pub struct A2aServer {
    pub base: String,
    pub state: Arc<A2aState>,
    pub rt: Arc<Runtime>,
}

pub async fn start(dir: &Path, source: &str, retention: Option<Duration>) -> A2aServer {
    std::fs::write(dir.join("served.ag"), source).expect("write served.ag");
    let rt = Runtime::new(
        Config {
            root: dir.to_path_buf(),
            ..Config::default()
        },
        None,
    );
    let agents = load_agents(&dir.join("served.ag")).unwrap_or_else(|d| panic!("{}", d.render()));
    let served = Arc::new(Served::new(rt.clone(), dir.join("served.ag"), None, agents));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let base = format!(
        "http://127.0.0.1:{}",
        listener.local_addr().expect("addr").port()
    );
    let store = retention.map(TaskStore::with_retention).unwrap_or_default();
    let state = Arc::new(A2aState {
        served,
        store,
        base: base.clone(),
    });
    let app = router(state.clone());
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    A2aServer { base, state, rt }
}

/// A hand written JSON-RPC call, written straight from the A2A 1.0 text, not from Metagente's own client.
pub async fn rpc(base_path: &str, method: &str, params: serde_json::Value) -> serde_json::Value {
    let body = serde_json::json!({"jsonrpc": "2.0", "id": 7, "method": method, "params": params});
    reqwest::Client::new()
        .post(base_path)
        .header("A2A-Version", "1.0")
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await
        .expect("request")
        .json()
        .await
        .expect("json")
}

pub fn user_text(text: &str) -> serde_json::Value {
    serde_json::json!({"message": {"messageId": "m-1", "role": "ROLE_USER", "parts": [{"text": text}]}})
}

pub const WEATHER: &str = r#"agent Weather
  goal "Answer questions about the weather"
  accepts ask city  # weather for a city
  accepts broken
  on ask
    reply "sunny in {city}"
  on broken
    fail "the barometer exploded"
"#;

/// Serves `file` with a runtime you built (for example one with a model configured).
#[allow(dead_code)]
pub async fn start_with_runtime(file: &Path, rt: Arc<Runtime>) -> A2aServer {
    let agents = load_agents(file).unwrap_or_else(|d| panic!("{}", d.render()));
    let served = Arc::new(Served::new(rt.clone(), file.to_path_buf(), None, agents));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let base = format!(
        "http://127.0.0.1:{}",
        listener.local_addr().expect("addr").port()
    );
    let state = Arc::new(A2aState {
        served,
        store: TaskStore::default(),
        base: base.clone(),
    });
    let app = router(state.clone());
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    A2aServer { base, state, rt }
}
