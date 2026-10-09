// 0.1.3: new file. Calling agents, tools and web addresses over https, and what happens when the certificate is bad.
use crate::common::a2a::{self, WEATHER};
use crate::common::tls_server::{self, TestCert};
use crate::common::*;
use axum::response::Redirect;
use axum::{Router, routing::get};
use std::path::Path;

fn trip(base: &str) -> String {
    project_file("examples/remote_weather.ag").replace("http://127.0.0.1:8080", base)
}

fn write_ca(dir: &Path, pem: &str) -> std::path::PathBuf {
    let path = dir.join("dev-ca.pem");
    std::fs::write(&path, pem).unwrap();
    path
}

/// Words that would mean a Rust or TLS library message reached the person.
fn assert_plain(text: &str) {
    for word in [
        "rustls",
        "UnknownIssuer",
        "InvalidCertificate",
        "NotValidForName",
        "hyper",
        "reqwest",
        "panicked",
    ] {
        assert!(!text.contains(word), "`{}` leaked: {}", word, text);
    }
}

struct Served {
    _dir: tempfile::TempDir,
    server: a2a::A2aServer,
    ca_pem: String,
}

async fn weather_over_tls(names: &[&str], expired: bool) -> Served {
    let dir = tempfile::tempdir().unwrap();
    let (ca_pem, cert) = tls_server::signed_by_new_authority_with(names, expired);
    let server = crate::common::a2a_tls::start(dir.path(), WEATHER, &cert).await;
    Served {
        _dir: dir,
        server,
        ca_pem,
    }
}

#[tokio::test]
async fn an_agent_is_called_over_https_when_its_authority_is_trusted_with_ca_file() {
    let served = weather_over_tls(&["localhost"], false).await;
    let dir = tempfile::tempdir().unwrap();
    let ca = write_ca(dir.path(), &served.ca_pem);
    let rt = runtime_in(dir.path(), None, |c| c.network.ca_file = Some(ca.clone()));
    let reply = run_source(
        &rt,
        &trip(&served.server.base),
        "plan",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap();
    assert_eq!(reply.to_display(), "Bob says: sunny in Lisbon");
}

#[tokio::test]
async fn an_unknown_authority_is_refused_in_plain_words_with_both_safe_fixes() {
    let served = weather_over_tls(&["localhost"], false).await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let err = run_source(
        &rt,
        &trip(&served.server.base),
        "plan",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap_err()
    .render();
    assert!(
        err.contains("I could not trust the certificate of"),
        "{}",
        err
    );
    assert!(
        err.contains(&served.server.base),
        "the address must be named: {}",
        err
    );
    assert!(err.contains("ca_file"), "{}", err);
    assert!(err.contains("allow_self_signed"), "{}", err);
    assert_plain(&err);
}

#[tokio::test]
async fn a_certificate_for_another_name_says_which_names_do_not_match() {
    let served = weather_over_tls(&["other.example"], false).await;
    let dir = tempfile::tempdir().unwrap();
    let ca = write_ca(dir.path(), &served.ca_pem);
    let rt = runtime_in(dir.path(), None, |c| c.network.ca_file = Some(ca.clone()));
    let err = run_source(
        &rt,
        &trip(&served.server.base),
        "plan",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap_err()
    .render();
    assert!(err.contains("is for"), "{}", err);
    assert!(err.contains("other.example"), "{}", err);
    assert!(err.contains("localhost"), "{}", err);
    assert_plain(&err);
}

#[tokio::test]
async fn an_expired_certificate_says_so() {
    let served = weather_over_tls(&["localhost"], true).await;
    let dir = tempfile::tempdir().unwrap();
    let ca = write_ca(dir.path(), &served.ca_pem);
    let rt = runtime_in(dir.path(), None, |c| c.network.ca_file = Some(ca.clone()));
    let err = run_source(
        &rt,
        &trip(&served.server.base),
        "plan",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap_err()
    .render();
    assert!(err.contains("expired"), "{}", err);
    assert_plain(&err);
}

fn http_agent(url: &str) -> String {
    format!(
        "agent Web\n  goal \"w\"\n  tool http\n  accepts go\n  on go\n    x = http.get url: \"{}\"\n    reply x.text\n",
        url
    )
}

#[tokio::test]
async fn the_http_tool_follows_the_same_rules() {
    let (ca_pem, cert) = tls_server::signed_by_new_authority(&["localhost"]);
    let port = tls_server::serve_tls(
        Router::new().route("/hi", get(|| async { "hello over tls" })),
        &cert,
    )
    .await;
    let address = format!("https://localhost:{}/hi", port);
    let dir = tempfile::tempdir().unwrap();

    let rt = runtime_in(dir.path(), None, |_| {});
    let err = run_source(&rt, &http_agent(&address), "go", &[])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("could not trust the certificate"), "{}", err);
    assert_plain(&err);

    let ca = write_ca(dir.path(), &ca_pem);
    let rt = runtime_in(dir.path(), None, |c| c.network.ca_file = Some(ca.clone()));
    let reply = run_source(&rt, &http_agent(&address), "go", &[])
        .await
        .unwrap();
    assert_eq!(reply.to_display(), "hello over tls");
}

#[tokio::test]
async fn an_https_address_that_redirects_to_plain_http_is_not_followed() {
    let plain = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let plain_port = plain.local_addr().unwrap().port();
    tokio::spawn(async move {
        let _ = axum::serve(
            plain,
            Router::new().route("/secret", get(|| async { "unprotected" })),
        )
        .await;
    });
    let (ca_pem, cert) = tls_server::signed_by_new_authority(&["localhost"]);
    let target = format!("http://127.0.0.1:{}/secret", plain_port);
    let app = Router::new().route(
        "/go",
        get(move || {
            let target = target.clone();
            async move { Redirect::temporary(&target) }
        }),
    );
    let port = tls_server::serve_tls(app, &cert).await;
    let dir = tempfile::tempdir().unwrap();
    let ca = write_ca(dir.path(), &ca_pem);
    let rt = runtime_in(dir.path(), None, |c| c.network.ca_file = Some(ca.clone()));
    let err = run_source(
        &rt,
        &http_agent(&format!("https://localhost:{}/go", port)),
        "go",
        &[],
    )
    .await
    .unwrap_err()
    .render();
    assert!(err.contains("will not follow"), "{}", err);
    assert!(
        err.contains("not secure") || err.contains("http address"),
        "{}",
        err
    );
    assert!(!err.contains("unprotected"), "{}", err);
    assert_plain(&err);
}

#[tokio::test]
async fn an_agent_card_that_sends_tasks_to_plain_http_is_refused() {
    // a plain server answers the tasks; the https one only hands out a card that points to it
    let plain_dir = tempfile::tempdir().unwrap();
    let plain = a2a::start(plain_dir.path(), WEATHER, None).await;
    let (ca_pem, cert) = tls_server::signed_by_new_authority(&["localhost"]);
    let plain_base = plain.base.clone();
    let app = Router::new().route(
        "/.well-known/agent-card.json",
        get(move || {
            let base = plain_base.clone();
            async move {
                axum::Json(serde_json::json!({
                    "name": "Weather",
                    "description": "x",
                    "version": "1",
                    "supportedInterfaces": [{"url": base, "protocolBinding": "JSONRPC", "protocolVersion": "1.0"}],
                    "capabilities": {},
                    "defaultInputModes": ["text/plain"],
                    "defaultOutputModes": ["text/plain"],
                    "skills": [{"id": "ask", "name": "ask", "description": "d", "tags": []}]
                }))
            }
        }),
    );
    let port = tls_server::serve_tls(app, &cert).await;
    let dir = tempfile::tempdir().unwrap();
    let ca = write_ca(dir.path(), &ca_pem);
    let rt = runtime_in(dir.path(), None, |c| c.network.ca_file = Some(ca.clone()));
    let err = run_source(
        &rt,
        &trip(&format!("https://localhost:{}", port)),
        "plan",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap_err()
    .render();
    assert!(err.contains("less secure"), "{}", err);
    assert!(err.contains("I will not follow it"), "{}", err);
}

#[allow(dead_code)]
fn _keep(_: TestCert) {}

// FR-023: https and a proxy do not widen what an agent may reach
#[tokio::test]
async fn https_and_a_proxy_do_not_let_an_agent_reach_what_it_never_declared() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |c| {
        c.network.allow_self_signed = true;
        c.network.proxy.url = Some("http://127.0.0.1:1".to_string());
    });
    // no `tool http` is declared
    let sneaky = "agent Sneaky\n  goal \"x\"\n  accepts go\n  on go\n    x = http.get url: \"https://localhost:9/\"\n    reply x.text\n";
    let err = run_source(&rt, sneaky, "go", &[])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("uses `http` but never declared it"), "{}", err);
    // no `remote` is declared
    let sneaky = "agent Sneaky\n  goal \"x\"\n  accepts go\n  on go\n    x = Bob.ask city: \"Lisbon\"\n    reply x\n";
    let err = run_source(&rt, sneaky, "go", &[])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("Bob"), "{}", err);
    assert!(
        err.contains("never declared") || err.contains("not declared") || err.contains("declare"),
        "{}",
        err
    );
}

// X9: the time spent in the handshake counts against the limit
#[tokio::test]
async fn a_server_that_never_finishes_the_handshake_is_given_up_on_in_time() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((stream, _)) = listener.accept().await {
            held.push(stream); // accepted, then silence: no TLS answer ever comes
        }
    });
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |c| c.timeout_seconds = 3);
    let started = std::time::Instant::now();
    let err = run_source(
        &rt,
        &http_agent(&format!("https://127.0.0.1:{}/", port)),
        "go",
        &[],
    )
    .await
    .unwrap_err()
    .render();
    assert!(
        started.elapsed().as_secs_f64() < 4.5,
        "took {:?}",
        started.elapsed()
    );
    assert!(
        err.contains("took too long") || err.contains("did not finish within"),
        "{}",
        err
    );
}

// 0.1.3: for an address given as an IP number the message must name that number readably
#[tokio::test]
async fn a_certificate_for_another_name_is_explained_readably_for_an_ip_address() {
    let (ca_pem, cert) = tls_server::signed_by_new_authority(&["other.example"]);
    let port =
        tls_server::serve_tls(Router::new().route("/hi", get(|| async { "x" })), &cert).await;
    let dir = tempfile::tempdir().unwrap();
    let ca = write_ca(dir.path(), &ca_pem);
    let rt = runtime_in(dir.path(), None, |c| c.network.ca_file = Some(ca.clone()));
    let err = run_source(
        &rt,
        &http_agent(&format!("https://127.0.0.1:{}/hi", port)),
        "go",
        &[],
    )
    .await
    .unwrap_err()
    .render();
    assert!(
        err.contains("is for other.example, but the address says 127.0.0.1"),
        "{}",
        err
    );
    assert!(!err.contains("V4") && !err.contains("Ipv4Addr"), "{}", err);
}
