use crate::common::web;
use crate::common::*;
use metagente::runtime::value::Value;
use serde_json::json;

#[tokio::test]
async fn http_get_and_post_return_status_text_and_json() {
    let web = web::start(json!({})).await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let source = format!(
        r#"agent Web
  goal "Talk to the web"
  tool http
  accepts go
  on go
    got = http.get url: "{base}/hello"
    posted = http.post url: "{base}/echo" body: "ping"
    reply "{{got.status}} {{got.json.greeting}} {{got.json.count}} {{posted.text}}"
"#,
        base = web.base
    );
    let reply = run_source(&rt, &source, "go", &[]).await.unwrap();
    assert_eq!(reply, Value::text("200 hi 3 ping"));
}

#[tokio::test]
async fn http_to_a_closed_port_is_explained() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let source = "agent Web\n  goal \"w\"\n  tool http\n  accepts go\n  on go\n    x = http.get url: \"http://127.0.0.1:1/nothing\"\n    reply x\n";
    let err = run_source(&rt, source, "go", &[])
        .await
        .unwrap_err()
        .render();
    assert!(
        err.contains("I could not reach http://127.0.0.1:1/nothing"),
        "{}",
        err
    );
    assert!(err.contains("internet connection"), "{}", err);
}

#[tokio::test]
async fn http_rejects_an_address_without_a_scheme() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let source = "agent Web\n  goal \"w\"\n  tool http\n  accepts go\n  on go\n    x = http.get url: \"example.org\"\n    reply x\n";
    let err = run_source(&rt, source, "go", &[])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("http:// or https://"), "{}", err);
}

const MEMORY: &str = r#"agent Memory
  goal "Remember things"
  tool state
  accepts remember what
  accepts recall
  on remember
    state.set key: "thing" value: what
    reply "ok"
  on recall
    reply state.get key: "thing"
"#;

#[tokio::test]
async fn state_is_kept_per_agent_between_requests() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let first = agent_from(&rt, MEMORY, None);
    ask(&first, "remember", &[("what", "blue")]).await.unwrap();
    // A new instance of the same agent shares its memory.
    let second = agent_from(&rt, MEMORY, None);
    assert_eq!(
        ask(&second, "recall", &[]).await.unwrap(),
        Value::text("blue")
    );
    // A different agent has its own memory.
    let other = agent_from(&rt, &MEMORY.replace("Memory", "Other"), None);
    assert_eq!(ask(&other, "recall", &[]).await.unwrap(), Value::Nothing);
}
