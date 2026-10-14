// 0.1.3: new file. A remote agent listed in [secure] agents is reached over https only (User Story 1).
use crate::common::a2a::{self, WEATHER};
use crate::common::tls_server;
use crate::common::*;
use axum::{Json, Router, response::Redirect, routing::get};
use metagente::lang::Params;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn trip(base: &str) -> String {
    project_file("examples/remote_weather.ag").replace("http://127.0.0.1:8080", base)
}

fn write_ca(dir: &std::path::Path, pem: &str) -> std::path::PathBuf {
    let path = dir.join("dev-ca.pem");
    std::fs::write(&path, pem).unwrap();
    path
}

/// A server that only counts how many connections it was asked to accept.
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

#[tokio::test]
async fn a_secure_remote_over_https_fetches_the_card_and_answers() {
    let dir = tempfile::tempdir().unwrap();
    let (ca_pem, cert) = tls_server::signed_by_new_authority_with(&["localhost"], false);
    let server = crate::common::a2a_tls::start(dir.path(), WEATHER, &cert).await;
    let ca = write_ca(dir.path(), &ca_pem);
    let rt = runtime_in(dir.path(), None, |c| {
        c.network.ca_file = Some(ca.clone());
        c.secure.agents = vec!["Bob".to_string()];
    });
    let reply = run_source(&rt, &trip(&server.base), "plan", &[("city", "Lisbon")])
        .await
        .unwrap();
    assert_eq!(reply.to_display(), "Bob says: sunny in Lisbon");
}

#[tokio::test]
async fn a_secure_remote_over_http_is_refused_before_any_connection_is_opened() {
    let (base, count) = counting_listener().await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |c| {
        c.secure.agents = vec!["Bob".to_string()];
    });
    let err = run_source(&rt, &trip(&base), "plan", &[("city", "Lisbon")])
        .await
        .unwrap_err()
        .render();
    assert!(
        err.contains(&format!(
            "Bob is declared secure in metagente.toml but this address uses http: {}",
            base
        )),
        "{}",
        err
    );
    assert!(err.contains("write https:// instead of http://"), "{}", err);
    assert!(err.contains("remove Bob from [secure] agents"), "{}", err);
    assert!(err.contains("line"), "the line must be named: {}", err);
    settle().await;
    assert_eq!(
        count.load(Ordering::SeqCst),
        0,
        "nothing may be sent to Bob"
    );
}

#[tokio::test]
async fn an_address_from_a_parameter_that_resolves_to_http_is_refused_at_run_time() {
    let (base, count) = counting_listener().await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |c| {
        c.secure.agents = vec!["leitor".to_string()];
    });
    let mut params = Params::default();
    params.values.insert("a2a_leitor".to_string(), base.clone());
    params
        .values
        .insert("prompt1".to_string(), "you are a recruiter".to_string());
    let agents = metagente::lang::parse_file_with(
        "parameters.ag",
        None,
        &project_file("examples/parameters.ag"),
        &params,
    )
    .unwrap();
    let def = agents.into_iter().next().unwrap();
    let agent = metagente::runtime::interpreter::Agent::new(rt.clone(), Arc::new(def)).unwrap();
    let err = ask(&agent, "screen", &[("candidate", "Ana")])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("leitor is declared secure"), "{}", err);
    assert!(err.contains(&base), "{}", err);
    settle().await;
    assert_eq!(count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_remote_that_is_not_secure_still_works_over_plain_http() {
    let dir = tempfile::tempdir().unwrap();
    let server = a2a::start(dir.path(), WEATHER, None).await;
    // a secure list exists, but it does not name Bob
    let rt = runtime_in(dir.path(), None, |c| {
        c.secure.agents = vec!["Somebody".to_string()];
    });
    let reply = run_source(&rt, &trip(&server.base), "plan", &[("city", "Lisbon")])
        .await
        .unwrap();
    assert_eq!(reply.to_display(), "Bob says: sunny in Lisbon");
}

/// An https server whose agent card says where tasks go.
async fn card_server(card_url: impl Fn(u16) -> String + Send + 'static) -> (String, String) {
    let (ca_pem, cert) = tls_server::signed_by_new_authority(&["localhost"]);
    let port = tls_server::serve_tls_with(
        move |port| {
            let url = card_url(port);
            Router::new().route(
                "/.well-known/agent-card.json",
                get(move || {
                    let url = url.clone();
                    async move {
                        Json(json!({
                            "supportedInterfaces": [{"protocolBinding": "JSONRPC", "protocolVersion": "1.0", "url": url}],
                            "skills": [{"id": "ask", "description": "weather"}]
                        }))
                    }
                }),
            )
        },
        &cert,
    )
    .await;
    (format!("https://localhost:{}", port), ca_pem)
}

#[tokio::test]
async fn a_card_that_sends_tasks_to_http_is_refused_naming_both_addresses() {
    let (base, ca_pem) = card_server(|port| format!("http://localhost:{}/a2a", port)).await;
    let dir = tempfile::tempdir().unwrap();
    let ca = write_ca(dir.path(), &ca_pem);
    let rt = runtime_in(dir.path(), None, |c| {
        c.network.ca_file = Some(ca.clone());
        c.secure.agents = vec!["Bob".to_string()];
    });
    let err = run_source(&rt, &trip(&base), "plan", &[("city", "Lisbon")])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("points tasks to http://localhost:"), "{}", err);
    assert!(err.contains(&base), "both addresses must be named: {}", err);
    assert!(err.contains("I will not follow it"), "{}", err);
}

#[tokio::test]
async fn a_card_that_moves_a_secure_agent_to_another_host_is_refused() {
    let (base, ca_pem) = card_server(|port| format!("https://127.0.0.1:{}/a2a", port)).await;
    let dir = tempfile::tempdir().unwrap();
    let ca = write_ca(dir.path(), &ca_pem);
    let rt = runtime_in(dir.path(), None, |c| {
        c.network.ca_file = Some(ca.clone());
        c.secure.agents = vec!["Bob".to_string()];
    });
    let err = run_source(&rt, &trip(&base), "plan", &[("city", "Lisbon")])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("a different host than"), "{}", err);
    assert!(
        err.contains("Bob is declared secure, so I will not follow it"),
        "{}",
        err
    );
    assert!(
        err.contains("https://127.0.0.1:") && err.contains(&base),
        "{}",
        err
    );
}

#[tokio::test]
async fn an_https_address_that_redirects_to_http_is_refused_for_a_secure_agent() {
    let plain = counting_listener().await;
    let target = plain.0.clone();
    let (ca_pem, cert) = tls_server::signed_by_new_authority(&["localhost"]);
    let app = Router::new().route(
        "/.well-known/agent-card.json",
        get(move || {
            let target = target.clone();
            async move { Redirect::temporary(&format!("{}/card", target)) }
        }),
    );
    let port = tls_server::serve_tls(app, &cert).await;
    let dir = tempfile::tempdir().unwrap();
    let ca = write_ca(dir.path(), &ca_pem);
    let rt = runtime_in(dir.path(), None, |c| {
        c.network.ca_file = Some(ca.clone());
        c.secure.agents = vec!["Bob".to_string()];
    });
    let err = run_source(
        &rt,
        &trip(&format!("https://localhost:{}", port)),
        "plan",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap_err()
    .render();
    assert!(err.contains("I will not follow it"), "{}", err);
    settle().await;
    assert_eq!(
        plain.1.load(Ordering::SeqCst),
        0,
        "the http address must never be contacted"
    );
}

#[tokio::test]
async fn an_https_address_on_a_port_that_answers_plain_http_fails_without_a_retry_over_http() {
    // a server that speaks plain http on the port the https address points to
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            seen.fetch_add(1, Ordering::SeqCst);
            use tokio::io::AsyncWriteExt;
            let _ = socket
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nhi")
                .await;
        }
    });
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |c| {
        c.secure.agents = vec!["Bob".to_string()];
    });
    let err = run_source(
        &rt,
        &trip(&format!("https://127.0.0.1:{}", port)),
        "plan",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap_err()
    .render();
    assert!(!err.contains("Bob says"), "{}", err);
    assert!(
        err.contains(&format!(
            "I could not complete a secure connection to https://127.0.0.1:{}",
            port
        )),
        "{}",
        err
    );
    assert!(err.contains("I never retry over http"), "{}", err);
    for word in ["rustls", "hyper", "reqwest"] {
        assert!(!err.contains(word), "`{}` leaked: {}", word, err);
    }
    settle().await;
    assert_eq!(
        count.load(Ordering::SeqCst),
        1,
        "one attempt over https, and no retry over http"
    );
}
