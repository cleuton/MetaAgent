// 0.1.3: new file. A self-signed certificate is accepted only when metagente.toml says so, and independently of the
// secure list (User Story 6, D13). The served agent is Metagente itself, started with `metagente serve`.
use crate::common::a2a::WEATHER;
use crate::common::self_signed::{ServeProc, free_port, self_signed_pair_in};
use crate::common::*;
use std::process::Command;

struct Served {
    _dir: tempfile::TempDir,
    _serve: ServeProc,
    port: u16,
    cert_pem: String,
    cert_path: std::path::PathBuf,
}

fn serve_self_signed() -> Served {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("weather.ag"), WEATHER).unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        "[secure]\nagents = [\"Weather\"]\n[serve]\ntls_cert = \"cert.pem\"\ntls_key = \"key.pem\"\n",
    )
    .unwrap();
    let files = self_signed_pair_in(dir.path());
    let port = free_port();
    let serve = ServeProc::start(
        dir.path(),
        &[
            "weather.ag".to_string(),
            "--a2a".to_string(),
            port.to_string(),
        ],
        1,
    );
    Served {
        cert_pem: files.ca_pem.clone(),
        cert_path: files.cert.clone(),
        port,
        _serve: serve,
        _dir: dir,
    }
}

fn trip(host: &str, port: u16) -> String {
    project_file("examples/remote_weather.ag").replace(
        "http://127.0.0.1:8080",
        &format!("https://{}:{}", host, port),
    )
}

async fn call(
    rt: &std::sync::Arc<metagente::runtime::Runtime>,
    host: &str,
    port: u16,
) -> Result<String, String> {
    run_source(rt, &trip(host, port), "plan", &[("city", "Lisbon")])
        .await
        .map(|v| v.to_display())
        .map_err(|d| d.render())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_any_option_a_self_signed_agent_is_refused_with_both_safe_fixes() {
    let served = serve_self_signed();
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |c| {
        c.secure.agents = vec!["Bob".to_string()]
    });
    let err = call(&rt, "localhost", served.port).await.unwrap_err();
    assert!(
        err.contains("I could not trust the certificate of"),
        "{}",
        err
    );
    assert!(
        err.contains(&format!("https://localhost:{}", served.port)),
        "{}",
        err
    );
    assert!(
        err.contains("ca_file") && err.contains("allow_self_signed"),
        "{}",
        err
    );
    for word in ["rustls", "UnknownIssuer", "hyper", "reqwest"] {
        assert!(!err.contains(word), "`{}` leaked: {}", word, err);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_file_that_trusts_the_certificate_makes_the_call_work_with_validation_still_on() {
    let served = serve_self_signed();
    let dir = tempfile::tempdir().unwrap();
    let ca = dir.path().join("ca.pem");
    std::fs::write(&ca, &served.cert_pem).unwrap();
    let rt = runtime_in(dir.path(), None, |c| {
        c.network.ca_file = Some(ca.clone());
        c.secure.agents = vec!["Bob".to_string()];
    });
    assert_eq!(
        call(&rt, "localhost", served.port).await.unwrap(),
        "Bob says: sunny in Lisbon"
    );
    // validation is still on: the name must match, so another name is refused even with ca_file
    let err = call(&rt, "127.0.0.2", served.port).await;
    assert!(
        err.is_err(),
        "another address must not be trusted: {:?}",
        err
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn allow_self_signed_makes_the_call_work_and_self_signed_hosts_limits_it() {
    let served = serve_self_signed();
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |c| {
        c.network.allow_self_signed = true;
        c.secure.agents = vec!["Bob".to_string()];
    });
    assert_eq!(
        call(&rt, "localhost", served.port).await.unwrap(),
        "Bob says: sunny in Lisbon"
    );
    // limited to one host: the other host keeps its certificate checks
    let rt = runtime_in(dir.path(), None, |c| {
        c.network.allow_self_signed = true;
        c.network.self_signed_hosts = vec!["127.0.0.1".to_string()];
    });
    assert_eq!(
        call(&rt, "127.0.0.1", served.port).await.unwrap(),
        "Bob says: sunny in Lisbon"
    );
    let err = call(&rt, "localhost", served.port).await.unwrap_err();
    assert!(
        err.contains("I could not trust the certificate of"),
        "{}",
        err
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn allow_self_signed_never_makes_http_acceptable_for_a_secure_name() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |c| {
        c.network.allow_self_signed = true;
        c.secure.agents = vec!["Bob".to_string()];
    });
    let source = project_file("examples/remote_weather.ag");
    let err = run_source(&rt, &source, "plan", &[("city", "Lisbon")])
        .await
        .unwrap_err()
        .render();
    assert!(
        err.contains("Bob is declared secure in metagente.toml but this address uses http"),
        "{}",
        err
    );
}

#[test]
fn the_warning_for_skipped_certificate_checks_appears_on_run_serve_and_check() {
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
    for command in ["check", "run"] {
        let o = Command::new(env!("CARGO_BIN_EXE_metagente"))
            .current_dir(dir.path())
            .args([command, "a.ag"])
            .output()
            .unwrap();
        let e = String::from_utf8_lossy(&o.stderr).to_string();
        assert!(
            e.contains("Warning") && e.contains("allow_self_signed"),
            "{}: {}",
            command,
            e
        );
    }
    let files = self_signed_pair_in(dir.path());
    let port = free_port();
    let serve = ServeProc::start(
        dir.path(),
        &[
            "a.ag".to_string(),
            "--a2a".to_string(),
            port.to_string(),
            "--tls-cert".to_string(),
            files.cert.display().to_string(),
            "--tls-key".to_string(),
            files.key.display().to_string(),
        ],
        1,
    );
    assert!(
        serve.said().contains("allow_self_signed"),
        "{}",
        serve.said()
    );
}

#[allow(dead_code)]
fn _keep(served: &Served) -> &std::path::Path {
    &served.cert_path
}
