// 0.1.5: new file (FR-009, SC-005). `using` is an ordinary name everywhere except right after a `think` prompt: a file
// that uses it as a variable, a message value or a parameter keeps working, and the first value after `think` is always
// the prompt, even a variable called `using`.
use crate::common::*;
use metagente::llm::fake::FakeLlm;
use metagente::llm::{ChatMsg, Llm, LlmReply};
use std::process::Command;
use std::sync::Arc;

fn text(t: &str) -> LlmReply {
    LlmReply::Text(t.to_string())
}

#[tokio::test]
async fn using_as_a_variable_name_works_as_before() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let source =
        "agent A\n  goal \"x\"\n  accepts go\n  on go\n    using = \"x\"\n    reply using\n";
    assert_eq!(
        run_source(&rt, source, "go", &[])
            .await
            .unwrap()
            .to_display(),
        "x"
    );
}

#[tokio::test]
async fn using_as_a_message_value_works_as_before() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let source = "agent A\n  goal \"x\"\n  accepts go using\n  on go\n    reply using\n";
    assert_eq!(
        run_source(&rt, source, "go", &[("using", "v")])
            .await
            .unwrap()
            .to_display(),
        "v"
    );
}

#[test]
fn using_as_a_parameter_name_works_as_before() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        "[parameters]\nusing = \"pvalue\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("a.ag"),
        "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply @parameters.using\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_metagente"))
        .current_dir(dir.path())
        .args(["run", "a.ag", "go"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "pvalue");
}

#[tokio::test]
async fn think_using_with_a_variable_called_using_uses_it_as_the_prompt_and_chooses_no_model() {
    let default = Arc::new(FakeLlm::always(text("answer")));
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), Some(default.clone() as Arc<dyn Llm>), |_| {});
    let source = "agent A\n  goal \"x\"\n  accepts go\n  on go\n    using = \"the prompt\"\n    reply think using\n";
    assert_eq!(
        run_source(&rt, source, "go", &[])
            .await
            .unwrap()
            .to_display(),
        "answer"
    );
    match &default.requests()[0].messages[0] {
        ChatMsg::User(t) => assert_eq!(t, "the prompt"),
        other => panic!("{:?}", other),
    }
}

#[tokio::test]
async fn think_using_using_fast_is_the_variable_as_prompt_and_the_model_fast() {
    let fast = Arc::new(FakeLlm::always(text("from fast")));
    let toml = "[llm.fast]\nprovider = \"anthropic\"\nmodel = \"m\"\napi_key_env = \"K\"\n";
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_with_models(
        dir.path(),
        toml,
        None,
        vec![("fast", fast.clone() as Arc<dyn Llm>)],
    );
    let source = "agent A\n  goal \"x\"\n  accepts go\n  on go\n    using = \"hi\"\n    x = think using using fast\n    reply x\n";
    assert_eq!(
        run_source(&rt, source, "go", &[])
            .await
            .unwrap()
            .to_display(),
        "from fast"
    );
    match &fast.requests()[0].messages[0] {
        ChatMsg::User(t) => assert_eq!(t, "hi"),
        other => panic!("{:?}", other),
    }
}

#[test]
fn a_using_with_no_nickname_stops_at_its_line_with_the_plain_message() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("bad.ag"),
        "agent A\n  goal \"x\"\n  accepts go\n  on go\n    x = think \"hi\" using\n    reply x\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_metagente"))
        .current_dir(dir.path())
        .args(["check", "bad.ag"])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(!out.status.success());
    assert!(err.contains("line 5"), "{}", err);
    assert!(
        err.contains("After using I need the name of a model, written as in [llm.NAME]."),
        "{}",
        err
    );
    assert!(
        err.contains("write it like: think \"...\" using fast"),
        "{}",
        err
    );
}
