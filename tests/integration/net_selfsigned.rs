// 0.1.3: new file. Self-signed certificates for development: one switch, a host list, and nothing else relaxed.
use crate::common::a2a::WEATHER;
use crate::common::tls_server;
use crate::common::*;
use axum::response::Redirect;
use axum::{Router, routing::get};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

fn trip(base: &str) -> String {
    project_file("examples/remote_weather.ag").replace("http://127.0.0.1:8080", base)
}

fn web_agent(url: &str) -> String {
    format!(
        "agent Web\n  goal \"w\"\n  tool http\n  accepts go\n  on go\n    x = http.get url: \"{}\"\n    reply x.text\n",
        url
    )
}

#[tokio::test]
async fn allow_self_signed_accepts_a_self_signed_agent_and_a_web_address() {
    let cert = tls_server::self_signed(&["localhost"]);
    let served_dir = tempfile::tempdir().unwrap();
    let server = crate::common::a2a_tls::start(served_dir.path(), WEATHER, &cert).await;
    let dir = tempfile::tempdir().unwrap();

    let strict = runtime_in(dir.path(), None, |_| {});
    let err = run_source(&strict, &trip(&server.base), "plan", &[("city", "Lisbon")])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("could not trust the certificate"), "{}", err);

    let lax = runtime_in(dir.path(), None, |c| c.network.allow_self_signed = true);
    let reply = run_source(&lax, &trip(&server.base), "plan", &[("city", "Lisbon")])
        .await
        .unwrap();
    assert_eq!(reply.to_display(), "Bob says: sunny in Lisbon");

    let port =
        tls_server::serve_tls(Router::new().route("/hi", get(|| async { "hello" })), &cert).await;
    let reply = run_source(
        &lax,
        &web_agent(&format!("https://localhost:{}/hi", port)),
        "go",
        &[],
    )
    .await
    .unwrap();
    assert_eq!(reply.to_display(), "hello");
}

#[tokio::test]
async fn a_name_that_is_not_on_the_certificate_is_skipped_only_under_the_switch() {
    let cert = tls_server::self_signed(&["localhost"]);
    let port =
        tls_server::serve_tls(Router::new().route("/hi", get(|| async { "hello" })), &cert).await;
    let by_ip = web_agent(&format!("https://127.0.0.1:{}/hi", port));
    let dir = tempfile::tempdir().unwrap();
    let lax = runtime_in(dir.path(), None, |c| c.network.allow_self_signed = true);
    assert_eq!(
        run_source(&lax, &by_ip, "go", &[])
            .await
            .unwrap()
            .to_display(),
        "hello"
    );
    let strict = runtime_in(dir.path(), None, |_| {});
    assert!(run_source(&strict, &by_ip, "go", &[]).await.is_err());
}

#[tokio::test]
async fn the_host_list_keeps_checks_on_for_every_other_host() {
    let cert = tls_server::self_signed(&["localhost"]);
    let port =
        tls_server::serve_tls(Router::new().route("/hi", get(|| async { "hello" })), &cert).await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |c| {
        c.network.allow_self_signed = true;
        c.network.self_signed_hosts = vec!["127.0.0.1".to_string()];
    });
    // on the list: accepted
    let listed = web_agent(&format!("https://127.0.0.1:{}/hi", port));
    assert_eq!(
        run_source(&rt, &listed, "go", &[])
            .await
            .unwrap()
            .to_display(),
        "hello"
    );
    // same server, another name that is not on the list: checks stay on
    let other = web_agent(&format!("https://localhost:{}/hi", port));
    let err = run_source(&rt, &other, "go", &[])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("could not trust the certificate"), "{}", err);
}

#[tokio::test]
async fn a_redirect_to_a_host_with_other_rules_is_judged_on_its_own() {
    // host A (127.0.0.1) is on the list and sends the call to host B (localhost), which is not
    let cert = tls_server::self_signed(&["localhost"]);
    let b = tls_server::serve_tls(
        Router::new().route("/hi", get(|| async { "from B" })),
        &cert,
    )
    .await;
    let target = format!("https://localhost:{}/hi", b);
    let a = tls_server::serve_tls(
        Router::new().route(
            "/go",
            get(move || {
                let target = target.clone();
                async move { Redirect::temporary(&target) }
            }),
        ),
        &cert,
    )
    .await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |c| {
        c.network.allow_self_signed = true;
        c.network.self_signed_hosts = vec!["127.0.0.1".to_string()];
    });
    let err = run_source(
        &rt,
        &web_agent(&format!("https://127.0.0.1:{}/go", a)),
        "go",
        &[],
    )
    .await
    .unwrap_err()
    .render();
    assert!(err.contains("will not follow it"), "{}", err);
    assert!(err.contains("different certificate rules"), "{}", err);
    assert!(!err.contains("from B"), "{}", err);
}

#[test]
fn serve_prints_the_warning_too() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("a.ag"),
        "agent A\n  goal \"a\"\n  accepts go\n  on go\n    reply \"ok\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        "[network]\nallow_self_signed = true\n",
    )
    .unwrap();
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mut child = Command::new(env!("CARGO_BIN_EXE_metagente"))
        .current_dir(dir.path())
        .args(["serve", "a.ag", "--a2a", &port.to_string()])
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stderr.take().unwrap()).lines();
    let first = lines.next().and_then(|l| l.ok()).unwrap_or_default();
    let _ = child.kill();
    let _ = child.wait();
    assert!(
        first.contains("certificate checks are OFF for agents and tools"),
        "{}",
        first
    );
}

// 0.1.3: "localhost:8443" in the list means the host localhost
#[tokio::test]
async fn a_host_in_the_list_may_be_written_with_a_port() {
    let cert = tls_server::self_signed(&["localhost"]);
    let port =
        tls_server::serve_tls(Router::new().route("/hi", get(|| async { "hello" })), &cert).await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |c| {
        c.network.allow_self_signed = true;
        c.network.self_signed_hosts = vec!["127.0.0.1:8443".to_string()];
    });
    let reply = run_source(
        &rt,
        &web_agent(&format!("https://127.0.0.1:{}/hi", port)),
        "go",
        &[],
    )
    .await
    .unwrap();
    assert_eq!(reply.to_display(), "hello");
}
