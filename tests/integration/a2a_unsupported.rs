use crate::common::a2a::{self, rpc, user_text};
use serde_json::json;

const TWO: &str = r#"agent Weather
  goal "Weather"
  accepts ask city
  accepts alerts
  on ask
    reply "sunny in {city}"
  on alerts
    reply "none"
"#;

#[tokio::test]
async fn an_unknown_skill_gets_a_not_supported_error_that_lists_what_is_handled() {
    let dir = tempfile::tempdir().unwrap();
    let server = a2a::start(dir.path(), TWO, None).await;
    let url = format!("{}/a2a", server.base);
    for params in [
        user_text("dance"),
        json!({"message": {"messageId": "m", "role": "ROLE_USER", "parts": [{"data": {"skill": "dance"}}]}}),
        json!({"message": {"messageId": "m", "role": "ROLE_USER", "metadata": {"skill": "dance"}, "parts": [{"text": "x"}]}}),
    ] {
        let r = rpc(&url, "SendMessage", params).await;
        assert_eq!(r["error"]["code"], -32004, "{}", r);
        let message = r["error"]["message"].as_str().unwrap();
        assert!(message.starts_with("Not supported"), "{}", message);
        assert!(message.contains("ask, alerts"), "{}", message);
        assert_eq!(r["error"]["data"][0]["reason"], "UNSUPPORTED_OPERATION");
    }
}

#[tokio::test]
async fn an_unknown_agent_name_is_not_supported_too() {
    let dir = tempfile::tempdir().unwrap();
    let server = a2a::start(dir.path(), TWO, None).await;
    let r = rpc(
        &format!("{}/agents/Nobody/a2a", server.base),
        "SendMessage",
        user_text("ask x"),
    )
    .await;
    assert_eq!(r["error"]["code"], -32004);
    assert!(
        r["error"]["message"]
            .as_str()
            .unwrap()
            .contains("no agent called Nobody")
    );
    let mut params = user_text("ask x");
    params["tenant"] = json!("Nobody");
    let r = rpc(&format!("{}/a2a", server.base), "SendMessage", params).await;
    assert_eq!(r["error"]["code"], -32004);
}
