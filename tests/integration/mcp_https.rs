// 0.1.3: new file. A tool server reached over https: discovery, argument checks and calls all work over TLS.
use crate::common::tls_server;
use crate::common::*;
use metagente::runtime::value::Value;

fn weather_source(url: &str) -> String {
    project_file("examples/weather.ag").replace("npx -y weather-mcp", url)
}

#[tokio::test]
async fn a_tool_over_https_works_when_its_authority_is_trusted() {
    let (ca_pem, cert) = tls_server::signed_by_new_authority(&["localhost"]);
    let server = crate::common::fake_mcp::start_tls(&cert).await;
    let dir = tempfile::tempdir().unwrap();
    let ca = dir.path().join("ca.pem");
    std::fs::write(&ca, ca_pem).unwrap();
    let rt = runtime_in(dir.path(), None, |c| c.network.ca_file = Some(ca.clone()));
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
async fn a_self_signed_tool_server_is_refused_by_default_in_plain_words() {
    let cert = tls_server::self_signed(&["localhost"]);
    let server = crate::common::fake_mcp::start_tls(&cert).await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let err = run_source(
        &rt,
        &weather_source(&server.url),
        "ask",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap_err()
    .render();
    assert!(
        err.contains("could not trust the certificate of"),
        "{}",
        err
    );
    assert!(err.contains(&server.url), "{}", err);
    assert!(
        err.contains("ca_file") && err.contains("allow_self_signed"),
        "{}",
        err
    );
    for word in ["rustls", "UnknownIssuer", "reqwest", "hyper"] {
        assert!(!err.contains(word), "{}", err);
    }
}

#[tokio::test]
async fn a_self_signed_tool_server_works_with_allow_self_signed() {
    let cert = tls_server::self_signed(&["localhost"]);
    let server = crate::common::fake_mcp::start_tls(&cert).await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |c| c.network.allow_self_signed = true);
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
