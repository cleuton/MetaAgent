// 0.1.5: new file (US3, FR-002, FR-021). What named models must NOT change: the note of 0.1.4 for an unknown scalar of
// [llm], the environment overrides, the default model and its messages, a project without sub-tables.
use crate::common::*;
use async_trait::async_trait;
use metagente::llm::providers::from_config_with;
use metagente::llm::{Llm, LlmReply, LlmRequest};
use metagente::runtime::config::Config;
use metagente::runtime::models::ProviderKind;
use metagente::runtime::net::ClientSet;
use std::process::Command;
use std::sync::Arc;

fn project(toml: &str, agent: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("metagente.toml"), toml).unwrap();
    std::fs::write(dir.path().join("a.ag"), agent).unwrap();
    dir
}

const HI: &str = "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply \"hi\"\n";

fn run(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_metagente"))
        .current_dir(dir)
        .args(args)
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .env_remove("METAGENTE_LLM_PROVIDER")
        .env_remove("METAGENTE_LLM_MODEL")
        .output()
        .unwrap()
}

#[test]
fn an_unknown_scalar_of_llm_prints_the_0_1_4_note_and_a_valid_sub_table_prints_none() {
    let toml = "[version]\nminimum = \"0.1.4\"\n\n[llm]\nprovider = \"anthropic\"\ncolour = \"x\"\n\n[llm.fast]\nprovider = \"openai\"\nmodel = \"m\"\n";
    let dir = project(toml, HI);
    let out = run(dir.path(), &["run", "a.ag", "go"]);
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(out.status.success(), "{}", err);
    assert!(
        err.contains("Note: metagente.toml has an unknown setting `colour` in [llm]; I ignored it"),
        "{}",
        err
    );
    assert!(
        !err.contains("`fast`"),
        "a valid sub-table prints no note: {}",
        err
    );
}

#[test]
fn the_environment_overrides_change_llm_only_and_never_a_named_model() {
    let toml = "[llm.fast]\nprovider = \"openai\"\nmodel = \"named-model\"\napi_key_env = \"K\"\n";
    let dir = project(toml, HI);
    let env = |name: &str| match name {
        "METAGENTE_LLM_PROVIDER" => Some("anthropic".to_string()),
        "METAGENTE_LLM_MODEL" => Some("override-model".to_string()),
        _ => None,
    };
    let config = Config::load_with_env(dir.path(), &env).unwrap();
    // [llm] had no provider; the override creates the default model, as in 0.1.4
    assert_eq!(config.llm.provider.as_deref(), Some("anthropic"));
    assert_eq!(config.llm.model.as_deref(), Some("override-model"));
    let net = Arc::new(ClientSet::for_config(&config));
    assert!(from_config_with(&config, net).unwrap().is_some());
    // the named model is untouched
    let fast = config.models.get("fast").unwrap();
    assert_eq!(fast.provider, ProviderKind::OpenAi);
    assert_eq!(
        fast.model,
        metagente::runtime::models::ModelName::Literal("named-model".to_string())
    );
    assert_eq!(config.models.len(), 1);
}

#[test]
fn an_empty_api_key_env_of_llm_still_means_the_usual_variable() {
    let dir = project("[llm]\nprovider = \"anthropic\"\napi_key_env = \"\"\n", HI);
    let config = Config::load_with_env(dir.path(), &|_| None).unwrap();
    assert!(config.llm.api_key_env.is_none());
}

#[test]
fn a_project_without_sub_tables_gets_no_new_output_from_check_or_run() {
    let agent = "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply think \"hi\"\n";
    let dir = project("[llm]\nprovider = \"anthropic\"\n", agent);
    let check = run(dir.path(), &["check", "a.ag"]);
    let stderr = String::from_utf8_lossy(&check.stderr).to_string();
    assert!(check.status.success(), "{}", stderr);
    assert!(
        !stderr.contains("Warning"),
        "an unset key of [llm] is still not looked at by check: {}",
        stderr
    );
    assert!(!stderr.contains("needs the variable"), "{}", stderr);
    assert!(
        String::from_utf8_lossy(&check.stdout).trim().is_empty()
            || !String::from_utf8_lossy(&check.stdout).contains("model")
    );
    let ran = run(dir.path(), &["run", "a.ag", "go"]);
    let err = String::from_utf8_lossy(&ran.stderr).to_string();
    assert!(
        err.contains(
            "the language model could not answer: the variable ANTHROPIC_API_KEY is not set"
        ),
        "{}",
        err
    );
}

struct Failing;

#[async_trait]
impl Llm for Failing {
    async fn complete(&self, _: &LlmRequest) -> Result<LlmReply, String> {
        Err("boom".to_string())
    }
}

#[tokio::test]
async fn a_failing_llm_model_still_gives_the_exact_0_1_4_message() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), Some(Arc::new(Failing) as Arc<dyn Llm>), |_| {});
    let source = "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply think \"hi\"\n";
    let err = run_source(&rt, source, "go", &[]).await.unwrap_err();
    assert_eq!(err.message, "the language model could not answer: boom");
    assert_eq!(
        err.suggestion.as_deref(),
        Some("check the [llm] settings in metagente.toml and that the key variable is set")
    );
}

#[test]
fn llm_with_only_sub_tables_and_no_provider_has_no_default_model() {
    let dir = project("[llm.fast]\nprovider = \"anthropic\"\n", HI);
    let config = Config::load_with_env(dir.path(), &|_| None).unwrap();
    let net = Arc::new(ClientSet::for_config(&config));
    assert!(from_config_with(&config, net).unwrap().is_none());
}
