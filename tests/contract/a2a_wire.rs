//! The JSON-RPC shapes of SendMessage and GetTask, checked against the A2A 1.0 text.

use crate::a2a::{self, rpc, user_text};
use serde_json::{Value, json};
use std::time::Duration;

fn endpoint(base: &str) -> String {
    format!("{}/a2a", base)
}

fn assert_rfc3339(text: &str) {
    assert!(chrono_like(text), "not an ISO 8601 timestamp: {}", text);
}

fn chrono_like(t: &str) -> bool {
    t.len() >= 20
        && t.as_bytes()[4] == b'-'
        && t.as_bytes()[10] == b'T'
        && (t.ends_with('Z') || t.contains('+'))
}

#[tokio::test]
async fn send_message_returns_a_completed_task_with_an_artifact() {
    let dir = tempfile::tempdir().unwrap();
    let server = a2a::start(dir.path(), a2a::WEATHER, None).await;
    let reply = rpc(
        &endpoint(&server.base),
        "SendMessage",
        user_text("ask city=Faro"),
    )
    .await;
    assert_eq!(reply["jsonrpc"], "2.0");
    assert_eq!(reply["id"], 7);
    assert!(reply.get("error").is_none(), "{}", reply);
    let task = &reply["result"]["task"];
    assert!(task["id"].as_str().map(|s| !s.is_empty()).unwrap_or(false));
    assert!(
        task["contextId"]
            .as_str()
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    );
    assert_eq!(task["status"]["state"], "TASK_STATE_COMPLETED");
    assert_rfc3339(task["status"]["timestamp"].as_str().unwrap());
    let artifact = &task["artifacts"][0];
    assert!(artifact["artifactId"].is_string());
    assert_eq!(artifact["parts"][0]["text"], "sunny in Faro");
}

#[tokio::test]
async fn a_skill_named_in_a_data_part_or_in_metadata_works_too() {
    let dir = tempfile::tempdir().unwrap();
    let server = a2a::start(dir.path(), a2a::WEATHER, None).await;
    let by_data = json!({"message": {"messageId": "m", "role": "ROLE_USER",
        "parts": [{"data": {"skill": "ask", "arguments": {"city": "Braga"}}, "mediaType": "application/json"}]}});
    let r = rpc(&endpoint(&server.base), "SendMessage", by_data).await;
    assert_eq!(
        r["result"]["task"]["artifacts"][0]["parts"][0]["text"],
        "sunny in Braga"
    );
    let by_meta = json!({"message": {"messageId": "m", "role": "ROLE_USER", "metadata": {"skill": "ask"},
        "parts": [{"data": {"city": "Viseu"}}]}});
    let r = rpc(&endpoint(&server.base), "SendMessage", by_meta).await;
    assert_eq!(
        r["result"]["task"]["artifacts"][0]["parts"][0]["text"],
        "sunny in Viseu"
    );
    // A generic client that only sends text: the one value of a one-value skill.
    let r = rpc(
        &endpoint(&server.base),
        "SendMessage",
        user_text("ask Lagos"),
    )
    .await;
    assert_eq!(
        r["result"]["task"]["artifacts"][0]["parts"][0]["text"],
        "sunny in Lagos"
    );
}

#[tokio::test]
async fn get_task_returns_what_send_message_returned() {
    let dir = tempfile::tempdir().unwrap();
    let server = a2a::start(dir.path(), a2a::WEATHER, None).await;
    let sent = rpc(
        &endpoint(&server.base),
        "SendMessage",
        user_text("ask city=Faro"),
    )
    .await;
    let id = sent["result"]["task"]["id"].as_str().unwrap().to_string();
    let got = rpc(&endpoint(&server.base), "GetTask", json!({"id": id})).await;
    assert_eq!(got["result"]["id"], id.as_str());
    assert_eq!(got["result"]["status"]["state"], "TASK_STATE_COMPLETED");
    assert_eq!(
        got["result"]["artifacts"][0]["parts"][0]["text"],
        "sunny in Faro"
    );
}

#[tokio::test]
async fn an_unknown_or_expired_task_is_task_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let server = a2a::start(dir.path(), a2a::WEATHER, Some(Duration::from_millis(300))).await;
    let r = rpc(&endpoint(&server.base), "GetTask", json!({"id": "nope"})).await;
    assert_eq!(r["error"]["code"], -32001);
    assert_eq!(r["error"]["data"][0]["reason"], "TASK_NOT_FOUND");
    assert_eq!(r["error"]["data"][0]["domain"], "a2a-protocol.org");
    // Finished tasks are kept for a limited time (10 minutes by default; 300 ms here).
    let sent = rpc(
        &endpoint(&server.base),
        "SendMessage",
        user_text("ask city=Faro"),
    )
    .await;
    let id = sent["result"]["task"]["id"].as_str().unwrap().to_string();
    assert!(
        rpc(&endpoint(&server.base), "GetTask", json!({"id": id}))
            .await
            .get("result")
            .is_some()
    );
    tokio::time::sleep(Duration::from_millis(450)).await;
    let gone = rpc(&endpoint(&server.base), "GetTask", json!({"id": id})).await;
    assert_eq!(gone["error"]["code"], -32001);
}

#[tokio::test]
async fn an_agent_that_fails_gives_a_failed_task_with_a_plain_message() {
    let dir = tempfile::tempdir().unwrap();
    let server = a2a::start(dir.path(), a2a::WEATHER, None).await;
    let r = rpc(&endpoint(&server.base), "SendMessage", user_text("broken")).await;
    let task = &r["result"]["task"];
    assert_eq!(task["status"]["state"], "TASK_STATE_FAILED");
    let why = task["status"]["message"]["parts"][0]["text"]
        .as_str()
        .unwrap();
    assert!(why.contains("the barometer exploded"), "{}", why);
    assert_eq!(task["status"]["message"]["role"], "ROLE_AGENT");
}

#[tokio::test]
async fn return_immediately_gives_a_task_that_finishes_later() {
    let dir = tempfile::tempdir().unwrap();
    let source = "agent Slow\n  goal \"Take a moment\"\n  tool clock\n  accepts go\n  on go\n    clock.wait seconds: 1\n    reply \"finally\"\n";
    let server = a2a::start(dir.path(), source, None).await;
    let mut params = user_text("go");
    params["configuration"] = json!({"returnImmediately": true});
    let r = rpc(&endpoint(&server.base), "SendMessage", params).await;
    let state = r["result"]["task"]["status"]["state"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        state == "TASK_STATE_WORKING" || state == "TASK_STATE_SUBMITTED",
        "{}",
        state
    );
    let id = r["result"]["task"]["id"].as_str().unwrap().to_string();
    let mut done = Value::Null;
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let got = rpc(&endpoint(&server.base), "GetTask", json!({"id": id})).await;
        if got["result"]["status"]["state"] == "TASK_STATE_COMPLETED" {
            done = got;
            break;
        }
    }
    assert_eq!(
        done["result"]["artifacts"][0]["parts"][0]["text"],
        "finally"
    );
}

#[tokio::test]
async fn protocol_errors_use_the_standard_json_rpc_codes() {
    let dir = tempfile::tempdir().unwrap();
    let server = a2a::start(dir.path(), a2a::WEATHER, None).await;
    let url = endpoint(&server.base);
    let r = rpc(&url, "Dance", json!({})).await;
    assert_eq!(r["error"]["code"], -32601);
    assert!(
        r["error"]["message"]
            .as_str()
            .unwrap()
            .contains("SendMessage and GetTask")
    );
    let r = rpc(&url, "SendMessage", json!({})).await;
    assert_eq!(r["error"]["code"], -32602);
    let r = rpc(
        &url,
        "SendMessage",
        json!({"message": {"messageId": "m", "role": "ROLE_USER", "parts": []}}),
    )
    .await;
    assert_eq!(r["error"]["code"], -32602);
    // Missing value for the skill.
    let r = rpc(&url, "SendMessage", user_text("ask")).await;
    assert_eq!(r["error"]["code"], -32602);
    assert!(
        r["error"]["message"]
            .as_str()
            .unwrap()
            .contains("needs a value for `city`"),
        "{}",
        r
    );
    // Not JSON at all.
    let raw = reqwest::Client::new()
        .post(&url)
        .body("{oops")
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(raw["error"]["code"], -32700);
    assert_eq!(raw["id"], Value::Null);
    // Not JSON-RPC 2.0.
    let raw = reqwest::Client::new()
        .post(&url)
        .body(r#"{"id":1,"method":"GetTask"}"#)
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(raw["error"]["code"], -32600);
}

#[tokio::test]
async fn an_unsupported_protocol_version_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let server = a2a::start(dir.path(), a2a::WEATHER, None).await;
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "GetTask", "params": {"id": "x"}});
    let r: Value = reqwest::Client::new()
        .post(endpoint(&server.base))
        .header("A2A-Version", "0.3")
        .body(body.to_string())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(r["error"]["code"], -32009);
    assert_eq!(r["error"]["data"][0]["reason"], "VERSION_NOT_SUPPORTED");
}

#[tokio::test]
async fn responses_declare_json() {
    let dir = tempfile::tempdir().unwrap();
    let server = a2a::start(dir.path(), a2a::WEATHER, None).await;
    let response = reqwest::get(format!("{}/.well-known/agent-card.json", server.base))
        .await
        .unwrap();
    assert!(
        response.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("application/json")
    );
}
