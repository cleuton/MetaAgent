//! SC-006: an A2A client written by someone else discovers the card and completes a task.
//! The client is the official Python SDK. Set METAGENTE_INTEROP_PYTHON to a Python that has
//! `a2a-sdk` installed (CI does this). Without it the test says it was skipped and passes.

use crate::common::a2a::{self, WEATHER};
use std::process::Command;

fn python() -> Option<String> {
    let candidates = std::env::var("METAGENTE_INTEROP_PYTHON")
        .ok()
        .into_iter()
        .chain(["python3".to_string()]);
    for py in candidates {
        let ok = Command::new(&py)
            .args(["-c", "import a2a.client"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ok {
            return Some(py);
        }
    }
    None
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_official_python_sdk_discovers_the_card_and_completes_a_task() {
    let Some(py) = python() else {
        eprintln!(
            "SKIPPED: no Python with a2a-sdk (pip install a2a-sdk; set METAGENTE_INTEROP_PYTHON)"
        );
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let server = a2a::start(dir.path(), WEATHER, None).await;
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/interop/a2a_sdk_client.py"
    );
    let base = server.base.clone();
    let out = tokio::task::spawn_blocking(move || {
        Command::new(py).arg(script).arg(base).output().unwrap()
    })
    .await
    .unwrap();
    assert!(
        out.status.success(),
        "the SDK client failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let seen: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("the client prints JSON");
    assert_eq!(seen["card_name"], "Weather");
    assert_eq!(seen["skills"], serde_json::json!(["ask", "broken"]));
    assert_eq!(seen["interfaces"][0], serde_json::json!(["JSONRPC", "1.0"]));
    assert_eq!(seen["state"], "TASK_STATE_COMPLETED");
    assert_eq!(seen["text"], "sunny in Lisbon");
    assert_eq!(seen["get_task_state"], "TASK_STATE_COMPLETED");
    assert_eq!(seen["get_task_text"], "sunny in Lisbon");
    let unsupported = seen["unsupported_error"]
        .as_str()
        .expect("the SDK should report the unsupported request");
    assert!(unsupported.contains("Not supported"), "{}", unsupported);
}
