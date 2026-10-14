// 0.1.3: new file. One named test for each cell of the baseline matrix (research.md section 1) that already worked:
// calling over https with a trusted certificate. They stay green when the secure rules are added (SC-011).
use crate::common::a2a::WEATHER;
use crate::common::tls_server;
use crate::common::*;
use axum::{
    Json, Router,
    routing::{get, post},
};
use metagente::llm::providers::from_config;
use metagente::llm::{ChatMsg, LlmRequest};
use metagente::runtime::value::Value;
use serde_json::json;

fn write_ca(dir: &std::path::Path, pem: &str) -> std::path::PathBuf {
    let path = dir.join("ca.pem");
    std::fs::write(&path, pem).unwrap();
    path
}

// rows 1 and 2: A2A client, card fetch and task call
#[tokio::test]
async fn rows_1_and_2_the_a2a_client_fetches_the_card_and_sends_the_task_over_tls() {
    let dir = tempfile::tempdir().unwrap();
    let (ca_pem, cert) = tls_server::signed_by_new_authority_with(&["localhost"], false);
    let server = crate::common::a2a_tls::start(dir.path(), WEATHER, &cert).await;
    let ca = write_ca(dir.path(), &ca_pem);
    let rt = runtime_in(dir.path(), None, |c| c.network.ca_file = Some(ca.clone()));
    let source =
        project_file("examples/remote_weather.ag").replace("http://127.0.0.1:8080", &server.base);
    let reply = run_source(&rt, &source, "plan", &[("city", "Lisbon")])
        .await
        .unwrap();
    assert_eq!(reply.to_display(), "Bob says: sunny in Lisbon");
}

// row 3: MCP HTTP client
#[tokio::test]
async fn row_3_the_mcp_http_client_lists_and_calls_over_tls() {
    let (ca_pem, cert) = tls_server::signed_by_new_authority(&["localhost"]);
    let server = crate::common::fake_mcp::start_tls(&cert).await;
    let dir = tempfile::tempdir().unwrap();
    let ca = write_ca(dir.path(), &ca_pem);
    let rt = runtime_in(dir.path(), None, |c| c.network.ca_file = Some(ca.clone()));
    let source = project_file("examples/weather.ag").replace("npx -y weather-mcp", &server.url);
    let reply = run_source(&rt, &source, "ask", &[("city", "Lisbon")])
        .await
        .unwrap();
    assert_eq!(reply, Value::text("In Lisbon it will be sunny, 24 degrees"));
}

// row 5: the http tool
#[tokio::test]
async fn row_5_the_http_tool_gets_a_page_over_tls() {
    let (ca_pem, cert) = tls_server::signed_by_new_authority(&["localhost"]);
    let port = tls_server::serve_tls(
        Router::new().route("/hi", get(|| async { "hello over tls" })),
        &cert,
    )
    .await;
    let dir = tempfile::tempdir().unwrap();
    let ca = write_ca(dir.path(), &ca_pem);
    let rt = runtime_in(dir.path(), None, |c| c.network.ca_file = Some(ca.clone()));
    let source = format!(
        "agent Web\n  goal \"w\"\n  tool http\n  accepts go\n  on go\n    x = http.get url: \"https://localhost:{}/hi\"\n    reply x.text\n",
        port
    );
    let reply = run_source(&rt, &source, "go", &[]).await.unwrap();
    assert_eq!(reply.to_display(), "hello over tls");
}

// row 6: the model provider
#[tokio::test]
async fn row_6_the_model_provider_calls_a_gateway_over_tls() {
    let (ca_pem, cert) = tls_server::signed_by_new_authority(&["localhost"]);
    let app = Router::new().route(
        "/v1/messages",
        post(|| async {
            Json(json!({"content": [{"type": "text", "text": "hello from the gateway"}]}))
        }),
    );
    let port = tls_server::serve_tls(app, &cert).await;
    unsafe { std::env::set_var("MG_MATRIX_KEY", "sk-test-key") };
    let dir = tempfile::tempdir().unwrap();
    let ca = write_ca(dir.path(), &ca_pem);
    let mut config = metagente::runtime::config::Config::default();
    config.llm.provider = Some("anthropic".to_string());
    config.llm.model = Some("test-model".to_string());
    config.llm.base_url = Some(format!("https://localhost:{}", port));
    config.llm.api_key_env = Some("MG_MATRIX_KEY".to_string());
    config.network.ca_file = Some(ca);
    let llm = from_config(&config).unwrap().unwrap();
    let reply = llm
        .complete(&LlmRequest {
            system: "test".to_string(),
            messages: vec![ChatMsg::User("hi".to_string())],
            tools: vec![],
        })
        .await
        .unwrap();
    assert!(format!("{:?}", reply).contains("hello from the gateway"));
}
