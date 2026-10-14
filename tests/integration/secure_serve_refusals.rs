// 0.1.3: new file. What `metagente serve` refuses to do over https, in plain words, and what changes while it runs.
use crate::common::a2a::WEATHER;
use crate::common::self_signed::{
    ServeProc, expired_pair_in, expiring_pair_in, free_port, mismatched_pair_in, pair_in,
    serve_refused,
};
use crate::common::*;
use std::path::Path;

fn folder(toml: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("weather.ag"), WEATHER).unwrap();
    std::fs::write(dir.path().join("metagente.toml"), toml).unwrap();
    dir
}

fn args(a2a: u16, more: &[&str]) -> Vec<String> {
    let mut v = vec![
        "weather.ag".to_string(),
        "--a2a".to_string(),
        a2a.to_string(),
    ];
    v.extend(more.iter().map(|s| s.to_string()));
    v
}

fn said(o: &std::process::Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

/// Words that would mean a Rust or TLS library message reached the person, or a secret did.
fn assert_plain(text: &str) {
    // the name of a PEM header may appear in a fix; the body of a key or certificate (it starts with MII) never may
    for word in ["rustls", "panicked", "Error {", "InvalidCertificate", "MII"] {
        assert!(!text.contains(word), "`{}` leaked: {}", word, text);
    }
}

const SECURE: &str = "[secure]\nagents = [\"Weather\"]\n";
const SERVE_PAIR: &str = "[serve]\ntls_cert = \"cert.pem\"\ntls_key = \"key.pem\"\n";

#[test]
fn a_secure_agent_without_a_certificate_refuses_to_start_and_never_falls_back_to_http() {
    let dir = folder(SECURE);
    let o = serve_refused(dir.path(), &args(free_port(), &[]));
    assert!(!o.status.success());
    let e = said(&o);
    assert!(
        e.contains("Weather is declared secure, so I will only serve it over https, but no certificate is set."),
        "{}",
        e
    );
    assert!(
        e.contains("tls_cert and tls_key in [serve]") && e.contains("--tls-cert and --tls-key"),
        "{}",
        e
    );
    assert_plain(&e);
}

#[test]
fn only_one_of_the_certificate_and_the_key_is_refused_naming_the_missing_one() {
    let dir = folder("[serve]\ntls_cert = \"cert.pem\"\n");
    let o = serve_refused(dir.path(), &args(free_port(), &[]));
    assert!(!o.status.success());
    assert!(
        said(&o).contains("I have a certificate but no key (tls_key)."),
        "{}",
        said(&o)
    );
    assert!(said(&o).contains("line 2"), "{}", said(&o));
    let dir = folder("[serve]\ntls_key = \"key.pem\"\n");
    let o = serve_refused(dir.path(), &args(free_port(), &[]));
    assert!(
        said(&o).contains("I have a key but no certificate (tls_cert)."),
        "{}",
        said(&o)
    );
    // the same with a flag alone
    let dir = folder("");
    let o = serve_refused(dir.path(), &args(free_port(), &["--tls-cert", "cert.pem"]));
    assert!(
        said(&o).contains("I have a certificate but no key (tls_key)."),
        "{}",
        said(&o)
    );
}

#[test]
fn a_key_that_does_not_match_the_certificate_is_refused() {
    let dir = folder(SERVE_PAIR);
    mismatched_pair_in(dir.path());
    let o = serve_refused(dir.path(), &args(free_port(), &[]));
    assert!(!o.status.success());
    let e = said(&o);
    assert!(e.contains("does not match the certificate in"), "{}", e);
    assert!(
        e.contains("line 3") || e.contains("metagente.toml"),
        "{}",
        e
    );
    assert_plain(&e);
}

#[test]
fn a_missing_or_unreadable_or_not_pem_file_names_the_file_and_the_toml_line() {
    let dir = folder(SERVE_PAIR);
    let o = serve_refused(dir.path(), &args(free_port(), &[]));
    let e = said(&o);
    assert!(e.contains("I could not read the certificate file"), "{}", e);
    assert!(
        e.contains("cert.pem") && e.contains("the file does not exist"),
        "{}",
        e
    );
    assert!(
        e.contains("line 2"),
        "the line of tls_cert in metagente.toml: {}",
        e
    );
    // not PEM
    std::fs::write(dir.path().join("cert.pem"), "this is not a certificate").unwrap();
    std::fs::write(dir.path().join("key.pem"), "this is not a key").unwrap();
    let e = said(&serve_refused(dir.path(), &args(free_port(), &[])));
    assert!(e.contains("is not a PEM certificate file"), "{}", e);
    assert_plain(&e);
    // a good certificate and a bad key
    let good = pair_in(dir.path());
    std::fs::write(&good.key, "nope").unwrap();
    let e = said(&serve_refused(dir.path(), &args(free_port(), &[])));
    assert!(e.contains("is not a PEM private key file"), "{}", e);
    assert!(e.contains("line 3"), "the line of tls_key: {}", e);
}

#[test]
fn an_expired_certificate_is_refused_and_one_that_expires_soon_only_warns() {
    let dir = folder(SERVE_PAIR);
    expired_pair_in(dir.path());
    let o = serve_refused(dir.path(), &args(free_port(), &[]));
    assert!(!o.status.success());
    let e = said(&o);
    assert!(e.contains("expired on 2000-01-01"), "{}", e);
    assert_plain(&e);
    expiring_pair_in(dir.path(), 10);
    let port = free_port();
    let serve = ServeProc::start(dir.path(), &args(port, &[]), 1);
    assert!(
        serve.said().contains("Warning: The certificate in"),
        "{}",
        serve.said()
    );
    assert!(serve.said().contains("expires on"), "{}", serve.said());
}

#[test]
fn a_secure_agent_over_mcp_stdio_is_refused() {
    let dir = folder(&format!("{}{}", SECURE, SERVE_PAIR));
    pair_in(dir.path());
    let o = serve_refused(
        dir.path(),
        &["weather.ag".into(), "--mcp".into(), "stdio".into()],
    );
    assert!(!o.status.success());
    let e = said(&o);
    assert!(
        e.contains("Weather is declared secure and stdio is not https."),
        "{}",
        e
    );
    assert!(e.contains("--mcp PORT and a certificate"), "{}", e);
}

#[test]
fn a_flag_wins_over_the_toml_and_one_line_says_so() {
    let dir = folder(SERVE_PAIR);
    pair_in(dir.path());
    let other = tempfile::tempdir().unwrap();
    let flagged = pair_in(other.path());
    let port = free_port();
    let serve = ServeProc::start(
        dir.path(),
        &args(
            port,
            &[
                "--tls-cert",
                &flagged.cert.display().to_string(),
                "--tls-key",
                &flagged.key.display().to_string(),
            ],
        ),
        1,
    );
    let said = serve.said();
    assert!(said.contains("Using --tls-cert"), "{}", said);
    assert!(
        said.contains("instead of tls_cert in metagente.toml"),
        "{}",
        said
    );
    assert!(said.contains("Using --tls-key"), "{}", said);
}

#[test]
fn a_relative_flag_path_is_relative_to_the_current_directory() {
    let dir = folder("");
    pair_in(dir.path());
    let port = free_port();
    let _serve = ServeProc::start(
        dir.path(),
        &args(port, &["--tls-cert", "cert.pem", "--tls-key", "key.pem"]),
        1,
    );
}

#[test]
fn public_keeps_its_warning_when_https_is_on() {
    let dir = folder(SERVE_PAIR);
    pair_in(dir.path());
    let port = free_port();
    let serve = ServeProc::start(dir.path(), &args(port, &["--public"]), 1);
    assert!(
        serve.said().contains("there is no login in this version"),
        "{}",
        serve.said()
    );
}

#[test]
fn a_port_that_is_already_in_use_gets_the_plain_message() {
    let dir = folder(SERVE_PAIR);
    pair_in(dir.path());
    let busy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = busy.local_addr().unwrap().port();
    let o = serve_refused(dir.path(), &args(port, &[]));
    assert!(!o.status.success());
    let e = said(&o);
    assert!(
        e.contains(&format!("I could not listen for A2A on 127.0.0.1:{}", port)),
        "{}",
        e
    );
    assert!(e.contains("choose another port"), "{}", e);
}

#[test]
fn the_help_lists_the_two_flags() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_metagente"))
        .args(["serve", "--help"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        text.contains("--tls-cert") && text.contains("--tls-key"),
        "{}",
        text
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_toml_change_to_the_certificate_while_serving_prints_one_restart_line_and_keeps_running()
{
    let dir = folder(&format!("{}{}", SECURE, SERVE_PAIR));
    let files = pair_in(dir.path());
    let port = free_port();
    let serve = ServeProc::start(dir.path(), &args(port, &[]), 1);
    let client = reqwest::Client::builder()
        .add_root_certificate(reqwest::Certificate::from_pem(files.ca_pem.as_bytes()).unwrap())
        .build()
        .unwrap();
    let card = format!("https://localhost:{}/.well-known/agent-card.json", port);
    assert!(
        client
            .get(&card)
            .send()
            .await
            .unwrap()
            .status()
            .is_success()
    );
    // point tls_cert at another file: a restart is needed for that
    std::fs::write(
        dir.path().join("metagente.toml"),
        format!(
            "{}[serve]\ntls_cert = \"other.pem\"\ntls_key = \"key.pem\"\n",
            SECURE
        ),
    )
    .unwrap();
    for _ in 0..3 {
        assert!(
            client
                .get(&card)
                .send()
                .await
                .unwrap()
                .status()
                .is_success()
        );
    }
    let text = serve.said();
    assert_eq!(
        text.matches("I keep serving as I started; restart `metagente serve`")
            .count(),
        1,
        "exactly one line, however many requests: {}",
        text
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_changed_secure_or_network_section_applies_to_the_next_request_without_a_restart() {
    use crate::common::tls_server;
    use axum::{Router, routing::get};
    use metagente::runtime::Runtime;
    use metagente::runtime::config::Config;
    use metagente::runtime::serve::Served;
    use std::sync::Arc;
    // an https tool whose certificate is only trusted when the toml says so, and a plain http one
    let cert = tls_server::self_signed(&["localhost"]);
    let tls_port =
        tls_server::serve_tls(Router::new().route("/hi", get(|| async { "hello" })), &cert).await;
    let plain = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let plain_port = plain.local_addr().unwrap().port();
    tokio::spawn(async move {
        let _ = axum::serve(plain, Router::new().route("/hi", get(|| async { "plain" }))).await;
    });
    let agent = |url: String| {
        format!(
            "agent Web\n  goal \"w\"\n  tool http\n  accepts go\n  on go\n    x = http.get url: \"{}\"\n    reply x.text\n",
            url
        )
    };
    let tls_source = agent(format!("https://localhost:{}/hi", tls_port));
    let plain_source = agent(format!("http://127.0.0.1:{}/hi", plain_port));
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.ag"), &plain_source).unwrap();
    let toml = |secure: &str, selfsigned: bool| {
        std::fs::write(
            dir.path().join("metagente.toml"),
            format!("{}[network]\nallow_self_signed = {}\n", secure, selfsigned),
        )
        .unwrap()
    };
    toml("", false);
    let rt: Arc<Runtime> = Runtime::new(Config::load(dir.path()).unwrap(), None);
    let agents = metagente::runtime::run::load_agents(&dir.path().join("a.ag")).unwrap();
    let served = Served::new(rt.clone(), dir.path().join("a.ag"), None, agents);
    let go = |source: String| {
        let rt = rt.clone();
        async move { run_source(&rt, &source, "go", &[]).await }
    };
    // 1. nothing is secure: plain http works, the self-signed https is refused
    assert_eq!(
        go(plain_source.clone()).await.unwrap().to_display(),
        "plain"
    );
    assert!(go(tls_source.clone()).await.is_err());
    // 2. `http` becomes secure: the next request refuses http, with no restart
    toml("[secure]\ntools = [\"http\"]\n", false);
    let _ = served.agents();
    let err = go(plain_source.clone()).await.unwrap_err().render();
    assert!(err.contains("http is declared secure"), "{}", err);
    // 3. allow_self_signed turns on: the https call works now, and plain http is still refused (D13)
    toml("[secure]\ntools = [\"http\"]\n", true);
    let _ = served.agents();
    assert_eq!(go(tls_source.clone()).await.unwrap().to_display(), "hello");
    assert!(go(plain_source.clone()).await.is_err());
    // 4. both go away again
    toml("", false);
    let _ = served.agents();
    assert_eq!(go(plain_source).await.unwrap().to_display(), "plain");
    assert!(go(tls_source).await.is_err());
}

fn _unused(_: &Path) {}
