// 0.1.3: new file. [secure] llm = true: the model is reached over https only, and no key is sent otherwise (User Story 5).
use crate::common::tls_server;
use crate::common::*;
use axum::{Json, Router, routing::post};
use metagente::llm::providers::from_config;
use metagente::runtime::config::Config;
use serde_json::json;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const KEY: &str = "MG_SECURE_LLM_KEY";
const THINK: &str = "agent T\n  goal \"t\"\n  accepts go\n  on go\n    reply think \"hello\"\n";

fn config_for(dir: &Path, base: &str, llm_secure: bool) -> Config {
    unsafe { std::env::set_var(KEY, "sk-secret-model-key") };
    let mut c = Config {
        root: dir.to_path_buf(),
        ..Config::default()
    };
    c.llm.provider = Some("anthropic".to_string());
    c.llm.model = Some("test-model".to_string());
    c.llm.base_url = Some(base.to_string());
    c.llm.api_key_env = Some(KEY.to_string());
    c.secure.llm = llm_secure;
    c
}

fn runtime(config: Config) -> Arc<metagente::runtime::Runtime> {
    let llm = from_config(&config).unwrap().unwrap();
    metagente::runtime::Runtime::new(config, Some(llm))
}

/// A gateway over plain http that counts every request it gets.
async fn plain_gateway() -> (String, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    let hits = count.clone();
    let app = Router::new().route(
        "/v1/messages",
        post(move || {
            let hits = hits.clone();
            async move {
                hits.fetch_add(1, Ordering::SeqCst);
                Json(json!({"content": [{"type": "text", "text": "hello from the gateway"}]}))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://127.0.0.1:{}", port), count)
}

#[tokio::test]
async fn with_llm_on_an_https_gateway_works() {
    let (ca_pem, cert) = tls_server::signed_by_new_authority(&["localhost"]);
    let app = Router::new().route(
        "/v1/messages",
        post(|| async {
            Json(json!({"content": [{"type": "text", "text": "hello from the gateway"}]}))
        }),
    );
    let port = tls_server::serve_tls(app, &cert).await;
    let dir = tempfile::tempdir().unwrap();
    let ca = dir.path().join("ca.pem");
    std::fs::write(&ca, ca_pem).unwrap();
    let mut config = config_for(dir.path(), &format!("https://localhost:{}", port), true);
    config.network.ca_file = Some(ca);
    let rt = runtime(config);
    let reply = run_source(&rt, THINK, "go", &[]).await.unwrap();
    assert_eq!(reply.to_display(), "hello from the gateway");
}

#[tokio::test]
async fn with_llm_on_an_http_gateway_is_refused_and_the_key_is_never_sent() {
    let (base, hits) = plain_gateway().await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(config_for(dir.path(), &base, true));
    let err = run_source(&rt, THINK, "go", &[])
        .await
        .unwrap_err()
        .render();
    assert!(
        err.contains(
            "The model address in [llm] uses http, and [secure] llm is on. Nothing was sent."
        ),
        "{}",
        err
    );
    assert!(err.contains("Problem on line"), "{}", err);
    assert!(!err.contains("sk-secret-model-key"), "{}", err);
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "not even a request may reach the gateway"
    );
}

#[tokio::test]
async fn with_llm_off_an_http_gateway_behaves_as_in_0_1_2() {
    let (base, hits) = plain_gateway().await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(config_for(dir.path(), &base, false));
    let reply = run_source(&rt, THINK, "go", &[]).await.unwrap();
    assert_eq!(reply.to_display(), "hello from the gateway");
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[test]
fn check_refuses_an_http_model_address_when_llm_is_on_and_accepts_the_default_hosted_one() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.ag"), THINK).unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        "[llm]\nprovider = \"anthropic\"\nbase_url = \"http://gateway.internal:8080\"\n\n[secure]\nllm = true\n",
    )
    .unwrap();
    let run = |dir: &Path| {
        Command::new(env!("CARGO_BIN_EXE_metagente"))
            .current_dir(dir)
            .args(["check", "a.ag"])
            .output()
            .unwrap()
    };
    let o = run(dir.path());
    let e = String::from_utf8_lossy(&o.stderr).to_string();
    assert!(!o.status.success(), "{}", e);
    assert!(
        e.contains(
            "The model address in [llm] uses http, and [secure] llm is on. Nothing was sent."
        ),
        "{}",
        e
    );
    assert!(e.contains("line 6"), "the line of llm in [secure]: {}", e);
    // the hosted address is https, so llm = true is satisfied without any base_url
    std::fs::write(
        dir.path().join("metagente.toml"),
        "[llm]\nprovider = \"anthropic\"\n\n[secure]\nllm = true\n",
    )
    .unwrap();
    let o = run(dir.path());
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}
