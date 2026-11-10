// 0.1.5: new file. `think ... using NAME` sends the call to the model of that nickname; a `think` without `using` still
// uses `[llm]`. The models here are scripted (one per nickname), so this proves the routing; the protocol of each
// provider is proved in models_fake_servers.rs.
use crate::common::*;
use metagente::llm::fake::FakeLlm;
use metagente::llm::{ChatMsg, Llm, LlmReply, ToolCallReq};
use serde_json::json;
use std::sync::Arc;

const TWO: &str = r#"[version]
minimum = "0.1.5"

[llm.fast]
provider = "anthropic"
model_env = "FAST_MODEL"
api_key_env = "FAST_KEY"

[llm.strong]
provider = "openai"
model_env = "STRONG_MODEL"
api_key_env = "STRONG_KEY"
"#;

const ASSISTANT: &str = r#"agent Assistant
  goal "Answer a question in two steps"
  accepts ask question
  on ask
    short = think "Rewrite as a short search query: {question}" using fast
    reply think "Answer this well: {short}" using strong
"#;

fn text(t: &str) -> LlmReply {
    LlmReply::Text(t.to_string())
}

fn user(request: &metagente::llm::LlmRequest) -> String {
    match &request.messages[0] {
        ChatMsg::User(t) => t.clone(),
        other => panic!("{:?}", other),
    }
}

#[tokio::test]
async fn the_first_call_goes_to_fast_and_the_second_to_strong() {
    let fast = Arc::new(FakeLlm::always(text("blue sky reason")));
    let strong = Arc::new(FakeLlm::always(text("Rayleigh scattering.")));
    let other = Arc::new(FakeLlm::always(text("the default must not answer")));
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_with_models(
        dir.path(),
        TWO,
        Some(other.clone() as Arc<dyn Llm>),
        vec![
            ("fast", fast.clone() as Arc<dyn Llm>),
            ("strong", strong.clone() as Arc<dyn Llm>),
        ],
    );
    let reply = run_source(
        &rt,
        ASSISTANT,
        "ask",
        &[("question", "why is the sky blue")],
    )
    .await
    .unwrap();
    assert_eq!(reply.to_display(), "Rayleigh scattering.");
    assert_eq!(fast.requests().len(), 1);
    assert_eq!(
        user(&fast.requests()[0]),
        "Rewrite as a short search query: why is the sky blue"
    );
    assert_eq!(strong.requests().len(), 1);
    assert_eq!(
        user(&strong.requests()[0]),
        "Answer this well: blue sky reason"
    );
    assert!(other.requests().is_empty(), "[llm] was not asked");
}

#[tokio::test]
async fn a_think_without_using_still_uses_the_model_of_llm() {
    let fast = Arc::new(FakeLlm::always(text("fast")));
    let default = Arc::new(FakeLlm::always(text("default")));
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_with_models(
        dir.path(),
        TWO,
        Some(default.clone() as Arc<dyn Llm>),
        vec![("fast", fast.clone() as Arc<dyn Llm>)],
    );
    let source = "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply think \"hi\"\n";
    let reply = run_source(&rt, source, "go", &[]).await.unwrap();
    assert_eq!(reply.to_display(), "default");
    assert_eq!(default.requests().len(), 1);
    assert!(fast.requests().is_empty());
}

#[tokio::test]
async fn a_named_model_works_with_no_model_in_llm_and_a_plain_think_says_so() {
    let fast = Arc::new(FakeLlm::always(text("from fast")));
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_with_models(
        dir.path(),
        TWO,
        None,
        vec![("fast", fast.clone() as Arc<dyn Llm>)],
    );
    let using = "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply think \"hi\" using fast\n";
    assert_eq!(
        run_source(&rt, using, "go", &[])
            .await
            .unwrap()
            .to_display(),
        "from fast"
    );
    let plain = "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply think \"hi\"\n";
    let err = run_source(&rt, plain, "go", &[]).await.unwrap_err();
    assert!(
        err.message
            .contains("this agent needs a language model to think, and none is set up"),
        "{}",
        err.message
    );
}

#[tokio::test]
async fn the_model_chosen_by_using_can_use_only_the_declared_tools() {
    let call = LlmReply::ToolCall(ToolCallReq {
        id: "c1".to_string(),
        name: "clock__now".to_string(),
        args: json!({}),
    });
    let fast = Arc::new(FakeLlm::scripted(vec![call], text("done")));
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_with_models(
        dir.path(),
        TWO,
        None,
        vec![("fast", fast.clone() as Arc<dyn Llm>)],
    );
    let with_tool = "agent A\n  goal \"x\"\n  tool clock\n  accepts go\n  on go\n    reply think \"time?\" using fast\n";
    let reply = run_source(&rt, with_tool, "go", &[]).await.unwrap();
    assert_eq!(reply.to_display(), "done");
    let seen = fast.requests();
    assert_eq!(seen.len(), 2);
    let names: Vec<&str> = seen[0].tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["clock__now", "clock__wait"]);
    match seen[1].messages.last().unwrap() {
        ChatMsg::ToolResult { content, .. } => assert!(content.contains("unix"), "{}", content),
        other => panic!("{:?}", other),
    }
    // an agent that declared no tool offers none
    let none = Arc::new(FakeLlm::always(text("ok")));
    let rt = runtime_with_models(
        dir.path(),
        TWO,
        None,
        vec![("fast", none.clone() as Arc<dyn Llm>)],
    );
    let no_tool =
        "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply think \"hi\" using fast\n";
    run_source(&rt, no_tool, "go", &[]).await.unwrap();
    assert!(none.requests()[0].tools.is_empty());
}

#[tokio::test]
async fn using_inside_a_loop_uses_that_model_every_turn() {
    let fast = Arc::new(FakeLlm::always(text("x")));
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_with_models(
        dir.path(),
        TWO,
        None,
        vec![("fast", fast.clone() as Arc<dyn Llm>)],
    );
    let source = "agent A\n  goal \"x\"\n  accepts go\n  on go\n    for item in [\"a\", \"b\", \"c\"]\n      y = think \"{item}\" using fast\n    reply y\n";
    run_source(&rt, source, "go", &[]).await.unwrap();
    assert_eq!(fast.requests().len(), 3);
}

#[tokio::test]
async fn a_nickname_and_llm_of_the_same_provider_each_use_their_own_model() {
    let toml = "[llm]\nprovider = \"anthropic\"\nmodel = \"claude-sonnet-5-5\"\n\n[llm.also]\nprovider = \"anthropic\"\nmodel_env = \"ALSO_MODEL\"\napi_key_env = \"ALSO_KEY\"\n";
    let default = Arc::new(FakeLlm::always(text("default")));
    let also = Arc::new(FakeLlm::always(text("also")));
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_with_models(
        dir.path(),
        toml,
        Some(default.clone() as Arc<dyn Llm>),
        vec![("also", also.clone() as Arc<dyn Llm>)],
    );
    let source = "agent A\n  goal \"x\"\n  accepts go\n  on go\n    a = think \"1\"\n    b = think \"2\" using also\n    reply \"{a} {b}\"\n";
    let reply = run_source(&rt, source, "go", &[]).await.unwrap();
    assert_eq!(reply.to_display(), "default also");
}
