//! Every kind of problem must read as plain language: where, what, and how to fix it.

use metagente::diagnostics::Diagnostic;
use metagente::lang::check::check_all;
use metagente::lang::parse_file;
use metagente::runtime::Runtime;
use metagente::runtime::config::Config;
use metagente::runtime::interpreter::Agent;
use metagente::runtime::task::{CallContext, new_task_id};
use metagente::runtime::value::Value;
use std::sync::Arc;

const FORBIDDEN: &[&str] = &[
    "unwrap",
    "panicked",
    "RUST_BACKTRACE",
    "Result<",
    "Option<",
    "Some(",
    "Err(",
    "::",
    ".rs:",
    "src/",
    "thread '",
    "stack backtrace",
    "Box<",
    "Arc<",
    "std::",
];

fn assert_plain(rendered: &str) {
    for word in FORBIDDEN {
        assert!(
            !rendered.contains(word),
            "found `{}` in:\n{}",
            word,
            rendered
        );
    }
}

fn located(d: &Diagnostic) -> String {
    let text = d.render();
    assert!(
        text.starts_with("Problem on line "),
        "no line reference:\n{}",
        text
    );
    assert!(text.contains("\nFix: "), "no suggested fix:\n{}", text);
    assert_plain(&text);
    text
}

fn problems(source: &str) -> Vec<Diagnostic> {
    match parse_file("golden.ag", None, source) {
        Ok(agents) => check_all(&agents.into_iter().map(Arc::new).collect::<Vec<_>>()),
        Err(d) => vec![d],
    }
}

#[test]
fn syntax_error() {
    let text = located(&problems("agent A\n  gaol \"x\"\n")[0]);
    assert!(text.contains("did you mean `goal`?"), "{}", text);
}

#[test]
fn undeclared_name() {
    let text =
        located(&problems("agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply nme\n")[0]);
    assert!(
        text.contains("I do not know what `nme` is here"),
        "{}",
        text
    );
}

#[test]
fn undeclared_tool() {
    let text = located(
        &problems(
            "agent A\n  goal \"x\"\n  accepts go\n  on go\n    x = file.read path: \"a\"\n    reply x\n",
        )[0],
    );
    assert!(text.contains("never declared it"), "{}", text);
    assert!(text.contains("tool file"), "{}", text);
}

#[test]
fn bad_parameter_for_a_builtin_tool() {
    let text = located(
        &problems(
            "agent A\n  goal \"x\"\n  tool file\n  accepts go\n  on go\n    x = file.read pth: \"a\"\n    reply x\n",
        )[0],
    );
    assert!(text.contains("needs a value for `path`"), "{}", text);
}

#[test]
fn unknown_action_of_a_builtin_tool() {
    let text = located(
        &problems(
            "agent A\n  goal \"x\"\n  tool file\n  accepts go\n  on go\n    x = file.reed path: \"a\"\n    reply x\n",
        )[0],
    );
    assert!(text.contains("did you mean `file.read`?"), "{}", text);
}

#[test]
fn handler_without_accepts_and_accepts_without_handler() {
    let all = problems("agent A\n  goal \"x\"\n  accepts one\n  on two\n    reply \"x\"\n");
    assert_eq!(all.len(), 2);
    for d in &all {
        located(d);
    }
}

#[test]
fn missing_goal() {
    let text = located(&problems("agent A\n  accepts go\n  on go\n    reply \"x\"\n")[0]);
    assert!(text.contains("has no goal"), "{}", text);
}

#[tokio::test]
async fn unknown_message_and_missing_or_extra_values() {
    let dir = tempfile::tempdir().unwrap();
    let rt = Runtime::new(
        Config {
            root: dir.path().to_path_buf(),
            ..Config::default()
        },
        None,
    );
    let source = "agent A\n  goal \"x\"\n  accepts ask city\n  on ask\n    reply city\n";
    let def = Arc::new(parse_file("golden.ag", None, source).unwrap().remove(0));
    let agent = Agent::new(rt, def).unwrap();
    let ctx = CallContext::new(new_task_id());
    let no_args = std::collections::BTreeMap::new();
    let mut wrong = std::collections::BTreeMap::new();
    wrong.insert("cty".to_string(), Value::text("x"));
    for (message, args, expected) in [
        ("aks", no_args.clone(), "did you mean `ask`?"),
        ("ask", no_args, "needs a value for `city`"),
        ("ask", wrong, "needs a value for `city`"),
    ] {
        let text = agent
            .handle(message, args, &ctx)
            .await
            .unwrap_err()
            .render();
        assert!(text.contains(expected), "{}", text);
        assert!(text.contains("Fix: "), "{}", text);
        assert_plain(&text);
    }
}

#[tokio::test]
async fn missing_language_model() {
    let dir = tempfile::tempdir().unwrap();
    let rt = Runtime::new(
        Config {
            root: dir.path().to_path_buf(),
            ..Config::default()
        },
        None,
    );
    let source = "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply think \"hi\"\n";
    let def = Arc::new(parse_file("golden.ag", None, source).unwrap().remove(0));
    let agent = Agent::new(rt, def).unwrap();
    let d = agent
        .handle("go", Default::default(), &CallContext::new(new_task_id()))
        .await
        .unwrap_err();
    let text = located(&d);
    assert!(text.contains("needs a language model"), "{}", text);
}

#[tokio::test]
async fn runtime_errors_point_at_the_line() {
    let dir = tempfile::tempdir().unwrap();
    let rt = Runtime::new(
        Config {
            root: dir.path().to_path_buf(),
            ..Config::default()
        },
        None,
    );
    let source = "agent A\n  goal \"x\"\n  tool file\n  accepts go\n  on go\n    x = file.read path: \"nope.txt\"\n    reply x\n";
    let def = Arc::new(parse_file("golden.ag", None, source).unwrap().remove(0));
    let agent = Agent::new(rt, def).unwrap();
    let d = agent
        .handle("go", Default::default(), &CallContext::new(new_task_id()))
        .await
        .unwrap_err();
    let text = located(&d);
    assert!(text.contains("line 6"), "{}", text);
    assert!(text.contains("does not exist"), "{}", text);
}
