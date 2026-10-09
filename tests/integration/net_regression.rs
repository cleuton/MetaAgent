// 0.1.3: new file. Things that already worked in 0.1.2 keep working: a computer with no certificates at all.
use crate::common::tls_server;
use axum::{Router, routing::get};

const AGENT: &str = "agent Web\n  goal \"w\"\n  tool http\n  accepts go\n  on go\n    x = http.get url: \"URL\"\n    reply x.text\n";

async fn run_with_no_certificate_store(url: &str) -> (bool, String, String) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("web.ag"), AGENT.replace("URL", url)).unwrap();
    let out = tokio::process::Command::new(env!("CARGO_BIN_EXE_metagente"))
        .current_dir(dir.path())
        .args(["run", "web.ag", "go"])
        // a bare container: the system has no certificate files to offer
        .env("SSL_CERT_FILE", "/nonexistent/ca.pem")
        .env("SSL_CERT_DIR", "/nonexistent")
        .env_remove("HTTPS_PROXY")
        .env_remove("https_proxy")
        .output()
        .await
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

#[tokio::test]
async fn startup_and_plain_http_work_with_no_certificate_store() {
    let app = Router::new().route("/hi", get(|| async { "plain works" }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let (ok, out, err) =
        run_with_no_certificate_store(&format!("http://127.0.0.1:{}/hi", port)).await;
    assert!(ok, "{}", err);
    assert!(out.contains("plain works"), "{}", out);
}

#[tokio::test]
async fn https_with_no_certificate_store_still_starts_and_explains_itself() {
    let cert = tls_server::self_signed(&["localhost"]);
    let port =
        tls_server::serve_tls(Router::new().route("/hi", get(|| async { "x" })), &cert).await;
    let (ok, _, err) =
        run_with_no_certificate_store(&format!("https://localhost:{}/hi", port)).await;
    assert!(!ok);
    assert!(err.contains("could not trust the certificate"), "{}", err);
    for word in ["panicked", "rustls", "RUST_BACKTRACE"] {
        assert!(!err.contains(word), "{}", err);
    }
}
