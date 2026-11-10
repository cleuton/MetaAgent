// 0.1.5: new file (SC-004, FR-015). No key read from the environment, no model name taken from the environment, and no
// text of a sub-table that is not a safe name appears in any output, log, Agent Card or file written, not even when the
// service answers an error that repeats them.
use crate::common::official::{Official, Reply, anthropic_text, openai_text};
use crate::common::self_signed::{ServeProc, free_port};
use serde_json::json;
use std::path::Path;
use std::process::Command;

const KEY_A: &str = "SCANKEY-anthropic-0001";
const KEY_O: &str = "SCANKEY-openai-0002";
const MODEL_A: &str = "scan-model-small";
const MODEL_O: &str = "scan-model-big";
// a real key is long (D10: longer than 24 characters, so it is never a safe name)
const PASTED: &str = "sk-ant-api03-PASTEDKEY-0123456789abcdefghijklmnopqrstuvwxyz-0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";

fn set(name: &str, value: &str) {
    unsafe { std::env::set_var(name, value) };
}

fn setup(fake: &Official) -> tempfile::TempDir {
    set("SCAN_FAST_KEY", KEY_A);
    set("SCAN_STRONG_KEY", KEY_O);
    set("SCAN_FAST_MODEL", MODEL_A);
    set("SCAN_STRONG_MODEL", MODEL_O);
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        format!(
            "[version]\nminimum = \"{}\"\n\n{}\n[llm.fast]\nprovider = \"anthropic\"\nmodel_env = \"SCAN_FAST_MODEL\"\napi_key_env = \"SCAN_FAST_KEY\"\n\n[llm.strong]\nprovider = \"openai\"\nmodel_env = \"SCAN_STRONG_MODEL\"\napi_key_env = \"SCAN_STRONG_KEY\"\n",
            env!("CARGO_PKG_VERSION"),
            fake.network_toml()
        ),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("assistant.ag"),
        "agent Assistant\n  goal \"two steps\"\n  accepts ask question\n  on ask\n    short = think \"Rewrite: {question}\" using fast\n    reply think \"Answer: {short}\" using strong\n",
    )
    .unwrap();
    dir
}

fn assert_clean(what: &str, text: &str, needles: &[&str]) {
    for n in needles {
        assert!(!text.contains(n), "{} shows `{}`:\n{}", what, n, text);
    }
}

fn files_written(dir: &Path) -> String {
    let mut out = String::new();
    for entry in walk(dir) {
        let name = entry.file_name().unwrap().to_string_lossy().to_string();
        if name == "metagente.toml" || name == "assistant.ag" || name == "ca.pem" {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&entry) {
            out.push_str(&format!("--- {}\n{}\n", entry.display(), text));
        }
    }
    out
}

fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(read) = std::fs::read_dir(dir) {
        for e in read.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(walk(&p));
            } else {
                out.push(p);
            }
        }
    }
    out
}

const ALL: &[&str] = &[KEY_A, KEY_O, MODEL_A, MODEL_O, "SCANKEY", "scan-model"];

#[tokio::test]
async fn a_successful_run_shows_neither_a_key_nor_a_model_name() {
    let fake = Official::start().await;
    fake.script_anthropic(vec![anthropic_text("short")]);
    fake.script_openai(vec![openai_text("done")]);
    let dir = setup(&fake);
    let out = tokio::process::Command::new(env!("CARGO_BIN_EXE_metagente"))
        .current_dir(dir.path())
        .args(["run", "assistant.ag", "ask", "question=why"])
        .output()
        .await
        .unwrap();
    assert!(out.status.success());
    assert_clean("stdout", &String::from_utf8_lossy(&out.stdout), ALL);
    assert_clean("stderr", &String::from_utf8_lossy(&out.stderr), ALL);
    assert_clean("files", &files_written(dir.path()), ALL);
}

#[tokio::test]
async fn a_failing_run_shows_no_key_model_name_or_address_even_when_the_service_repeats_them() {
    let fake = Official::start().await;
    let echo = format!(
        "Incorrect API key provided: {} for model {} at https://api.anthropic.com/v1/messages (also {})",
        KEY_A, MODEL_A, KEY_O
    );
    fake.script_anthropic(vec![Reply::Json(401, json!({"error": {"message": echo}}))]);
    let dir = setup(&fake);
    let out = tokio::process::Command::new(env!("CARGO_BIN_EXE_metagente"))
        .current_dir(dir.path())
        .args(["run", "assistant.ag", "ask", "question=why"])
        .output()
        .await
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(!out.status.success());
    assert!(
        stderr.contains("The model fast (anthropic) answered with status 401."),
        "{}",
        stderr
    );
    assert_clean("stdout", &String::from_utf8_lossy(&out.stdout), ALL);
    assert_clean("stderr", &stderr, ALL);
    assert_clean(
        "stderr",
        &stderr,
        &["api.anthropic.com", "api.openai.com", "Incorrect"],
    );
    assert_clean("files", &files_written(dir.path()), ALL);
}

#[test]
fn a_pasted_key_in_a_sub_table_is_refused_and_never_printed() {
    for toml in [
        format!(
            "[llm.fast]\nprovider = \"anthropic\"\napi_key_env = \"{}\"\n",
            PASTED
        ),
        format!(
            "[llm.fast]\nprovider = \"anthropic\"\nmodel_env = \"{}\"\n",
            PASTED
        ),
        format!("[llm.fast]\nprovider = \"{}\"\n", PASTED),
        format!("[llm.fast]\nprovider = \"anthropic\"\n{} = \"x\"\n", PASTED),
        format!("[llm.{}]\nprovider = \"anthropic\"\n", PASTED),
        format!(
            "[llm.fast]\nprovider = \"anthropic\"\nkey = \"{}\"\n",
            PASTED
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("metagente.toml"), &toml).unwrap();
        std::fs::write(
            dir.path().join("a.ag"),
            "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply think \"hi\" using fast\n",
        )
        .unwrap();
        for command in ["check", "run"] {
            let args: Vec<&str> = if command == "check" {
                vec!["check", "a.ag"]
            } else {
                vec!["run", "a.ag", "go"]
            };
            let out = Command::new(env!("CARGO_BIN_EXE_metagente"))
                .current_dir(dir.path())
                .args(args)
                .output()
                .unwrap();
            let all = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            assert!(!out.status.success(), "{} of {}: {}", command, toml, all);
            assert_clean(
                &format!("{} of `{}`", command, toml),
                &all,
                &["PASTEDKEY", "sk-ant", "0123456789"],
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_served_agent_shows_no_key_in_its_card_its_answers_or_its_log() {
    let fake = Official::start().await;
    let echo = format!("bad key {} model {}", KEY_A, MODEL_A);
    fake.script_anthropic(vec![Reply::Json(401, json!({"error": {"message": echo}}))]);
    let dir = setup(&fake);
    let port = free_port();
    let serve = ServeProc::start(
        dir.path(),
        &[
            "assistant.ag".to_string(),
            "--a2a".to_string(),
            port.to_string(),
        ],
        1,
    );
    let client = reqwest::Client::new();
    let base = format!("http://127.0.0.1:{}", port);
    let card = client
        .get(format!("{}/.well-known/agent-card.json", base))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "SendMessage", "params": crate::common::a2a::user_text("ask question=why")});
    let reply = client
        .post(format!("{}/a2a", base))
        .header("A2A-Version", "1.0")
        .body(body.to_string())
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(reply.contains("answered with status 401"), "{}", reply);
    assert_clean("the Agent Card", &card, ALL);
    assert_clean("the answer", &reply, ALL);
    assert_clean("the answer", &reply, &["api.anthropic.com", "Incorrect"]);
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_clean("the log of serve", &serve.said(), ALL);
    assert_clean("files", &files_written(dir.path()), ALL);
}
