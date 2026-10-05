use crate::common::*;
use metagente::llm::fake::FakeLlm;
use metagente::llm::{ChatMsg, LlmReply, ToolCallReq};
use serde_json::json;
use std::sync::Arc;

const AGENT: &str = r#"agent Timekeeper
  goal "Tell people the time"
  tool clock
  accepts ask
  on ask
    reply think "what time is it?"
"#;

fn call(name: &str) -> LlmReply {
    LlmReply::ToolCall(ToolCallReq {
        id: "c1".to_string(),
        name: name.to_string(),
        args: json!({}),
    })
}

#[tokio::test]
async fn think_lets_the_model_use_a_declared_tool_then_answers() {
    let fake = Arc::new(FakeLlm::scripted(
        vec![call("clock__now")],
        LlmReply::Text("It is late.".to_string()),
    ));
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), Some(fake.clone()), |_| {});
    let reply = run_source(&rt, AGENT, "ask", &[]).await.unwrap();
    assert_eq!(reply.to_display(), "It is late.");
    let seen = fake.requests();
    assert_eq!(seen.len(), 2);
    // The model was told the goal and offered exactly the declared tool's actions.
    assert!(seen[0].system.contains("Tell people the time"));
    let names: Vec<&str> = seen[0].tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["clock__now", "clock__wait"]);
    // The tool result went back to the model.
    match seen[1].messages.last().unwrap() {
        ChatMsg::ToolResult { content, .. } => assert!(content.contains("unix"), "{}", content),
        other => panic!("{:?}", other),
    }
}

#[tokio::test]
async fn a_model_that_keeps_asking_for_tools_stops_at_the_step_limit() {
    let fake = Arc::new(FakeLlm::always(call("clock__now")));
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), Some(fake.clone()), |c| c.think_max_steps = 3);
    let err = run_source(&rt, AGENT, "ask", &[])
        .await
        .unwrap_err()
        .render();
    assert!(
        err.contains("the agent used 3 steps and did not finish"),
        "{}",
        err
    );
    assert!(err.contains("think_max_steps"), "{}", err);
    assert_eq!(fake.requests().len(), 3);
}

#[tokio::test]
async fn the_default_step_limit_is_ten() {
    let fake = Arc::new(FakeLlm::always(call("clock__now")));
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), Some(fake.clone()), |_| {});
    let err = run_source(&rt, AGENT, "ask", &[])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("used 10 steps"), "{}", err);
}

#[tokio::test]
async fn a_malformed_or_undeclared_tool_request_goes_back_to_the_model_not_the_user() {
    let fake = Arc::new(FakeLlm::scripted(
        vec![call("file__read"), call("nonsense")],
        LlmReply::Text("recovered".to_string()),
    ));
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), Some(fake.clone()), |_| {});
    let reply = run_source(&rt, AGENT, "ask", &[]).await.unwrap();
    assert_eq!(reply.to_display(), "recovered");
    let last = fake.requests().pop().unwrap();
    let results: Vec<String> = last
        .messages
        .iter()
        .filter_map(|m| {
            if let ChatMsg::ToolResult { content, .. } = m {
                Some(content.clone())
            } else {
                None
            }
        })
        .collect();
    assert!(
        results[0].contains("no tool called `file`"),
        "{:?}",
        results
    );
    assert!(results[1].contains("not a tool name"), "{:?}", results);
}

#[tokio::test]
async fn think_without_a_model_says_how_to_set_one_up() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let err = run_source(&rt, AGENT, "ask", &[])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("needs a language model"), "{}", err);
    assert!(err.contains("metagente.toml"), "{}", err);
}
