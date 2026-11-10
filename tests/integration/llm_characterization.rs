// 0.1.5: new file. What 0.1.4 sends for `[llm]` (address, key header, body, user agent) and what it says when a call
// fails. Written against the untouched 0.1.4 code and kept green after: this is how "[llm] does not change" is proved
// (FR-001, FR-021). The user agent is compared as `metagente/<any version>` (F16).
use crate::common::official::{
    Official, Reply, anthropic_text, anthropic_tool_call, openai_text, openai_tool_call,
};
use crate::common::web;
use metagente::llm::providers::from_config;
use metagente::llm::{ChatMsg, LlmReply, LlmRequest, ToolCallReq, ToolSpec};
use metagente::runtime::config::Config;
use serde_json::json;

/// The tests that set the usual key variables must not run at the same time.
static USUAL_KEYS: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn plain() -> LlmRequest {
    LlmRequest {
        system: "You are a test".to_string(),
        messages: vec![ChatMsg::User("hi".to_string())],
        tools: vec![],
    }
}

fn with_tool() -> LlmRequest {
    LlmRequest {
        system: "You are a test".to_string(),
        messages: vec![ChatMsg::User("what time is it".to_string())],
        tools: vec![ToolSpec {
            name: "clock__now".to_string(),
            description: "the time".to_string(),
            schema: json!({"type": "object", "properties": {}}),
        }],
    }
}

fn round_trip() -> LlmRequest {
    let call = ToolCallReq {
        id: "call_1".to_string(),
        name: "clock__now".to_string(),
        args: json!({}),
    };
    LlmRequest {
        system: "You are a test".to_string(),
        messages: vec![
            ChatMsg::User("what time is it".to_string()),
            ChatMsg::Assistant {
                text: None,
                tool_call: Some(call),
            },
            ChatMsg::ToolResult {
                id: "call_1".to_string(),
                name: "clock__now".to_string(),
                content: "noon".to_string(),
            },
        ],
        tools: with_tool().tools,
    }
}

fn config(provider: &str, key_var: &str) -> Config {
    let mut c = Config::default();
    c.llm.provider = Some(provider.to_string());
    c.llm.model = Some("test-model".to_string());
    c.llm.api_key_env = Some(key_var.to_string());
    c
}

fn is_metagente_user_agent(ua: &str) -> bool {
    ua.strip_prefix("metagente/")
        .map(|v| !v.is_empty() && v.chars().all(|c| c.is_ascii_digit() || c == '.'))
        .unwrap_or(false)
}

async fn ask(c: &Config, req: &LlmRequest) -> Result<LlmReply, String> {
    from_config(c).unwrap().unwrap().complete(req).await
}

#[tokio::test]
async fn openai_with_no_base_url_goes_to_the_official_address_with_a_bearer_key() {
    let fake = Official::start().await;
    fake.script_openai(vec![openai_text("hello")]);
    unsafe { std::env::set_var("MG_CHAR_OPENAI_KEY", "key-openai-1") };
    let mut c = config("openai", "MG_CHAR_OPENAI_KEY");
    fake.apply(&mut c);
    assert_eq!(
        ask(&c, &plain()).await.unwrap(),
        LlmReply::Text("hello".into())
    );
    let seen = fake.seen();
    assert_eq!(seen.len(), 1);
    let s = &seen[0];
    assert_eq!(s.path, "/v1/chat/completions");
    assert_eq!(s.host, "api.openai.com");
    assert_eq!(s.authorization.as_deref(), Some("Bearer key-openai-1"));
    assert!(s.x_api_key.is_none());
    assert!(is_metagente_user_agent(&s.user_agent), "{}", s.user_agent);
    assert_eq!(s.body["model"], "test-model");
    assert_eq!(
        s.body["messages"][0],
        json!({"role": "system", "content": "You are a test"})
    );
    assert_eq!(
        s.body["messages"][1],
        json!({"role": "user", "content": "hi"})
    );
    assert!(s.body.get("tools").is_none(), "{}", s.body);
    assert!(
        s.body.get("max_tokens").is_none(),
        "no limit on the size of the answer"
    );
    assert!(
        fake.proxy_log
            .lines()
            .iter()
            .any(|l| l.starts_with("CONNECT api.openai.com:443")),
        "{:?}",
        fake.proxy_log.lines()
    );
}

#[tokio::test]
async fn anthropic_with_no_base_url_goes_to_the_official_address_with_its_own_key_header() {
    let fake = Official::start().await;
    fake.script_anthropic(vec![anthropic_text("hello")]);
    unsafe { std::env::set_var("MG_CHAR_ANTHROPIC_KEY", "key-anthropic-1") };
    let mut c = config("anthropic", "MG_CHAR_ANTHROPIC_KEY");
    fake.apply(&mut c);
    assert_eq!(
        ask(&c, &plain()).await.unwrap(),
        LlmReply::Text("hello".into())
    );
    let s = &fake.seen()[0];
    assert_eq!(s.path, "/v1/messages");
    assert_eq!(s.host, "api.anthropic.com");
    assert_eq!(s.x_api_key.as_deref(), Some("key-anthropic-1"));
    assert_eq!(s.anthropic_version.as_deref(), Some("2023-06-01"));
    assert!(s.authorization.is_none());
    assert!(is_metagente_user_agent(&s.user_agent), "{}", s.user_agent);
    assert_eq!(s.body["model"], "test-model");
    assert_eq!(s.body["max_tokens"], 1024);
    assert_eq!(s.body["system"], "You are a test");
    assert_eq!(
        s.body["messages"][0],
        json!({"role": "user", "content": "hi"})
    );
    assert!(s.body.get("tools").is_none(), "{}", s.body);
    assert!(
        fake.proxy_log
            .lines()
            .iter()
            .any(|l| l.starts_with("CONNECT api.anthropic.com:443")),
        "{:?}",
        fake.proxy_log.lines()
    );
}

#[tokio::test]
async fn the_usual_key_variable_of_each_provider_is_used_when_api_key_env_is_not_set() {
    let _guard = USUAL_KEYS.lock().await;
    let fake = Official::start().await;
    fake.script_openai(vec![openai_text("o")]);
    fake.script_anthropic(vec![anthropic_text("a")]);
    unsafe {
        std::env::set_var("OPENAI_API_KEY", "usual-openai");
        std::env::set_var("ANTHROPIC_API_KEY", "usual-anthropic");
    }
    for provider in ["openai", "anthropic"] {
        let mut c = Config::default();
        c.llm.provider = Some(provider.to_string());
        c.llm.model = Some("m".to_string());
        fake.apply(&mut c);
        ask(&c, &plain()).await.unwrap();
    }
    let seen = fake.seen();
    assert_eq!(
        seen[0].authorization.as_deref(),
        Some("Bearer usual-openai")
    );
    assert_eq!(seen[1].x_api_key.as_deref(), Some("usual-anthropic"));
}

#[tokio::test]
async fn anthropic_has_a_usual_model_and_openai_needs_one() {
    let fake = Official::start().await;
    fake.script_anthropic(vec![anthropic_text("a")]);
    unsafe { std::env::set_var("MG_CHAR_USUAL_MODEL_KEY", "k") };
    let mut c = Config::default();
    c.llm.provider = Some("anthropic".to_string());
    c.llm.api_key_env = Some("MG_CHAR_USUAL_MODEL_KEY".to_string());
    fake.apply(&mut c);
    ask(&c, &plain()).await.unwrap();
    assert_eq!(fake.seen()[0].body["model"], "claude-sonnet-5-5");
    let mut o = Config::default();
    o.llm.provider = Some("openai".to_string());
    let err = from_config(&o).err().expect("openai needs a model name");
    assert!(
        err.message
            .contains("the provider `openai` needs a model name"),
        "{}",
        err.message
    );
}

#[tokio::test]
async fn a_tool_in_the_request_and_a_tool_round_trip_have_the_same_shape_as_in_0_1_4() {
    let fake = Official::start().await;
    fake.script_openai(vec![
        openai_tool_call("call_1", "clock__now", json!({})),
        openai_text("done"),
    ]);
    fake.script_anthropic(vec![
        anthropic_tool_call("call_1", "clock__now", json!({})),
        anthropic_text("done"),
    ]);
    unsafe {
        std::env::set_var("MG_CHAR_RT_OPENAI", "k1");
        std::env::set_var("MG_CHAR_RT_ANTHROPIC", "k2");
    }
    let mut o = config("openai", "MG_CHAR_RT_OPENAI");
    let mut a = config("anthropic", "MG_CHAR_RT_ANTHROPIC");
    fake.apply(&mut o);
    fake.apply(&mut a);
    let call = LlmReply::ToolCall(ToolCallReq {
        id: "call_1".into(),
        name: "clock__now".into(),
        args: json!({}),
    });
    assert_eq!(ask(&o, &with_tool()).await.unwrap(), call);
    assert_eq!(ask(&a, &with_tool()).await.unwrap(), call);
    assert_eq!(
        ask(&o, &round_trip()).await.unwrap(),
        LlmReply::Text("done".into())
    );
    assert_eq!(
        ask(&a, &round_trip()).await.unwrap(),
        LlmReply::Text("done".into())
    );
    let on = fake.seen_on("/v1/chat/completions");
    assert_eq!(
        on[0].body["tools"][0],
        json!({"type": "function", "function": {"name": "clock__now", "description": "the time", "parameters": {"type": "object", "properties": {}}}})
    );
    let m = &on[1].body["messages"];
    assert_eq!(m[2]["role"], "assistant");
    assert_eq!(m[2]["tool_calls"][0]["id"], "call_1");
    assert_eq!(m[2]["tool_calls"][0]["function"]["name"], "clock__now");
    assert_eq!(
        m[3],
        json!({"role": "tool", "tool_call_id": "call_1", "content": "noon"})
    );
    let an = fake.seen_on("/v1/messages");
    assert_eq!(
        an[0].body["tools"][0],
        json!({"name": "clock__now", "description": "the time", "input_schema": {"type": "object", "properties": {}}})
    );
    let m = &an[1].body["messages"];
    assert_eq!(m[1]["role"], "assistant");
    assert_eq!(m[1]["content"][0]["type"], "tool_use");
    assert_eq!(
        m[2],
        json!({"role": "user", "content": [{"type": "tool_result", "tool_use_id": "call_1", "content": "noon"}]})
    );
}

/// The rows of the address table of the spec that have a `base_url`. A local http server stands in for the host; the
/// path rule is the same code.
#[tokio::test]
async fn openai_base_url_rows_give_the_addresses_of_the_table() {
    unsafe { std::env::set_var("MG_CHAR_BASE_KEY", "k") };
    for (suffix, label) in [
        ("", "https://host"),
        ("/", "https://host/"),
        ("/v1", "https://host/v1"),
    ] {
        let web = web::start(json!({"choices": [{"message": {"content": "ok"}}]})).await;
        let mut c = config("openai", "MG_CHAR_BASE_KEY");
        c.llm.base_url = Some(format!("{}{}", web.base, suffix));
        assert_eq!(
            ask(&c, &plain()).await.unwrap(),
            LlmReply::Text("ok".into()),
            "{}",
            label
        );
        let seen = web.seen.lock().unwrap();
        assert_eq!(seen.paths, vec!["/v1/chat/completions"], "row {}", label);
        assert_eq!(seen.headers[0].get("authorization").unwrap(), "Bearer k");
    }
}

#[tokio::test]
async fn anthropic_base_url_is_used_and_a_trailing_slash_is_removed() {
    unsafe { std::env::set_var("MG_CHAR_BASE_KEY2", "k") };
    for suffix in ["", "/"] {
        let web = web::start(json!({"content": [{"type": "text", "text": "ok"}]})).await;
        let mut c = config("anthropic", "MG_CHAR_BASE_KEY2");
        c.llm.base_url = Some(format!("{}{}", web.base, suffix));
        ask(&c, &plain()).await.unwrap();
        assert_eq!(web.seen.lock().unwrap().paths, vec!["/v1/messages"]);
    }
}

#[tokio::test]
async fn openai_compatible_is_the_same_as_openai() {
    let web = web::start(json!({"choices": [{"message": {"content": "ok"}}]})).await;
    unsafe { std::env::set_var("MG_CHAR_COMPAT_KEY", "k") };
    let mut c = config("openai-compatible", "MG_CHAR_COMPAT_KEY");
    c.llm.base_url = Some(web.base.clone());
    ask(&c, &plain()).await.unwrap();
    assert_eq!(web.seen.lock().unwrap().paths, vec!["/v1/chat/completions"]);
}

#[tokio::test]
async fn the_messages_of_a_failing_llm_call_are_the_ones_of_0_1_4() {
    let fake = Official::start().await;
    fake.script_openai(vec![Reply::Json(
        429,
        json!({"error": {"message": "slow down"}}),
    )]);
    unsafe { std::env::set_var("MG_CHAR_FAIL_KEY", "k") };
    let mut c = config("openai", "MG_CHAR_FAIL_KEY");
    fake.apply(&mut c);
    assert_eq!(
        ask(&c, &plain()).await.unwrap_err(),
        "https://api.openai.com/v1/chat/completions answered 429: slow down"
    );
    fake.script_openai(vec![Reply::Raw(200, "not json".to_string())]);
    assert_eq!(
        ask(&c, &plain()).await.unwrap_err(),
        "https://api.openai.com/v1/chat/completions did not answer with readable data"
    );
    let mut unset = config("openai", "MG_CHAR_THIS_IS_NOT_SET");
    fake.apply(&mut unset);
    unsafe { std::env::remove_var("MG_CHAR_THIS_IS_NOT_SET") };
    assert_eq!(
        ask(&unset, &plain()).await.unwrap_err(),
        "the variable MG_CHAR_THIS_IS_NOT_SET is not set"
    );
    let unknown = config("gemini", "X");
    let err = from_config(&unknown).err().unwrap();
    assert_eq!(
        err.message,
        "I do not know the language model provider `gemini`"
    );
}
