// 0.1.2: new file. `@parameters.name` reads the [parameters] section of metagente.toml.
use crate::common::*;
use metagente::llm::fake::FakeLlm;
use metagente::llm::{ChatMsg, LlmReply};
use metagente::runtime::Runtime;
use metagente::runtime::config::Config;
use metagente::runtime::run::{RunOptions, run_file};
use metagente::runtime::serve::Served;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

// 0.1.2: a runtime built the way `metagente run` builds it: from the metagente.toml of the folder
fn runtime_from_folder(dir: &Path, llm: Option<Arc<dyn metagente::llm::Llm>>) -> Arc<Runtime> {
    Runtime::new(
        Config::load(dir).unwrap_or_else(|d| panic!("{}", d.render())),
        llm,
    )
}

fn options(file: &Path, message: &str) -> RunOptions {
    RunOptions {
        file: file.to_path_buf(),
        message: Some(message.to_string()),
        params: vec![],
        agent: None,
    }
}

const THINKER: &str =
    "agent A\n  goal \"a\"\n  accepts go resume\n  on go\n    reply think @parameters.prompt1\n";

// 0.1.2: scenario 5
#[tokio::test]
async fn the_model_receives_the_text_of_the_parameter() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        "[parameters]\nprompt1 = \"you are a recruiter...\"\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("a.ag"), THINKER).unwrap();
    let fake = Arc::new(FakeLlm::always(LlmReply::Text("ok".to_string())));
    let rt = runtime_from_folder(dir.path(), Some(fake.clone()));
    let mut opts = options(&dir.path().join("a.ag"), "go");
    opts.params = vec!["resume=unused".to_string()];
    run_file(&opts, rt).await.unwrap();
    match &fake.requests()[0].messages[0] {
        ChatMsg::User(text) => assert_eq!(text, "you are a recruiter..."),
        other => panic!("{:?}", other),
    }
}

// 0.1.2: a {name} in a parameter used as text is filled in (clarification of session 1)
#[tokio::test]
async fn a_name_in_braces_inside_a_parameter_is_filled_in() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        "[parameters]\nprompt1 = \"Read this: {resume}\"\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("a.ag"), THINKER).unwrap();
    let fake = Arc::new(FakeLlm::always(LlmReply::Text("ok".to_string())));
    let rt = runtime_from_folder(dir.path(), Some(fake.clone()));
    let mut opts = options(&dir.path().join("a.ag"), "go");
    opts.params = vec!["resume=Ada".to_string()];
    run_file(&opts, rt).await.unwrap();
    match &fake.requests()[0].messages[0] {
        ChatMsg::User(text) => assert_eq!(text, "Read this: Ada"),
        other => panic!("{:?}", other),
    }
}

// 0.1.2: in a declaration the value is used as it is, braces included
#[test]
fn in_a_declaration_the_value_is_used_as_it_is() {
    let mut params = metagente::lang::Params::default();
    params
        .values
        .insert("g".to_string(), "Answer {anything}".to_string());
    let agents = metagente::lang::parse_file_with(
        "t.ag",
        None,
        "agent A\n  goal @parameters.g\n  accepts go\n  on go\n    reply \"x\"\n",
        &params,
    )
    .unwrap();
    assert_eq!(agents[0].goal.as_ref().unwrap().0, "Answer {anything}");
}

fn metagente(dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_metagente"));
    c.current_dir(dir);
    c
}

// 0.1.2: scenario 6, and `check` reports it without running anything
#[test]
fn a_missing_parameter_is_reported_by_check_and_by_run() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        "[parameters]\nother = \"x\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("a.ag"),
        "agent A\n  goal \"a\"\n  accepts go\n  on go\n    reply @parameters.missing\n",
    )
    .unwrap();
    for command in ["check", "run"] {
        let out = metagente(dir.path())
            .args([command, "a.ag"])
            .output()
            .unwrap();
        assert!(!out.status.success());
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("Problem on line 5"), "{}: {}", command, err);
        assert!(
            err.contains("there is no parameter called `missing`"),
            "{}",
            err
        );
        assert!(
            err.contains("add it under [parameters] in metagente.toml"),
            "{}",
            err
        );
        for word in ["panicked", "unwrap", "src/", "RUST_BACKTRACE"] {
            assert!(!err.contains(word), "{}", err);
        }
    }
}

#[test]
fn a_parameter_without_any_metagente_toml_is_missing_too() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("a.ag"),
        "agent A\n  goal \"a\"\n  accepts go\n  on go\n    reply @parameters.x\n",
    )
    .unwrap();
    let out = metagente(dir.path())
        .args(["check", "a.ag"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("there is no parameter called `x`"));
}

// 0.1.2: a value that is not text says so
#[test]
fn a_parameter_that_is_not_text_is_explained() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        "[parameters]\nretries = 3\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("a.ag"),
        "agent A\n  goal \"a\"\n  accepts go\n  on go\n    reply @parameters.retries\n",
    )
    .unwrap();
    let out = metagente(dir.path())
        .args(["check", "a.ag"])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("the parameter `retries` is not text"),
        "{}",
        err
    );
    assert!(err.contains("Fix: write its value in quotes"), "{}", err);
}

// 0.1.2: backward compatibility: no [parameters], no metagente.toml, and a variable named `parameters`
#[tokio::test]
async fn files_from_0_1_1_behave_the_same() {
    let dir = tempfile::tempdir().unwrap();
    let source =
        "agent A\n  goal \"a\"\n  accepts go parameters\n  on go\n    reply \"got {parameters}\"\n";
    std::fs::write(dir.path().join("a.ag"), source).unwrap();
    for toml in [None, Some("[runtime]\ntimeout_seconds = 30\n")] {
        if let Some(t) = toml {
            std::fs::write(dir.path().join("metagente.toml"), t).unwrap();
        }
        let rt = runtime_from_folder(dir.path(), None);
        assert!(rt.config.warnings.is_empty(), "{:?}", rt.config.warnings);
        let mut opts = options(&dir.path().join("a.ag"), "go");
        opts.params = vec!["parameters=yes".to_string()];
        assert_eq!(run_file(&opts, rt).await.unwrap().to_display(), "got yes");
    }
}

#[test]
fn the_parameters_section_is_not_reported_as_unknown() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        "[parameters]\na = \"1\"\nb = \"2\"\n",
    )
    .unwrap();
    let config = Config::load(dir.path()).unwrap();
    assert!(config.warnings.is_empty(), "{:?}", config.warnings);
    assert_eq!(config.parameters.values.len(), 2);
}

// 0.1.2: FR-010, the rules of 0.1.1 apply to the value of a parameter
#[tokio::test]
async fn the_folder_limit_of_tool_file_applies_to_a_parameter_value() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("data")).unwrap();
    std::fs::write(dir.path().join("data/ok.txt"), "fine").unwrap();
    std::fs::write(dir.path().join("secret.txt"), "top secret").unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        "[parameters]\nfolder = \"data/\"\n",
    )
    .unwrap();
    let source = "agent Scoped\n  goal \"s\"\n  tool file @parameters.folder\n  accepts read path\n  on read\n    reply file.read path: path\n";
    let rt = runtime_from_folder(dir.path(), None);
    let mut params = rt.config.parameters.clone();
    params
        .values
        .insert("folder".to_string(), "data/".to_string());
    let def = metagente::lang::parse_file_with("t.ag", None, source, &params).unwrap();
    let agent =
        metagente::runtime::interpreter::Agent::new(rt, Arc::new(def.into_iter().next().unwrap()))
            .unwrap();
    assert_eq!(
        ask(&agent, "read", &[("path", "ok.txt")])
            .await
            .unwrap()
            .to_display(),
        "fine"
    );
    let err = ask(&agent, "read", &[("path", "../secret.txt")])
        .await
        .unwrap_err()
        .render();
    assert!(
        err.contains("outside the folder this agent may use (data/)"),
        "{}",
        err
    );
}

#[tokio::test]
async fn the_protected_model_key_stays_protected_when_named_by_a_parameter() {
    unsafe { std::env::set_var("ANTHROPIC_API_KEY", "shh") };
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        "[parameters]\nvar = \"ANTHROPIC_API_KEY\"\n",
    )
    .unwrap();
    let source = "agent E\n  goal \"e\"\n  tool env @parameters.var\n  accepts get name\n  on get\n    reply env.get name: name\n";
    let rt = runtime_from_folder(dir.path(), None);
    let def =
        metagente::lang::parse_file_with("t.ag", None, source, &rt.config.parameters).unwrap();
    let agent =
        metagente::runtime::interpreter::Agent::new(rt, Arc::new(def.into_iter().next().unwrap()))
            .unwrap();
    let err = ask(&agent, "get", &[("name", "ANTHROPIC_API_KEY")])
        .await
        .unwrap_err()
        .render();
    assert!(
        err.contains("holds the key for the language model"),
        "{}",
        err
    );
    assert!(!err.contains("shh"));
}

// 0.1.2: FR-010, a cycle is found when the link path comes from a parameter
#[tokio::test]
async fn cycle_detection_applies_to_linked_paths_given_by_parameters() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        "[parameters]\na = \"a.ag\"\nb = \"b.ag\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("a.ag"),
        "agent CycleA\n  goal \"a\"\n  link CycleB from @parameters.b\n  accepts go\n  on go\n    reply CycleB.go\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("b.ag"),
        "agent CycleB\n  goal \"b\"\n  link CycleA from @parameters.a\n  accepts go\n  on go\n    reply CycleA.go\n",
    )
    .unwrap();
    let rt = runtime_from_folder(dir.path(), None);
    let err = run_file(&options(&dir.path().join("a.ag"), "go"), rt)
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("CycleA -> CycleB -> CycleA"), "{}", err);
}

// 0.1.2: scenario 4, the address of a remote agent comes from the parameter
#[test]
fn a_remote_address_can_come_from_a_parameter() {
    let mut params = metagente::lang::Params::default();
    params.values.insert(
        "a2a_leitor".to_string(),
        "http://127.0.0.1:8080".to_string(),
    );
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/parameters.ag"),
    )
    .unwrap();
    params.values.insert("prompt1".to_string(), "p".to_string());
    let agents = metagente::lang::parse_file_with("parameters.ag", None, &source, &params).unwrap();
    assert_eq!(agents[0].remotes[0].url, "http://127.0.0.1:8080");
}

// 0.1.2: FR-011, a changed parameter applies to the next request of `serve`
#[test]
fn serve_picks_up_a_changed_parameter_on_the_next_request() {
    let dir = tempfile::tempdir().unwrap();
    let toml = dir.path().join("metagente.toml");
    std::fs::write(&toml, "[parameters]\ng = \"first goal\"\n").unwrap();
    let file = dir.path().join("a.ag");
    std::fs::write(
        &file,
        "agent A\n  goal @parameters.g\n  accepts go\n  on go\n    reply \"x\"\n",
    )
    .unwrap();
    let rt = runtime_from_folder(dir.path(), None);
    let agents = metagente::runtime::run::load_agents_with(&file, &rt.config.parameters).unwrap();
    let served = Served::new(rt, file, None, agents);
    assert_eq!(served.agents()[0].goal.as_ref().unwrap().0, "first goal");
    std::fs::write(&toml, "[parameters]\ng = \"second goal\"\n").unwrap();
    assert_eq!(served.agents()[0].goal.as_ref().unwrap().0, "second goal");
}

// 0.1.2: `metagente new` still makes the same files, and its commented [parameters] example works once uncommented
#[test]
fn new_writes_a_commented_parameters_example() {
    let dir = tempfile::tempdir().unwrap();
    let out = metagente(dir.path())
        .args(["new", "hello"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let mut names: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    assert_eq!(names, vec!["hello.ag", "metagente.toml"]);
    let toml = std::fs::read_to_string(dir.path().join("metagente.toml")).unwrap();
    assert!(toml.contains("# [parameters]"));
    let config = Config::load(dir.path()).unwrap();
    assert!(config.parameters.values.is_empty());
    assert!(config.warnings.is_empty(), "{:?}", config.warnings);
    let uncommented = toml
        .replace("# [parameters]", "[parameters]")
        .replace("# prompt1", "prompt1")
        .replace("# a2a_leitor", "a2a_leitor");
    std::fs::write(dir.path().join("metagente.toml"), uncommented).unwrap();
    let config = Config::load(dir.path()).unwrap();
    assert_eq!(config.parameters.values.len(), 2);
}
