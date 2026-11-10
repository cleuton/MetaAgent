// 0.1.5: new file. Some OpenAI models (the ones that reason by default) refuse a request that has tools with a 400 that
// says `reasoning_effort` must be 'none'. The first request is exactly what 0.1.4 sent; only after that refusal the same
// request is sent once more with `reasoning_effort: "none"`, and the next calls of that model send it at once. Any other
// answer and every other provider are left alone, for `[llm]` and for a named model.
use crate::common::official::{Official, Reply, anthropic_text, openai_text};
use metagente::llm::providers::{from_config, named_from_config};
use metagente::llm::{ChatMsg, Llm, LlmReply, LlmRequest, ToolSpec};
use metagente::runtime::config::Config;
use metagente::runtime::models::read_models;
use metagente::runtime::net::ClientSet;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;

const REFUSAL: &str = "Function tools with reasoning_effort are not supported for gpt-x in /v1/chat/completions. To use function tools, use /v1/responses or set reasoning_effort to 'none'.";

fn with_tool() -> LlmRequest {
    LlmRequest {
        system: "s".to_string(),
        messages: vec![ChatMsg::User("hi".to_string())],
        tools: vec![ToolSpec {
            name: "file__read".to_string(),
            description: "read".to_string(),
            schema: json!({"type": "object", "properties": {}}),
        }],
    }
}

fn set(name: &str, value: &str) {
    unsafe { std::env::set_var(name, value) };
}

fn refusal() -> Reply {
    Reply::Json(400, json!({"error": {"message": REFUSAL}}))
}

fn named_openai(fake: &Official, key_var: &str) -> Arc<dyn Llm> {
    let toml = format!(
        "[llm.gpt]\nprovider = \"openai\"\nmodel = \"gpt-x\"\napi_key_env = \"{}\"\n",
        key_var
    );
    let table: toml::Table = toml.parse().unwrap();
    let mut config = Config {
        models: read_models(
            table["llm"].as_table().unwrap(),
            &toml,
            Path::new("metagente.toml"),
        )
        .unwrap(),
        ..Config::default()
    };
    fake.apply(&mut config);
    let net = Arc::new(ClientSet::for_config(&config));
    named_from_config(&config, net).remove("gpt").unwrap()
}

fn llm_openai(fake: &Official, key_var: &str) -> Arc<dyn Llm> {
    let mut config = Config::default();
    config.llm.provider = Some("openai".to_string());
    config.llm.model = Some("gpt-x".to_string());
    config.llm.api_key_env = Some(key_var.to_string());
    fake.apply(&mut config);
    from_config(&config).unwrap().unwrap()
}

async fn assert_retries_once_and_remembers(fake: &Official, llm: Arc<dyn Llm>) {
    fake.script_openai(vec![refusal(), openai_text("first"), openai_text("later")]);
    assert_eq!(
        llm.complete_detailed(&with_tool()).await.unwrap(),
        LlmReply::Text("first".into())
    );
    let seen = fake.seen_on("/v1/chat/completions");
    assert_eq!(seen.len(), 2, "one refusal and one retry");
    assert!(
        seen[0].body.get("reasoning_effort").is_none(),
        "the first request is the 0.1.4 one"
    );
    assert_eq!(seen[1].body["reasoning_effort"], "none");
    // everything else of the request is the same
    let mut a = seen[0].body.clone();
    let mut b = seen[1].body.clone();
    b.as_object_mut().unwrap().remove("reasoning_effort");
    a.as_object_mut().unwrap().remove("reasoning_effort");
    assert_eq!(a, b);
    // the next call sends it at once: no new refusal
    assert_eq!(
        llm.complete_detailed(&with_tool()).await.unwrap(),
        LlmReply::Text("later".into())
    );
    let seen = fake.seen_on("/v1/chat/completions");
    assert_eq!(seen.len(), 3);
    assert_eq!(seen[2].body["reasoning_effort"], "none");
}

#[tokio::test]
async fn a_named_openai_model_retries_once_with_reasoning_effort_none_and_remembers_it() {
    let fake = Official::start().await;
    set("MG_T_RE_NAMED", "k");
    assert_retries_once_and_remembers(&fake, named_openai(&fake, "MG_T_RE_NAMED")).await;
}

#[tokio::test]
async fn the_openai_model_of_llm_retries_once_with_reasoning_effort_none_and_remembers_it() {
    let fake = Official::start().await;
    set("MG_T_RE_LLM", "k");
    assert_retries_once_and_remembers(&fake, llm_openai(&fake, "MG_T_RE_LLM")).await;
}

#[tokio::test]
async fn any_other_400_is_not_retried_and_keeps_its_0_1_4_message() {
    let fake = Official::start().await;
    set("MG_T_RE_OTHER", "k");
    fake.script_openai(vec![Reply::Json(
        400,
        json!({"error": {"message": "bad model name"}}),
    )]);
    let err = llm_openai(&fake, "MG_T_RE_OTHER")
        .complete(&with_tool())
        .await
        .unwrap_err();
    assert_eq!(
        err,
        "https://api.openai.com/v1/chat/completions answered 400: bad model name"
    );
    assert_eq!(fake.seen().len(), 1, "no retry");
    let err = named_openai(&fake, "MG_T_RE_OTHER")
        .complete_detailed(&with_tool())
        .await
        .unwrap_err();
    assert_eq!(
        err.message,
        "The model gpt (openai) answered with status 400."
    );
    assert_eq!(fake.seen().len(), 2, "no retry");
}

#[tokio::test]
async fn a_second_refusal_after_the_retry_is_an_error_and_never_loops() {
    let fake = Official::start().await;
    set("MG_T_RE_TWICE", "k");
    fake.script_openai(vec![refusal()]);
    let err = named_openai(&fake, "MG_T_RE_TWICE")
        .complete_detailed(&with_tool())
        .await
        .unwrap_err();
    assert_eq!(
        err.message,
        "The model gpt (openai) answered with status 400."
    );
    assert_eq!(fake.seen().len(), 2, "the request and one retry, no more");
    assert!(!err.message.contains("reasoning_effort"));
}

#[tokio::test]
async fn anthropic_is_never_retried() {
    let fake = Official::start().await;
    set("MG_T_RE_AN", "k");
    fake.script_anthropic(vec![
        Reply::Json(400, json!({"error": {"message": REFUSAL}})),
        anthropic_text("x"),
    ]);
    let mut config = Config::default();
    config.llm.provider = Some("anthropic".to_string());
    config.llm.model = Some("m".to_string());
    config.llm.api_key_env = Some("MG_T_RE_AN".to_string());
    fake.apply(&mut config);
    let err = from_config(&config)
        .unwrap()
        .unwrap()
        .complete(&with_tool())
        .await
        .unwrap_err();
    assert!(err.contains("answered 400"), "{}", err);
    assert_eq!(fake.seen().len(), 1);
}
