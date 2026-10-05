use crate::common::*;
use metagente::lang::parse_file;

fn source(names: &str) -> String {
    format!(
        "agent E\n  goal \"x\"\n  tool env {}\n  accepts get name\n  on get\n    reply env.get name: name\n",
        names
    )
}

#[tokio::test]
async fn only_the_declared_variables_can_be_read() {
    unsafe { std::env::set_var("MG_TEST_ENV_A", "alpha") };
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let agent = agent_from(&rt, &source("\"MG_TEST_ENV_A\""), None);
    assert_eq!(
        ask(&agent, "get", &[("name", "MG_TEST_ENV_A")])
            .await
            .unwrap()
            .to_display(),
        "alpha"
    );
    let err = ask(&agent, "get", &[("name", "PATH")])
        .await
        .unwrap_err()
        .render();
    assert!(
        err.contains("did not declare the variable `PATH`"),
        "{}",
        err
    );
    assert!(
        err.contains("tool env \"MG_TEST_ENV_A\" \"PATH\""),
        "{}",
        err
    );
}

#[tokio::test]
async fn a_declared_variable_that_is_not_set_gives_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let agent = agent_from(&rt, &source("\"MG_TEST_ENV_NOT_SET\""), None);
    assert_eq!(
        ask(&agent, "get", &[("name", "MG_TEST_ENV_NOT_SET")])
            .await
            .unwrap()
            .to_display(),
        "nothing"
    );
}

#[tokio::test]
async fn the_model_key_variable_is_unreadable_even_when_declared() {
    unsafe { std::env::set_var("MG_TEST_KEY_HIDDEN", "very-secret") };
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |c| {
        c.llm.api_key_env = Some("MG_TEST_KEY_HIDDEN".to_string())
    });
    let agent = agent_from(&rt, &source("\"MG_TEST_KEY_HIDDEN\" \"OTHER\""), None);
    let err = ask(&agent, "get", &[("name", "MG_TEST_KEY_HIDDEN")])
        .await
        .unwrap_err()
        .render();
    assert!(
        err.contains("holds the key for the language model"),
        "{}",
        err
    );
    assert!(!err.contains("very-secret"));
}

#[tokio::test]
async fn the_usual_key_variables_are_hidden_without_any_configuration() {
    unsafe { std::env::set_var("ANTHROPIC_API_KEY", "shh") };
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let agent = agent_from(&rt, &source("\"ANTHROPIC_API_KEY\""), None);
    let err = ask(&agent, "get", &[("name", "ANTHROPIC_API_KEY")])
        .await
        .unwrap_err()
        .render();
    assert!(
        err.contains("holds the key for the language model"),
        "{}",
        err
    );
}

#[test]
fn a_bare_tool_env_is_a_syntax_error_with_a_hint() {
    let err = parse_file("e.ag", None, &source("")).unwrap_err().render();
    assert!(err.contains("needs the names of the variables"), "{}", err);
    assert!(err.contains("tool env \"HOME\""), "{}", err);
}
