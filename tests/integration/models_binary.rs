// 0.1.5: new file. The two-model agent of the spec appendix, run with the REAL binary: a metagente.toml with [version],
// [network] (proxy and ca_file, the only way the fakes are reached) and two named models. Each fake sees only its own
// key and the strong model answers after the fast model shortened the question.
use crate::common::official::{Official, anthropic_text, openai_text};
use std::process::Stdio;

const ASSISTANT: &str = "agent Assistant\n  goal \"Answer a question in two steps\"\n  accepts ask question\n  on ask\n    short = think \"Rewrite as a short search query: {question}\" using fast\n    reply think \"Answer this well: {short}\" using strong\n";

fn toml(fake: &Official) -> String {
    format!(
        "[version]\nminimum = \"{}\"\n\n{}\n[llm.fast]\nprovider = \"anthropic\"\nmodel_env = \"FAST_MODEL\"\napi_key_env = \"FAST_KEY\"\n\n[llm.strong]\nprovider = \"openai\"\nmodel_env = \"STRONG_MODEL\"\napi_key_env = \"STRONG_KEY\"\n",
        env!("CARGO_PKG_VERSION"),
        fake.network_toml()
    )
}

fn command(dir: &std::path::Path) -> tokio::process::Command {
    let mut c = tokio::process::Command::new(env!("CARGO_BIN_EXE_metagente"));
    c.current_dir(dir)
        .env_remove("FAST_MODEL")
        .env_remove("FAST_KEY")
        .env_remove("STRONG_MODEL")
        .env_remove("STRONG_KEY")
        .env_remove("METAGENTE_LLM_PROVIDER")
        .env_remove("METAGENTE_LLM_MODEL")
        .stdin(Stdio::null());
    c
}

#[tokio::test]
async fn the_binary_uses_two_models_in_one_agent_and_each_key_reaches_only_its_own_provider() {
    let fake = Official::start().await;
    fake.script_anthropic(vec![anthropic_text("blue sky reason")]);
    fake.script_openai(vec![openai_text("Rayleigh scattering.")]);
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("metagente.toml"), toml(&fake)).unwrap();
    std::fs::write(dir.path().join("assistant.ag"), ASSISTANT).unwrap();
    let out = command(dir.path())
        .args(["run", "assistant.ag", "ask", "question=why is the sky blue"])
        .env("FAST_MODEL", "small-model")
        .env("FAST_KEY", "anthropic-key-aaa")
        .env("STRONG_MODEL", "big-model")
        .env("STRONG_KEY", "openai-key-bbb")
        .output()
        .await
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(out.status.success(), "{}\n{}", stdout, stderr);
    assert_eq!(stdout.trim(), "Rayleigh scattering.");
    assert!(!stderr.contains("Warning"), "{}", stderr);
    assert!(
        !stderr.contains("Note:"),
        "a valid sub-table prints no note: {}",
        stderr
    );
    let anthropic = fake.seen_on("/v1/messages");
    let openai = fake.seen_on("/v1/chat/completions");
    assert_eq!(anthropic.len(), 1);
    assert_eq!(openai.len(), 1);
    assert_eq!(anthropic[0].x_api_key.as_deref(), Some("anthropic-key-aaa"));
    assert_eq!(anthropic[0].body["model"], "small-model");
    assert_eq!(
        openai[0].authorization.as_deref(),
        Some("Bearer openai-key-bbb")
    );
    assert_eq!(openai[0].body["model"], "big-model");
    assert!(!format!("{:?}", anthropic).contains("openai-key-bbb"));
    assert!(!format!("{:?}", openai).contains("anthropic-key-aaa"));
    assert_eq!(
        openai[0].body["messages"][1]["content"],
        "Answer this well: blue sky reason"
    );
}

#[tokio::test]
async fn the_binary_says_which_variable_is_missing_without_showing_a_value() {
    let fake = Official::start().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("metagente.toml"), toml(&fake)).unwrap();
    std::fs::write(dir.path().join("assistant.ag"), ASSISTANT).unwrap();
    let out = command(dir.path())
        .args(["run", "assistant.ag", "ask", "question=x"])
        .env("FAST_MODEL", "small-model")
        .env("STRONG_MODEL", "big-model")
        .output()
        .await
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(!out.status.success());
    assert!(
        stderr.contains("The model fast needs the variable FAST_KEY, and it is not set."),
        "{}",
        stderr
    );
    assert!(stderr.contains("set the variable"), "{}", stderr);
    assert!(fake.seen().is_empty(), "nothing was sent without a key");
}
