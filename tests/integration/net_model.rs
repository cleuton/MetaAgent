// 0.1.3: new file. The language model has its own certificate switch, and nothing else shares it.
use crate::common::proxy::{self, Mode};
use crate::common::tls_server::{self, TestCert};
use crate::common::*;
use axum::{
    Json, Router,
    routing::{get, post},
};
use metagente::llm::providers::from_config;
use metagente::llm::{ChatMsg, LlmReply, LlmRequest};
use metagente::runtime::config::Config;
use serde_json::json;

const KEY: &str = "MG_NET_MODEL_KEY";
const KEY_VALUE: &str = "sk-very-secret-model-key";

fn request() -> LlmRequest {
    LlmRequest {
        system: "test".to_string(),
        messages: vec![ChatMsg::User("hi".to_string())],
        tools: vec![],
    }
}

/// A model gateway that answers over https with this certificate. Returns its address.
async fn gateway(cert: &TestCert) -> String {
    let app = Router::new().route(
        "/v1/messages",
        post(|| async {
            Json(json!({"content": [{"type": "text", "text": "hello from the gateway"}]}))
        }),
    );
    let port = tls_server::serve_tls(app, cert).await;
    format!("https://127.0.0.1:{}", port)
}

fn config_for(base: &str) -> Config {
    unsafe { std::env::set_var(KEY, KEY_VALUE) };
    let mut c = Config::default();
    c.llm.provider = Some("anthropic".to_string());
    c.llm.model = Some("test-model".to_string());
    c.llm.base_url = Some(base.to_string());
    c.llm.api_key_env = Some(KEY.to_string());
    c
}

async fn ask_model(config: &Config) -> Result<LlmReply, String> {
    let llm = from_config(config).unwrap().unwrap();
    llm.complete(&request()).await
}

fn web_agent(url: &str) -> String {
    format!(
        "agent Web\n  goal \"w\"\n  tool http\n  accepts go\n  on go\n    x = http.get url: \"{}\"\n    reply x.text\n",
        url
    )
}

#[tokio::test]
async fn allow_self_signed_does_not_open_the_model_connection_and_the_message_names_the_right_switch()
 {
    let cert = tls_server::self_signed(&["127.0.0.1"]);
    let base = gateway(&cert).await;
    let mut config = config_for(&base);
    config.network.allow_self_signed = true;
    let err = ask_model(&config).await.unwrap_err();
    assert!(err.contains("llm_allow_self_signed"), "{}", err);
    assert!(
        err.contains("allow_self_signed does not apply to the language model"),
        "{}",
        err
    );
    assert!(!err.contains(KEY_VALUE), "{}", err);
    for word in ["rustls", "UnknownIssuer", "reqwest"] {
        assert!(!err.contains(word), "{}", err);
    }

    // agents and tools still accept a self-signed server under the same settings
    let web =
        tls_server::serve_tls(Router::new().route("/hi", get(|| async { "hello" })), &cert).await;
    let dir = tempfile::tempdir().unwrap();
    let rt = metagente::runtime::Runtime::new(config, None);
    let reply = run_source(
        &rt,
        &web_agent(&format!("https://127.0.0.1:{}/hi", web)),
        "go",
        &[],
    )
    .await
    .unwrap();
    assert_eq!(reply.to_display(), "hello");
    drop(dir);
}

#[tokio::test]
async fn llm_allow_self_signed_opens_only_the_model_connection() {
    let cert = tls_server::self_signed(&["127.0.0.1"]);
    let base = gateway(&cert).await;
    let mut config = config_for(&base);
    config.network.llm_allow_self_signed = true;
    assert_eq!(
        ask_model(&config).await.unwrap(),
        LlmReply::Text("hello from the gateway".to_string())
    );

    // an agent or tool call to another self-signed server is still refused
    let web =
        tls_server::serve_tls(Router::new().route("/hi", get(|| async { "hello" })), &cert).await;
    let rt = metagente::runtime::Runtime::new(config, None);
    let err = run_source(
        &rt,
        &web_agent(&format!("https://127.0.0.1:{}/hi", web)),
        "go",
        &[],
    )
    .await
    .unwrap_err()
    .render();
    assert!(err.contains("could not trust the certificate"), "{}", err);
}

#[tokio::test]
async fn a_host_list_for_agents_does_not_change_the_model_rule() {
    let cert = tls_server::self_signed(&["127.0.0.1"]);
    let base = gateway(&cert).await;
    let mut config = config_for(&base);
    config.network.llm_allow_self_signed = true;
    config.network.self_signed_hosts = vec!["somewhere.else".to_string()];
    assert!(ask_model(&config).await.is_ok());
}

#[tokio::test]
async fn ca_file_applies_to_the_model_connection_with_checks_still_on() {
    let (ca_pem, cert) = tls_server::signed_by_new_authority(&["127.0.0.1"]);
    let base = gateway(&cert).await;
    let dir = tempfile::tempdir().unwrap();
    let ca = dir.path().join("ca.pem");
    std::fs::write(&ca, ca_pem).unwrap();
    let mut config = config_for(&base);
    assert!(ask_model(&config).await.is_err());
    config.network.ca_file = Some(ca);
    assert_eq!(
        ask_model(&config).await.unwrap(),
        LlmReply::Text("hello from the gateway".to_string())
    );
}

#[tokio::test]
async fn the_model_call_goes_through_the_proxy_like_every_other_call() {
    let cert = tls_server::self_signed(&["127.0.0.1"]);
    let base = gateway(&cert).await;
    let p = proxy::start(Mode::Open).await;
    let mut config = config_for(&base);
    config.network.llm_allow_self_signed = true;
    config.network.proxy.url = Some(p.url());
    assert!(ask_model(&config).await.is_ok());
    let log = p.log.lines();
    assert_eq!(log.len(), 1, "{:?}", log);
    assert!(
        log[0].starts_with(&format!("CONNECT {}", base.trim_start_matches("https://"))),
        "{:?}",
        log
    );
}
