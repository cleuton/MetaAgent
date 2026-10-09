// 0.1.3: new file. Going through a proxy: login by variable name, errors in plain words, no_proxy, the environment.
use crate::common::a2a::{self, WEATHER};
use crate::common::proxy::{self, Mode, TestProxy};
use crate::common::tls_server::{self, TestCert};
use crate::common::*;
use axum::http::HeaderMap;
use axum::{Router, routing::get};
use metagente::runtime::Runtime;
use metagente::runtime::config::Config;
use metagente::runtime::net::ClientSet;
use metagente::runtime::value::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const PASSWORD: &str = "p@ss:/%word";
const USER: &str = "ann";

fn trip(base: &str) -> String {
    project_file("examples/remote_weather.ag").replace("http://127.0.0.1:8080", base)
}

fn web_agent(url: &str) -> String {
    format!(
        "agent Web\n  goal \"w\"\n  tool http\n  accepts go\n  on go\n    x = http.get url: \"{}\"\n    reply x.text\n",
        url
    )
}

/// A runtime whose environment is exactly `env` (the real one never leaks in).
fn runtime_with(
    dir: &std::path::Path,
    env: &[(&str, &str)],
    tweak: impl FnOnce(&mut Config),
) -> Arc<Runtime> {
    let mut config = Config {
        root: dir.to_path_buf(),
        ..Config::default()
    };
    tweak(&mut config);
    let map: HashMap<String, String> = env
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let net = Arc::new(ClientSet::with_env(
        config.network.clone(),
        Duration::from_secs(2),
        move |n| map.get(n).cloned(),
    ));
    Runtime::with_net(config, None, net)
}

fn login_env() -> Vec<(&'static str, &'static str)> {
    vec![("PROXY_USER", USER), ("PROXY_PASSWORD", PASSWORD)]
}

fn with_login(config: &mut Config, proxy_url: &str) {
    config.network.proxy.url = Some(proxy_url.to_string());
    config.network.proxy.username_env = Some("PROXY_USER".to_string());
    config.network.proxy.password_env = Some("PROXY_PASSWORD".to_string());
}

struct TlsAgent {
    _dir: tempfile::TempDir,
    server: a2a::A2aServer,
    ca: std::path::PathBuf,
    port: u16,
}

async fn weather_over_tls(work: &std::path::Path) -> TlsAgent {
    let dir = tempfile::tempdir().unwrap();
    let (ca_pem, cert): (String, TestCert) = tls_server::signed_by_new_authority(&["localhost"]);
    let server = crate::common::a2a_tls::start(dir.path(), WEATHER, &cert).await;
    let ca = work.join("ca.pem");
    std::fs::write(&ca, ca_pem).unwrap();
    let port: u16 = server.base.rsplit(':').next().unwrap().parse().unwrap();
    TlsAgent {
        _dir: dir,
        server,
        ca,
        port,
    }
}

fn assert_no_secret(text: &str) {
    for secret in [PASSWORD, USER, "cDpzcw"] {
        // the user name is short, so it is only searched for as a word of its own
        if secret == USER {
            assert!(
                !text
                    .split(|c: char| !c.is_alphanumeric())
                    .any(|w| w == USER),
                "{}",
                text
            );
        } else {
            assert!(!text.contains(secret), "{}", text);
        }
    }
}

#[tokio::test]
async fn an_https_agent_is_reached_through_a_proxy_that_asks_for_a_login() {
    let work = tempfile::tempdir().unwrap();
    let agent = weather_over_tls(work.path()).await;
    let p = proxy::start(Mode::Login(USER.into(), PASSWORD.into())).await;
    let rt = runtime_with(work.path(), &login_env(), |c| {
        with_login(c, &p.url());
        c.network.ca_file = Some(agent.ca.clone());
    });
    let reply = run_source(
        &rt,
        &trip(&agent.server.base),
        "plan",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap();
    assert_eq!(reply.to_display(), "Bob says: sunny in Lisbon");
    let log = p.log.lines();
    assert!(!log.is_empty(), "the proxy was never used");
    for line in &log {
        assert_eq!(
            line,
            &format!("CONNECT localhost:{} ok", agent.port),
            "{:?}",
            log
        );
    }
}

#[tokio::test]
async fn a_tool_server_and_a_web_address_over_https_use_the_proxy_too() {
    let work = tempfile::tempdir().unwrap();
    let (ca_pem, cert) = tls_server::signed_by_new_authority(&["localhost"]);
    let ca = work.path().join("ca.pem");
    std::fs::write(&ca, ca_pem).unwrap();
    let mcp = crate::common::fake_mcp::start_tls(&cert).await;
    let web =
        tls_server::serve_tls(Router::new().route("/hi", get(|| async { "hello" })), &cert).await;
    let p = proxy::start(Mode::Login(USER.into(), PASSWORD.into())).await;
    let rt = runtime_with(work.path(), &login_env(), |c| {
        with_login(c, &p.url());
        c.network.ca_file = Some(ca.clone());
    });
    let weather = project_file("examples/weather.ag").replace("npx -y weather-mcp", &mcp.url);
    let reply = run_source(&rt, &weather, "ask", &[("city", "Lisbon")])
        .await
        .unwrap();
    assert_eq!(reply, Value::text("In Lisbon it will be sunny, 24 degrees"));
    let reply = run_source(
        &rt,
        &web_agent(&format!("https://localhost:{}/hi", web)),
        "go",
        &[],
    )
    .await
    .unwrap();
    assert_eq!(reply.to_display(), "hello");
    let log = p.log.lines();
    assert!(
        log.iter()
            .any(|l| l.contains(&mcp.url.split('/').nth(2).unwrap().to_string())),
        "{:?}",
        log
    );
    assert!(
        log.iter()
            .any(|l| l.contains(&format!("localhost:{}", web))),
        "{:?}",
        log
    );
}

/// A plain web server that remembers the headers it saw.
async fn plain_server() -> (u16, Arc<Mutex<Vec<HeaderMap>>>) {
    let seen: Arc<Mutex<Vec<HeaderMap>>> = Default::default();
    let state = seen.clone();
    let app = Router::new().route(
        "/hi",
        get(move |headers: HeaderMap| {
            let state = state.clone();
            async move {
                state.lock().unwrap().push(headers);
                "plain hello"
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (port, seen)
}

#[tokio::test]
async fn a_plain_http_address_goes_through_the_proxy_and_the_login_stays_with_the_proxy() {
    let (port, seen) = plain_server().await;
    let work = tempfile::tempdir().unwrap();
    let p = proxy::start(Mode::Login(USER.into(), PASSWORD.into())).await;
    let rt = runtime_with(work.path(), &login_env(), |c| with_login(c, &p.url()));
    let reply = run_source(
        &rt,
        &web_agent(&format!("http://127.0.0.1:{}/hi", port)),
        "go",
        &[],
    )
    .await
    .unwrap();
    assert_eq!(reply.to_display(), "plain hello");
    assert_eq!(p.log.lines(), vec![format!("GET 127.0.0.1:{} ok", port)]);
    for headers in seen.lock().unwrap().iter() {
        assert!(
            headers.get("proxy-authorization").is_none(),
            "the login reached the destination"
        );
        assert!(headers.get("authorization").is_none());
    }
}

async fn refused_by(mode: Mode, env: &[(&str, &str)]) -> (String, TestProxy) {
    let work = tempfile::tempdir().unwrap();
    let agent = weather_over_tls(work.path()).await;
    let p = proxy::start(mode).await;
    let rt = runtime_with(work.path(), env, |c| {
        with_login(c, &p.url());
        c.network.ca_file = Some(agent.ca.clone());
    });
    let err = run_source(
        &rt,
        &trip(&agent.server.base),
        "plan",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap_err()
    .render();
    (err, p)
}

#[tokio::test]
async fn a_wrong_login_says_the_proxy_refused_it_and_what_to_check() {
    let mut env = login_env();
    env[1] = ("PROXY_PASSWORD", "not-the-password");
    let (err, p) = refused_by(Mode::Login(USER.into(), PASSWORD.into()), &env).await;
    assert!(err.contains("refused the login"), "{}", err);
    assert!(err.contains(&p.url()), "{}", err);
    assert!(
        err.contains("username_env") && err.contains("password_env"),
        "{}",
        err
    );
    assert_no_secret(&err);
    assert!(!err.contains("not-the-password"), "{}", err);
}

#[tokio::test]
async fn a_proxy_that_wants_another_kind_of_login_says_which() {
    let (err, _) = refused_by(Mode::OfferScheme("NTLM"), &login_env()).await;
    assert!(err.contains("NTLM"), "{}", err);
    assert!(err.contains("supports only Basic"), "{}", err);
}

#[tokio::test]
async fn a_proxy_that_refuses_the_tunnel_says_what_it_answered() {
    let (err, _) = refused_by(Mode::Status(403), &login_env()).await;
    assert!(err.contains("refused to open a connection"), "{}", err);
    assert!(err.contains("403"), "{}", err);
}

#[tokio::test]
async fn a_proxy_that_cannot_be_reached_says_the_destination_was_never_contacted() {
    let work = tempfile::tempdir().unwrap();
    let agent = weather_over_tls(work.path()).await;
    let rt = runtime_with(work.path(), &login_env(), |c| {
        with_login(c, "http://127.0.0.1:1");
        c.network.ca_file = Some(agent.ca.clone());
    });
    let err = run_source(
        &rt,
        &trip(&agent.server.base),
        "plan",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap_err()
    .render();
    assert!(
        err.contains("could not reach the proxy http://127.0.0.1:1"),
        "{}",
        err
    );
    assert!(err.contains("was never contacted"), "{}", err);
}

#[tokio::test]
async fn a_proxy_that_never_answers_is_reported_as_such() {
    let (err, _) = refused_by(Mode::Silent, &login_env()).await;
    assert!(err.contains("did not answer in time"), "{}", err);
}

#[tokio::test]
async fn hosts_in_no_proxy_go_direct() {
    let work = tempfile::tempdir().unwrap();
    let agent = weather_over_tls(work.path()).await;
    // a proxy that would refuse everything proves that it was not used
    let p = proxy::start(Mode::Status(403)).await;
    let rt = runtime_with(work.path(), &login_env(), |c| {
        with_login(c, &p.url());
        c.network.proxy.no_proxy = vec!["localhost".to_string()];
        c.network.ca_file = Some(agent.ca.clone());
    });
    let reply = run_source(
        &rt,
        &trip(&agent.server.base),
        "plan",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap();
    assert_eq!(reply.to_display(), "Bob says: sunny in Lisbon");
    assert!(p.log.lines().is_empty(), "{:?}", p.log.lines());
}

#[tokio::test]
async fn no_message_and_no_log_ever_holds_the_login() {
    let mut env = login_env();
    env[1] = ("PROXY_PASSWORD", PASSWORD);
    let (err, p) = refused_by(Mode::Login("someone".into(), "else".into()), &env).await;
    assert_no_secret(&err);
    assert_no_secret(&p.log.lines().join("\n"));
}

// ---------- the proxy from the environment ----------

#[tokio::test]
async fn the_environment_proxy_is_used_when_the_toml_has_none() {
    let work = tempfile::tempdir().unwrap();
    let agent = weather_over_tls(work.path()).await;
    let p = proxy::start(Mode::Open).await;
    for name in ["HTTPS_PROXY", "https_proxy"] {
        p.log.0.lock().unwrap().clear();
        let rt = runtime_with(work.path(), &[(name, &p.url())], |c| {
            c.network.ca_file = Some(agent.ca.clone())
        });
        let reply = run_source(
            &rt,
            &trip(&agent.server.base),
            "plan",
            &[("city", "Lisbon")],
        )
        .await
        .unwrap();
        assert_eq!(reply.to_display(), "Bob says: sunny in Lisbon");
        assert!(!p.log.lines().is_empty(), "{} was not used", name);
    }
}

#[tokio::test]
async fn http_proxy_is_for_http_addresses_and_no_proxy_from_the_environment_skips_it() {
    let (port, _) = plain_server().await;
    let work = tempfile::tempdir().unwrap();
    let p = proxy::start(Mode::Open).await;
    let address = format!("http://127.0.0.1:{}/hi", port);
    let rt = runtime_with(work.path(), &[("HTTP_PROXY", &p.url())], |_| {});
    run_source(&rt, &web_agent(&address), "go", &[])
        .await
        .unwrap();
    assert_eq!(p.log.lines(), vec![format!("GET 127.0.0.1:{} ok", port)]);

    p.log.0.lock().unwrap().clear();
    let rt = runtime_with(
        work.path(),
        &[("http_proxy", &p.url()), ("NO_PROXY", "127.0.0.1")],
        |_| {},
    );
    run_source(&rt, &web_agent(&address), "go", &[])
        .await
        .unwrap();
    assert!(p.log.lines().is_empty(), "{:?}", p.log.lines());
}

#[tokio::test]
async fn the_toml_proxy_wins_and_the_environment_is_ignored_completely() {
    let work = tempfile::tempdir().unwrap();
    let agent = weather_over_tls(work.path()).await;
    let toml_proxy = proxy::start(Mode::Open).await;
    let env_proxy = proxy::start(Mode::Open).await;
    let rt = runtime_with(
        work.path(),
        // the environment even says "no proxy for localhost", which must be ignored too
        &[("HTTPS_PROXY", &env_proxy.url()), ("NO_PROXY", "localhost")],
        |c| {
            c.network.proxy.url = Some(toml_proxy.url());
            c.network.ca_file = Some(agent.ca.clone());
        },
    );
    run_source(
        &rt,
        &trip(&agent.server.base),
        "plan",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap();
    assert!(!toml_proxy.log.lines().is_empty());
    assert!(
        env_proxy.log.lines().is_empty(),
        "{:?}",
        env_proxy.log.lines()
    );
}

#[tokio::test]
async fn credentials_in_the_environment_address_are_used_and_not_shown() {
    let work = tempfile::tempdir().unwrap();
    let agent = weather_over_tls(work.path()).await;
    let p = proxy::start(Mode::Login("bob".into(), "s3cret".into())).await;
    let with_creds = p.url().replace("http://", "http://bob:s3cret@");
    let rt = runtime_with(work.path(), &[("HTTPS_PROXY", &with_creds)], |c| {
        c.network.ca_file = Some(agent.ca.clone())
    });
    run_source(
        &rt,
        &trip(&agent.server.base),
        "plan",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap();
    assert!(p.log.lines().iter().all(|l| !l.contains("s3cret")));
    // and a wrong one is refused without repeating it
    let wrong = p.url().replace("http://", "http://bob:wrong-one@");
    let rt = runtime_with(work.path(), &[("HTTPS_PROXY", &wrong)], |c| {
        c.network.ca_file = Some(agent.ca.clone())
    });
    let err = run_source(
        &rt,
        &trip(&agent.server.base),
        "plan",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap_err()
    .render();
    assert!(err.contains("refused the login"), "{}", err);
    assert!(
        !err.contains("wrong-one") && !err.contains("bob:"),
        "{}",
        err
    );
}

// ---------- a proxy that is itself reached over https ----------

#[tokio::test]
async fn a_proxy_with_a_certificate_of_its_own_needs_trust_like_any_server() {
    let work = tempfile::tempdir().unwrap();
    let agent = weather_over_tls(work.path()).await;
    let proxy_cert = tls_server::self_signed(&["127.0.0.1"]);
    let p = proxy::start_tls(Mode::Open, &proxy_cert).await;

    // by default: refused, and the message does not blame the wrong server
    let rt = runtime_with(work.path(), &[], |c| {
        c.network.proxy.url = Some(p.url());
        c.network.ca_file = Some(agent.ca.clone());
    });
    let err = run_source(
        &rt,
        &trip(&agent.server.base),
        "plan",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap_err()
    .render();
    assert!(err.contains("could not trust a certificate"), "{}", err);
    assert!(err.contains(&p.url()), "{}", err);
    assert!(
        err.contains("allow_self_signed") && err.contains("ca_file"),
        "{}",
        err
    );

    // allow_self_signed covers the proxy connection itself
    let rt = runtime_with(work.path(), &[], |c| {
        c.network.proxy.url = Some(p.url());
        c.network.ca_file = Some(agent.ca.clone());
        c.network.allow_self_signed = true;
    });
    let reply = run_source(
        &rt,
        &trip(&agent.server.base),
        "plan",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap();
    assert_eq!(reply.to_display(), "Bob says: sunny in Lisbon");

    // so does trusting the proxy's certificate with ca_file, with checks still on
    let both = work.path().join("both.pem");
    std::fs::write(
        &both,
        format!(
            "{}\n{}",
            std::fs::read_to_string(&agent.ca).unwrap(),
            proxy_cert.cert_pem
        ),
    )
    .unwrap();
    let rt = runtime_with(work.path(), &[], |c| {
        c.network.proxy.url = Some(p.url());
        c.network.ca_file = Some(both.clone());
    });
    let reply = run_source(
        &rt,
        &trip(&agent.server.base),
        "plan",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap();
    assert_eq!(reply.to_display(), "Bob says: sunny in Lisbon");
}
