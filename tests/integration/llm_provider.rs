use crate::common::web;
use crate::common::*;
use metagente::llm::providers::from_config;
use metagente::llm::{ChatMsg, LlmReply, LlmRequest, ToolSpec};
use serde_json::json;

fn request() -> LlmRequest {
    LlmRequest {
        system: "You are a test".to_string(),
        messages: vec![ChatMsg::User("hi".to_string())],
        tools: vec![ToolSpec {
            name: "clock__now".to_string(),
            description: "the time".to_string(),
            schema: json!({"type": "object", "properties": {}}),
        }],
    }
}

fn config(provider: &str, base: &str, key_var: &str) -> metagente::runtime::config::Config {
    let mut c = metagente::runtime::config::Config::default();
    c.llm.provider = Some(provider.to_string());
    c.llm.model = Some("test-model".to_string());
    c.llm.base_url = Some(base.to_string());
    c.llm.api_key_env = Some(key_var.to_string());
    c
}

#[tokio::test]
async fn anthropic_request_shape_and_key_from_the_named_variable() {
    let web = web::start(json!({"content": [{"type": "text", "text": "hello there"}]})).await;
    unsafe { std::env::set_var("MG_TEST_KEY_ANTHROPIC", "secret-a") };
    let llm = from_config(&config("anthropic", &web.base, "MG_TEST_KEY_ANTHROPIC"))
        .unwrap()
        .unwrap();
    let reply = llm.complete(&request()).await.unwrap();
    assert_eq!(reply, LlmReply::Text("hello there".to_string()));
    let seen = web.seen.lock().unwrap();
    assert_eq!(seen.paths, vec!["/v1/messages"]);
    assert_eq!(seen.headers[0].get("x-api-key").unwrap(), "secret-a");
    assert!(seen.headers[0].get("anthropic-version").is_some());
    let body = &seen.bodies[0];
    assert_eq!(body["model"], "test-model");
    assert_eq!(body["system"], "You are a test");
    assert_eq!(body["messages"][0]["content"], "hi");
    assert_eq!(body["tools"][0]["name"], "clock__now");
    assert!(body["tools"][0]["input_schema"].is_object());
}

#[tokio::test]
async fn anthropic_tool_use_becomes_a_tool_call() {
    let web = web::start(
        json!({"content": [{"type": "tool_use", "id": "t1", "name": "clock__now", "input": {}}]}),
    )
    .await;
    unsafe { std::env::set_var("MG_TEST_KEY_ANTHROPIC2", "k") };
    let llm = from_config(&config("anthropic", &web.base, "MG_TEST_KEY_ANTHROPIC2"))
        .unwrap()
        .unwrap();
    match llm.complete(&request()).await.unwrap() {
        LlmReply::ToolCall(c) => {
            assert_eq!(c.id, "t1");
            assert_eq!(c.name, "clock__now");
        }
        other => panic!("{:?}", other),
    }
}

#[tokio::test]
async fn openai_compatible_request_shape_and_bearer_key() {
    let web = web::start(json!({"choices": [{"message": {"content": "hi from gpt"}}]})).await;
    unsafe { std::env::set_var("MG_TEST_KEY_OPENAI", "secret-o") };
    let llm = from_config(&config(
        "openai-compatible",
        &web.base,
        "MG_TEST_KEY_OPENAI",
    ))
    .unwrap()
    .unwrap();
    let reply = llm.complete(&request()).await.unwrap();
    assert_eq!(reply, LlmReply::Text("hi from gpt".to_string()));
    let seen = web.seen.lock().unwrap();
    assert_eq!(seen.paths, vec!["/v1/chat/completions"]);
    assert_eq!(
        seen.headers[0].get("authorization").unwrap(),
        "Bearer secret-o"
    );
    assert_eq!(seen.bodies[0]["messages"][0]["role"], "system");
    assert_eq!(seen.bodies[0]["tools"][0]["function"]["name"], "clock__now");
}

#[tokio::test]
async fn a_missing_key_variable_is_a_plain_message() {
    let web = web::start(json!({})).await;
    let llm = from_config(&config(
        "anthropic",
        &web.base,
        "MG_TEST_KEY_NOT_SET_ANYWHERE",
    ))
    .unwrap()
    .unwrap();
    let err = llm.complete(&request()).await.unwrap_err();
    assert!(
        err.contains("MG_TEST_KEY_NOT_SET_ANYWHERE is not set"),
        "{}",
        err
    );
}

#[test]
fn unknown_provider_and_missing_model_are_explained() {
    let mut c = metagente::runtime::config::Config::default();
    c.llm.provider = Some("mystery".to_string());
    let e = from_config(&c).err().unwrap().render();
    assert!(e.contains("mystery"), "{}", e);
    c.llm.provider = Some("openai-compatible".to_string());
    let e = from_config(&c).err().unwrap().render();
    assert!(e.contains("needs a model name"), "{}", e);
    c.llm.provider = None;
    assert!(from_config(&c).unwrap().is_none());
}

#[tokio::test]
async fn think_uses_the_configured_provider_without_naming_one_in_the_agent_file() {
    let web = web::start(json!({"content": [{"type": "text", "text": "four"}]})).await;
    unsafe { std::env::set_var("MG_TEST_KEY_THINK", "k") };
    let cfg = config("anthropic", &web.base, "MG_TEST_KEY_THINK");
    let llm = from_config(&cfg).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), llm, |_| {});
    let source = "agent Math\n  goal \"Do arithmetic\"\n  accepts ask\n  on ask\n    reply think \"what is 2 + 2\"\n";
    assert!(!source.contains("anthropic"));
    let reply = run_source(&rt, source, "ask", &[]).await.unwrap();
    assert_eq!(reply.to_display(), "four");
    assert!(
        web.seen.lock().unwrap().bodies[0]["system"]
            .as_str()
            .unwrap()
            .contains("Do arithmetic")
    );
}
