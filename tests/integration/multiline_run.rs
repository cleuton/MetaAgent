// 0.1.2: new file. A multi-line prompt reaches the model with exactly the characters between the quotes.
use crate::common::*;
use metagente::llm::ChatMsg;
use metagente::llm::LlmReply;
use metagente::llm::fake::FakeLlm;
use std::sync::Arc;

fn first_prompt(fake: &FakeLlm) -> String {
    match &fake.requests()[0].messages[0] {
        ChatMsg::User(text) => text.clone(),
        other => panic!("{:?}", other),
    }
}

// 0.1.2: SC-002, a prompt of 40 lines
#[tokio::test]
async fn a_forty_line_prompt_reaches_the_model_exactly_as_typed() {
    let mut prompt = String::new();
    for n in 1..=40 {
        // different indentation, trailing spaces and an empty line, none of which may be touched
        prompt.push_str(&format!(
            "{}line {} of the prompt {}\n",
            " ".repeat(n % 5),
            n,
            " ".repeat(n % 3)
        ));
        if n % 10 == 0 {
            prompt.push('\n');
        }
    }
    let source = format!(
        "agent Writer\n  goal \"w\"\n  accepts go\n  on go\n    reply think \"\"\"{}\"\"\"\n",
        prompt
    );
    let fake = Arc::new(FakeLlm::always(LlmReply::Text("done".to_string())));
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), Some(fake.clone()), |_| {});
    let reply = run_source(&rt, &source, "go", &[]).await.unwrap();
    assert_eq!(reply.to_display(), "done");
    assert_eq!(first_prompt(&fake), prompt);
}

// 0.1.2: the shipped example, with {resume} filled in
#[tokio::test]
async fn the_multiline_example_fills_in_the_resume() {
    let fake = Arc::new(FakeLlm::always(LlmReply::Text("ok".to_string())));
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), Some(fake.clone()), |_| {});
    let source = project_file("examples/multiline.ag");
    run_source(
        &rt,
        &source,
        "screen",
        &[("resume", "Ada, 10 years of Rust")],
    )
    .await
    .unwrap();
    assert_eq!(
        first_prompt(&fake),
        "You are a technical recruiter.\nRead the resume below and list its three strongest points.\n\nResume:\nAda, 10 years of Rust"
    );
}

// 0.1.2: scenario 2 of the spec, end to end: nothing is trimmed or aligned
#[tokio::test]
async fn the_spec_example_keeps_every_space() {
    let source = "agent A\n  goal \"a\"\n  accepts go\n  on go\n    a = think \"\"\"\nAqui vale fazer isso. \n         E isso também\n  \"\"\"\n    reply a\n";
    let fake = Arc::new(FakeLlm::always(LlmReply::Text("x".to_string())));
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), Some(fake.clone()), |_| {});
    run_source(&rt, source, "go", &[]).await.unwrap();
    assert_eq!(
        first_prompt(&fake),
        "\nAqui vale fazer isso. \n         E isso também\n  "
    );
}
