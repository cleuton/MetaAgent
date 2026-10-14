// 0.1.3: new file. An agent listed in [secure] agents makes only https calls, in every direction (User Story 4).
use crate::common::*;
use metagente::llm::providers::from_config;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

async fn counting_listener() -> (String, Arc<AtomicUsize>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    tokio::spawn(async move {
        while listener.accept().await.is_ok() {
            seen.fetch_add(1, Ordering::SeqCst);
        }
    });
    (format!("http://127.0.0.1:{}", port), count)
}

async fn settle() {
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
}

fn secure(c: &mut metagente::runtime::config::Config, name: &str) {
    c.secure.agents = vec![name.to_string()];
}

/// One handler per way out of the agent.
fn researcher(base: &str) -> String {
    format!(
        "agent Researcher\n  goal \"r\"\n  tool http\n  tool kb from mcp \"{base}/mcp\"\n  remote Facts at \"{base}\"\n  accepts web\n  accepts mcp\n  accepts remote\n  accepts model\n  on web\n    x = http.get url: \"{base}/x\"\n    reply x.status\n  on mcp\n    x = kb.forecast city: \"Lisbon\"\n    reply x\n  on remote\n    x = Facts.ask city: \"Lisbon\"\n    reply x\n  on model\n    reply think \"hello\"\n",
        base = base
    )
}

#[tokio::test]
async fn a_secure_agent_is_refused_on_every_outbound_http_call_before_any_connection() {
    let (base, count) = counting_listener().await;
    unsafe { std::env::set_var("MG_SECURE_AGENT_KEY", "sk-test") };
    let dir = tempfile::tempdir().unwrap();
    let rt = {
        let mut config = metagente::runtime::config::Config {
            root: dir.path().to_path_buf(),
            ..Default::default()
        };
        secure(&mut config, "Researcher");
        config.llm.provider = Some("anthropic".to_string());
        config.llm.model = Some("m".to_string());
        config.llm.base_url = Some(base.clone());
        config.llm.api_key_env = Some("MG_SECURE_AGENT_KEY".to_string());
        let llm = from_config(&config).unwrap().unwrap();
        metagente::runtime::Runtime::new(config, Some(llm))
    };
    let agent = agent_from(&rt, &researcher(&base), None);
    for message in ["web", "mcp", "remote", "model"] {
        let err = ask(&agent, message, &[]).await.unwrap_err().render();
        assert!(
            err.contains("Researcher is declared secure, so it may only call https addresses"),
            "{}: {}",
            message,
            err
        );
        assert!(
            err.contains(&base) || message == "model",
            "{}: {}",
            message,
            err
        );
        assert!(err.contains("Problem on line"), "{}: {}", message, err);
    }
    settle().await;
    assert_eq!(
        count.load(Ordering::SeqCst),
        0,
        "nothing may be sent anywhere"
    );
}

#[tokio::test]
async fn an_agent_that_is_not_secure_is_not_affected_by_the_secure_list() {
    let (base, _count) = counting_listener().await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |c| secure(c, "SomebodyElse"));
    let agent = agent_from(&rt, &researcher(&base), None);
    // the server never answers a real request, so the call fails, but not because of the secure rule
    let err = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        ask(&agent, "remote", &[]),
    )
    .await;
    if let Ok(Err(e)) = err {
        assert!(!e.render().contains("is declared secure"), "{}", e.render());
    }
}

#[tokio::test]
async fn a_secure_agent_still_uses_local_tools_and_built_in_tools() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |c| secure(c, "Local"));
    let source = "agent Local\n  goal \"l\"\n  tool clock\n  tool state\n  accepts go\n  on go\n    state.set key: \"k\" value: \"v\"\n    x = state.get key: \"k\"\n    reply x\n";
    let reply = run_source(&rt, source, "go", &[]).await.unwrap();
    assert_eq!(reply.to_display(), "v");
}

#[tokio::test]
async fn a_secure_agent_still_works_over_https_in_every_direction() {
    use crate::common::tls_server;
    use axum::{Router, routing::get};
    let (ca_pem, cert) = tls_server::signed_by_new_authority(&["localhost"]);
    let port = tls_server::serve_tls(
        Router::new().route("/x", get(|| async { "secure ok" })),
        &cert,
    )
    .await;
    let dir = tempfile::tempdir().unwrap();
    let ca = dir.path().join("ca.pem");
    std::fs::write(&ca, ca_pem).unwrap();
    let rt = runtime_in(dir.path(), None, |c| {
        secure(c, "Web");
        c.network.ca_file = Some(ca.clone());
    });
    let source = format!(
        "agent Web\n  goal \"w\"\n  tool http\n  accepts go\n  on go\n    x = http.get url: \"https://localhost:{}/x\"\n    reply x.text\n",
        port
    );
    let reply = run_source(&rt, &source, "go", &[]).await.unwrap();
    assert_eq!(reply.to_display(), "secure ok");
}
