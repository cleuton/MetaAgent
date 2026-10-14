// 0.1.3: new file. A tool listed in [secure] tools is reached over https only (User Story 2).
use crate::common::tls_server;
use crate::common::*;
use axum::{Router, response::Redirect, routing::get};
use metagente::runtime::value::Value;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn weather_source(url: &str) -> String {
    project_file("examples/weather.ag").replace("npx -y weather-mcp", url)
}

fn write_ca(dir: &std::path::Path, pem: &str) -> std::path::PathBuf {
    let path = dir.join("ca.pem");
    std::fs::write(&path, pem).unwrap();
    path
}

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
async fn a_secure_tool_over_https_lists_validates_and_calls_over_tls() {
    let (ca_pem, cert) = tls_server::signed_by_new_authority(&["localhost"]);
    let server = crate::common::fake_mcp::start_tls(&cert).await;
    let dir = tempfile::tempdir().unwrap();
    let ca = write_ca(dir.path(), &ca_pem);
    let rt = runtime_in(dir.path(), None, |c| {
        c.network.ca_file = Some(ca.clone());
        c.secure.tools = vec!["weather".to_string()];
    });
    let reply = run_source(
        &rt,
        &weather_source(&server.url),
        "ask",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap();
    assert_eq!(reply, Value::text("In Lisbon it will be sunny, 24 degrees"));
    // argument checking works over TLS too
    let wrong = format!(
        "agent W\n  goal \"w\"\n  tool weather from mcp \"{}\"\n  accepts go\n  on go\n    x = weather.forecast town: \"Lisbon\"\n    reply x\n",
        server.url
    );
    let err = run_source(&rt, &wrong, "go", &[])
        .await
        .unwrap_err()
        .render();
    assert!(
        err.contains("`forecast` needs a value for `city`"),
        "{}",
        err
    );
}

#[tokio::test]
async fn a_secure_tool_over_http_is_refused_before_any_connection() {
    let (base, count) = counting_listener().await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |c| {
        c.secure.tools = vec!["weather".to_string()];
    });
    let err = run_source(
        &rt,
        &weather_source(&format!("{}/mcp", base)),
        "ask",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap_err()
    .render();
    assert!(
        err.contains(&format!(
            "weather is declared secure in metagente.toml but this address uses http: {}/mcp",
            base
        )),
        "{}",
        err
    );
    assert!(
        err.contains("remove weather from [secure] tools"),
        "{}",
        err
    );
    assert!(err.contains("line"), "{}", err);
    settle().await;
    assert_eq!(count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_tool_that_is_not_secure_still_works_over_plain_http() {
    let server = crate::common::fake_mcp::start().await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |c| {
        c.secure.tools = vec!["somethingelse".to_string()];
    });
    let reply = run_source(
        &rt,
        &weather_source(&server.url),
        "ask",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap();
    assert_eq!(reply, Value::text("In Lisbon it will be sunny, 24 degrees"));
}

fn http_agent(url: &str) -> String {
    format!(
        "agent Web\n  goal \"w\"\n  tool http\n  accepts go\n  on go\n    x = http.get url: \"{}\"\n    reply x.text\n",
        url
    )
}

#[tokio::test]
async fn the_builtin_http_tool_can_be_secure() {
    let (ca_pem, cert) = tls_server::signed_by_new_authority(&["localhost"]);
    let port = tls_server::serve_tls(
        Router::new().route("/hi", get(|| async { "hello over tls" })),
        &cert,
    )
    .await;
    let (plain, count) = counting_listener().await;
    let dir = tempfile::tempdir().unwrap();
    let ca = write_ca(dir.path(), &ca_pem);
    let rt = runtime_in(dir.path(), None, |c| {
        c.network.ca_file = Some(ca.clone());
        c.secure.tools = vec!["http".to_string()];
    });
    let reply = run_source(
        &rt,
        &http_agent(&format!("https://localhost:{}/hi", port)),
        "go",
        &[],
    )
    .await
    .unwrap();
    assert_eq!(reply.to_display(), "hello over tls");
    // `run` stops at the literal address before anything is sent; the call itself is refused too
    let agent = agent_from(&rt, &http_agent(&format!("{}/x", plain)), None);
    let err = ask(&agent, "go", &[]).await.unwrap_err().render();
    assert!(
        err.contains("http is declared secure in metagente.toml but this address uses http"),
        "{}",
        err
    );
    assert!(err.contains("remove http from [secure] tools"), "{}", err);
    settle().await;
    assert_eq!(count.load(Ordering::SeqCst), 0);
    // post is refused as well
    let post = format!(
        "agent Web\n  goal \"w\"\n  tool http\n  accepts go\n  on go\n    x = http.post url: \"{}/p\" body: \"x\"\n    reply x.status\n",
        plain
    );
    let agent = agent_from(&rt, &post, None);
    assert!(ask(&agent, "go", &[]).await.is_err());
    settle().await;
    assert_eq!(count.load(Ordering::SeqCst), 0);
}

fn metagente(dir: &std::path::Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_metagente"));
    c.current_dir(dir);
    c
}

#[test]
fn a_local_tool_listed_as_secure_is_an_error_from_check_and_run() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("a.ag"),
        "agent A\n  goal \"a\"\n  tool files from mcp \"npx -y files-mcp\"\n  accepts go\n  on go\n    reply \"ok\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        "[secure]\ntools = [\"files\"]\n",
    )
    .unwrap();
    for command in ["check", "run"] {
        let o = metagente(dir.path())
            .args([command, "a.ag"])
            .output()
            .unwrap();
        let e = String::from_utf8_lossy(&o.stderr).to_string();
        assert!(!o.status.success(), "{}: {}", command, e);
        assert!(
            e.contains("files runs as a local program, so it has no address to secure."),
            "{}: {}",
            command,
            e
        );
        assert!(
            e.contains("give the tool an https:// address"),
            "{}: {}",
            command,
            e
        );
        assert!(
            e.contains("Problem on line 3 of a.ag"),
            "{}: {}",
            command,
            e
        );
    }
}

#[test]
fn a_builtin_tool_listed_as_secure_says_it_never_uses_the_network() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("a.ag"),
        "agent A\n  goal \"a\"\n  tool clock\n  accepts go\n  on go\n    reply clock.now\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        "[secure]\ntools = [\"clock\"]\n",
    )
    .unwrap();
    let o = metagente(dir.path())
        .args(["check", "a.ag"])
        .output()
        .unwrap();
    let e = String::from_utf8_lossy(&o.stderr).to_string();
    assert!(!o.status.success(), "{}", e);
    assert!(
        e.contains("clock is a built in tool that never uses the network"),
        "{}",
        e
    );
}

#[tokio::test]
async fn a_third_party_tool_server_is_refused_over_http_and_an_https_redirect_to_http_is_not_followed()
 {
    // a fake external MCP server (not Metagente) answers over https; the same name over http is refused above
    let plain = counting_listener().await;
    let target = plain.0.clone();
    let (ca_pem, cert) = tls_server::signed_by_new_authority(&["localhost"]);
    let app = Router::new().route(
        "/mcp",
        axum::routing::any(move || {
            let target = target.clone();
            async move { Redirect::temporary(&format!("{}/mcp", target)) }
        }),
    );
    let port = tls_server::serve_tls(app, &cert).await;
    let dir = tempfile::tempdir().unwrap();
    let ca = write_ca(dir.path(), &ca_pem);
    let rt = runtime_in(dir.path(), None, |c| {
        c.network.ca_file = Some(ca.clone());
        c.secure.tools = vec!["weather".to_string()];
    });
    let err = run_source(
        &rt,
        &weather_source(&format!("https://localhost:{}/mcp", port)),
        "ask",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap_err()
    .render();
    assert!(
        err.contains("I will not follow it: an https address sent me to an http address"),
        "{}",
        err
    );
    for word in ["reqwest", "hyper", "rustls", "panicked"] {
        assert!(!err.contains(word), "`{}` leaked: {}", word, err);
    }
    settle().await;
    assert_eq!(
        plain.1.load(Ordering::SeqCst),
        0,
        "the http address behind the redirect must never be contacted"
    );
}

#[tokio::test]
async fn a_tool_server_that_stops_is_reported_in_plain_words_without_library_text() {
    // 0.1.3: the message used to carry the names of Rust types from the MCP library
    let server = crate::common::fake_mcp::start().await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let source = weather_source(&server.url);
    let first = run_source(&rt, &source, "ask", &[("city", "Lisbon")])
        .await
        .unwrap();
    assert_eq!(first, Value::text("In Lisbon it will be sunny, 24 degrees"));
    server.stop();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let err = run_source(&rt, &source, "ask", &[("city", "Lisbon")])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("the tool server"), "{}", err);
    for word in ["rmcp", "reqwest", "::", "WorkerTransport", "Transport ["] {
        assert!(!err.contains(word), "`{}` leaked: {}", word, err);
    }
}
