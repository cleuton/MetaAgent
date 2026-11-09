// 0.1.4: bearer tokens between Metagente and the official A2A Python SDK, in both directions (FR-049).
// Without Python the tests say they were skipped (in CI they fail).
use crate::common::auth::*;
use crate::common::python::{client, python_or_skip, start_recording_agent};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_python_sdk_client_calls_a_protected_metagente_agent_with_a_token() {
    let Some(py) = python_or_skip() else { return };
    let dir = project(BASIC_TOML);
    let port = free_port();
    let _server = serve(
        dir.path(),
        "a2a",
        &["planner.ag", "--a2a", &port.to_string()],
        port,
    );
    let base = format!("http://127.0.0.1:{}", port);
    let py2 = py.clone();
    let b2 = base.clone();
    let seen = tokio::task::spawn_blocking(move || {
        client(
            &py2,
            &b2,
            &["--message", "plan topic=rust", "--bearer", "good-token"],
        )
    })
    .await
    .unwrap()
    .unwrap();
    // the SDK read the card with the new security fields
    assert_eq!(seen["card_name"], "Planner");
    assert_eq!(seen["state"], "TASK_STATE_COMPLETED");
    assert_eq!(seen["text"], "planned rust");
    assert_eq!(
        seen["get_task_text"], "planned rust",
        "the same token reads the task back"
    );
    // without a token the SDK is refused
    let py3 = py.clone();
    let none =
        tokio::task::spawn_blocking(move || client(&py3, &base, &["--message", "plan topic=rust"]))
            .await
            .unwrap();
    let error = none.expect_err("a client without a token must be refused");
    assert!(
        error.contains("401") || error.to_lowercase().contains("unauthor"),
        "{}",
        error
    );
    assert!(
        runs(dir.path()) >= 1,
        "the validator was asked about the token that had one"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_python_agent_receives_the_token_a_metagente_agent_passes_on() {
    let Some(py) = python_or_skip() else { return };
    let record = tempfile::NamedTempFile::new().unwrap();
    let (_agent, url) = start_recording_agent(&py, record.path());
    let relay = format!(
        "agent Relay\n  goal \"Ask the Python agent\"\n  remote Pyra at \"{}\"\n  accepts relay text\n  on relay\n    reply Pyra.echo text: text\n",
        url
    );
    let dir = stage(
        "relay.ag",
        &relay,
        &toml_for("\"Relay.relay\"", "\"Pyra\""),
        "good-token\n",
    );
    let port = free_port();
    let _server = serve(
        dir.path(),
        "a2a",
        &["relay.ag", "--a2a", &port.to_string()],
        port,
    );
    let body = serde_json::json!({"jsonrpc": "2.0", "id": 7, "method": "SendMessage", "params": {"message": {
        "messageId": "m-1", "role": "ROLE_USER",
        "parts": [{"data": {"skill": "relay", "arguments": {"text": "hi"}}, "mediaType": "application/json"}],
        "metadata": {"skill": "relay"}}}});
    let r = post(port, "/a2a", Some("good-token"), body).await;
    assert_eq!(answer(&r), "echo: hi", "{}", r.text);
    let recorded = std::fs::read_to_string(record.path()).unwrap();
    let lines: Vec<&str> = recorded.lines().collect();
    assert!(!lines.is_empty());
    assert!(
        lines.iter().all(|l| *l == "Bearer good-token"),
        "{:?}",
        lines
    );
}
