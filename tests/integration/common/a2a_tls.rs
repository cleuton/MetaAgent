// 0.1.3: new file. The A2A test server over TLS (kept apart because the contract tests share a2a.rs without TLS).
use super::a2a::A2aServer;
use metagente::a2a::server::{A2aState, router};
use metagente::a2a::store::TaskStore;
use metagente::runtime::Runtime;
use metagente::runtime::config::Config;
use metagente::runtime::run::load_agents;
use metagente::runtime::serve::Served;
use std::path::Path;
use std::sync::Arc;

/// 0.1.3: the same server over TLS, reachable as https://localhost:PORT (the certificate must be valid for `localhost`).
pub async fn start(dir: &Path, source: &str, cert: &super::tls_server::TestCert) -> A2aServer {
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
    let state_cell: Arc<std::sync::Mutex<Option<Arc<A2aState>>>> = Default::default();
    let cell = state_cell.clone();
    let rt2 = rt.clone();
    let port = super::tls_server::serve_tls_with(
        move |port| {
            let base = format!("https://localhost:{}", port);
            let state = Arc::new(A2aState {
                served,
                store: TaskStore::default(),
                base,
            });
            if let Ok(mut c) = cell.lock() {
                *c = Some(state.clone());
            }
            // stands in for a reverse proxy that ends https, which tells the agent so
            router(state).layer(axum::middleware::from_fn(
                |mut request: axum::extract::Request, next: axum::middleware::Next| async move {
                    request.headers_mut().insert(
                        "x-forwarded-proto",
                        axum::http::HeaderValue::from_static("https"),
                    );
                    next.run(request).await
                },
            ))
        },
        cert,
    )
    .await;
    let state = state_cell.lock().expect("lock").clone().expect("state");
    A2aServer {
        base: format!("https://localhost:{}", port),
        state,
        rt: rt2,
    }
}
