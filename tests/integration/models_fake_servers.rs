// 0.1.5: new file (FR-019). Named models against fake Anthropic and OpenAI that answer for their OFFICIAL host names,
// reached only through [network] proxy and ca_file (common/official.rs). Proves the protocol and the routing of both
// providers, that each key reaches only its own provider, the whole tool round trip, and the plain failures. It does
// NOT prove that the real services accept what Metagente sends: only the live checks of validation.md prove that.
use crate::common::official::{
    Official, Reply, anthropic_text, anthropic_tool_call, openai_text, openai_tool_call,
};
use crate::common::proxy::{self, Mode};
use crate::common::*;
use metagente::llm::providers::{from_config, named_from_config};
use metagente::llm::{ChatMsg, Llm, LlmFailure, LlmReply, LlmRequest};
use metagente::runtime::Runtime;
use metagente::runtime::config::Config;
use metagente::runtime::models::read_models;
use metagente::runtime::net::ClientSet;
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

fn request() -> LlmRequest {
    LlmRequest {
        system: "You are a test".to_string(),
        messages: vec![ChatMsg::User("hi".to_string())],
        tools: vec![],
    }
}

fn config_for(toml: &str, tweak: impl FnOnce(&mut Config)) -> Config {
    let table: toml::Table = toml.parse().expect("metagente.toml");
    let llm = table
        .get("llm")
        .and_then(|v| v.as_table())
        .cloned()
        .unwrap_or_default();
    let mut config = Config {
        models: read_models(&llm, toml, Path::new("metagente.toml"))
            .unwrap_or_else(|d| panic!("{}", d.render())),
        ..Config::default()
    };
    if let Some(table) = table.get("llm").and_then(|v| v.as_table()) {
        config.llm.provider = table
            .get("provider")
            .and_then(|v| v.as_str())
            .map(String::from);
        config.llm.model = table
            .get("model")
            .and_then(|v| v.as_str())
            .map(String::from);
        config.llm.api_key_env = table
            .get("api_key_env")
            .and_then(|v| v.as_str())
            .map(String::from);
        config.llm.base_url = table
            .get("base_url")
            .and_then(|v| v.as_str())
            .map(String::from);
    }
    tweak(&mut config);
    config
}

fn named(config: &Config) -> HashMap<String, Arc<dyn Llm>> {
    let net = Arc::new(ClientSet::for_config(config));
    named_from_config(config, net)
}

async fn detailed(llm: &Arc<dyn Llm>) -> Result<LlmReply, LlmFailure> {
    llm.complete_detailed(&request()).await
}

fn runtime(config: Config, default: Option<Arc<dyn Llm>>) -> Arc<Runtime> {
    let net = Arc::new(ClientSet::for_config(&config));
    let models = named_from_config(&config, net.clone());
    Runtime::with_models(config, default, models, net)
}

fn set(name: &str, value: &str) {
    unsafe { std::env::set_var(name, value) };
}

fn unset(name: &str) {
    unsafe { std::env::remove_var(name) };
}

fn is_metagente_user_agent(ua: &str) -> bool {
    ua.strip_prefix("metagente/")
        .map(|v| !v.is_empty() && v.chars().all(|c| c.is_ascii_digit() || c == '.'))
        .unwrap_or(false)
}

// ---------- the protocol of each provider, as [llm] sends it ----------

#[tokio::test]
async fn a_named_openai_model_sends_what_llm_sends_for_openai_to_the_official_address() {
    let fake = Official::start().await;
    fake.script_openai(vec![openai_text("hello"), openai_text("hello")]);
    set("MG_T_OA_KEY", "key-oa");
    let toml =
        "[llm.gpt]\nprovider = \"openai\"\nmodel = \"gpt-test\"\napi_key_env = \"MG_T_OA_KEY\"\n";
    let config = config_for(toml, |c| fake.apply(c));
    let llm = named(&config).remove("gpt").unwrap();
    assert_eq!(
        detailed(&llm).await.unwrap(),
        LlmReply::Text("hello".into())
    );
    // the same request through [llm]
    let mut plain = Config::default();
    plain.llm.provider = Some("openai".into());
    plain.llm.model = Some("gpt-test".into());
    plain.llm.api_key_env = Some("MG_T_OA_KEY".into());
    fake.apply(&mut plain);
    from_config(&plain)
        .unwrap()
        .unwrap()
        .complete(&request())
        .await
        .unwrap();
    let seen = fake.seen();
    assert_eq!(seen.len(), 2);
    let (a, b) = (&seen[0], &seen[1]);
    assert_eq!(a.path, "/v1/chat/completions");
    assert_eq!(a.host, "api.openai.com");
    assert_eq!(a.authorization.as_deref(), Some("Bearer key-oa"));
    assert!(a.x_api_key.is_none());
    assert!(is_metagente_user_agent(&a.user_agent));
    assert_eq!(
        a.body, b.body,
        "a named model sends the request [llm] sends"
    );
    assert_eq!(a.body["model"], "gpt-test");
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
async fn a_named_anthropic_model_sends_what_llm_sends_for_anthropic_to_the_official_address() {
    let fake = Official::start().await;
    fake.script_anthropic(vec![anthropic_text("hello")]);
    set("MG_T_AN_KEY", "key-an");
    // no model written: the usual model of Anthropic applies
    let toml = "[llm.claude]\nprovider = \"anthropic\"\napi_key_env = \"MG_T_AN_KEY\"\n";
    let config = config_for(toml, |c| fake.apply(c));
    let llm = named(&config).remove("claude").unwrap();
    assert_eq!(
        detailed(&llm).await.unwrap(),
        LlmReply::Text("hello".into())
    );
    let mut plain = Config::default();
    plain.llm.provider = Some("anthropic".into());
    plain.llm.api_key_env = Some("MG_T_AN_KEY".into());
    fake.apply(&mut plain);
    from_config(&plain)
        .unwrap()
        .unwrap()
        .complete(&request())
        .await
        .unwrap();
    let seen = fake.seen();
    let (a, b) = (&seen[0], &seen[1]);
    assert_eq!(a.path, "/v1/messages");
    assert_eq!(a.host, "api.anthropic.com");
    assert_eq!(a.x_api_key.as_deref(), Some("key-an"));
    assert_eq!(a.anthropic_version.as_deref(), Some("2023-06-01"));
    assert!(a.authorization.is_none());
    assert_eq!(a.body, b.body);
    assert_eq!(a.body["model"], "claude-sonnet-5-5");
    assert_eq!(a.body["max_tokens"], 1024);
}

// ---------- the whole tool round trip, through `think` ----------

const WITH_TOOL: &str = "agent A\n  goal \"Tell the time\"\n  tool clock\n  accepts go\n  on go\n    reply think \"time?\" using live\n";

#[tokio::test]
async fn the_tool_round_trip_works_for_a_named_openai_model() {
    let fake = Official::start().await;
    fake.script_openai(vec![
        openai_tool_call("call_1", "clock__now", json!({})),
        openai_text("It is late."),
    ]);
    set("MG_T_RT_OA", "k");
    let toml = "[llm.live]\nprovider = \"openai\"\nmodel = \"m\"\napi_key_env = \"MG_T_RT_OA\"\n";
    let rt = runtime(config_for(toml, |c| fake.apply(c)), None);
    let reply = run_source(&rt, WITH_TOOL, "go", &[]).await.unwrap();
    assert_eq!(reply.to_display(), "It is late.");
    let on = fake.seen_on("/v1/chat/completions");
    assert_eq!(on.len(), 2);
    let names: Vec<String> = on[0].body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["function"]["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["clock__now", "clock__wait"]);
    let m = &on[1].body["messages"];
    assert_eq!(m[2]["tool_calls"][0]["function"]["name"], "clock__now");
    assert_eq!(m[3]["role"], "tool");
    assert!(m[3]["content"].as_str().unwrap().contains("unix"));
}

#[tokio::test]
async fn the_tool_round_trip_works_for_a_named_anthropic_model() {
    let fake = Official::start().await;
    fake.script_anthropic(vec![
        anthropic_tool_call("toolu_1", "clock__now", json!({})),
        anthropic_text("It is late."),
    ]);
    set("MG_T_RT_AN", "k");
    let toml =
        "[llm.live]\nprovider = \"anthropic\"\nmodel = \"m\"\napi_key_env = \"MG_T_RT_AN\"\n";
    let rt = runtime(config_for(toml, |c| fake.apply(c)), None);
    let reply = run_source(&rt, WITH_TOOL, "go", &[]).await.unwrap();
    assert_eq!(reply.to_display(), "It is late.");
    let on = fake.seen_on("/v1/messages");
    assert_eq!(on.len(), 2);
    assert_eq!(on[0].body["tools"][0]["name"], "clock__now");
    let m = &on[1].body["messages"];
    assert_eq!(m[1]["content"][0]["type"], "tool_use");
    assert_eq!(m[2]["content"][0]["type"], "tool_result");
    assert_eq!(m[2]["content"][0]["tool_use_id"], "toolu_1");
}

// ---------- two providers in one agent: each key reaches only its own provider ----------

#[tokio::test]
async fn each_key_reaches_only_its_own_provider() {
    let fake = Official::start().await;
    fake.script_anthropic(vec![anthropic_text("short query")]);
    fake.script_openai(vec![openai_text("The final answer.")]);
    set("MG_T_TWO_FAST_KEY", "anthropic-secret-111");
    set("MG_T_TWO_STRONG_KEY", "openai-secret-222");
    set("MG_T_TWO_FAST_MODEL", "small-model");
    set("MG_T_TWO_STRONG_MODEL", "big-model");
    let toml = "[llm.fast]\nprovider = \"anthropic\"\nmodel_env = \"MG_T_TWO_FAST_MODEL\"\napi_key_env = \"MG_T_TWO_FAST_KEY\"\n\n[llm.strong]\nprovider = \"openai\"\nmodel_env = \"MG_T_TWO_STRONG_MODEL\"\napi_key_env = \"MG_T_TWO_STRONG_KEY\"\n";
    let rt = runtime(config_for(toml, |c| fake.apply(c)), None);
    let source = "agent Assistant\n  goal \"two steps\"\n  accepts ask question\n  on ask\n    short = think \"Rewrite: {question}\" using fast\n    reply think \"Answer: {short}\" using strong\n";
    let reply = run_source(&rt, source, "ask", &[("question", "why")])
        .await
        .unwrap();
    assert_eq!(reply.to_display(), "The final answer.");
    let anthropic = fake.seen_on("/v1/messages");
    let openai = fake.seen_on("/v1/chat/completions");
    assert_eq!(anthropic.len(), 1);
    assert_eq!(openai.len(), 1);
    assert_eq!(
        anthropic[0].x_api_key.as_deref(),
        Some("anthropic-secret-111")
    );
    assert_eq!(anthropic[0].body["model"], "small-model");
    assert_eq!(
        openai[0].authorization.as_deref(),
        Some("Bearer openai-secret-222")
    );
    assert_eq!(openai[0].body["model"], "big-model");
    let everything_anthropic = format!("{:?}", anthropic);
    let everything_openai = format!("{:?}", openai);
    assert!(!everything_anthropic.contains("openai-secret-222"));
    assert!(!everything_openai.contains("anthropic-secret-111"));
    assert_eq!(anthropic[0].host, "api.anthropic.com");
    assert_eq!(openai[0].host, "api.openai.com");
    // the second call carries the answer of the first
    assert_eq!(
        openai[0].body["messages"][1]["content"],
        "Answer: short query"
    );
}

#[tokio::test]
async fn llm_with_a_base_url_goes_to_its_own_address_and_a_named_model_to_the_official_one() {
    let fake = Official::start().await;
    fake.script_anthropic(vec![anthropic_text("named")]);
    let web =
        crate::common::web::start(json!({"choices": [{"message": {"content": "default"}}]})).await;
    set("MG_T_BASE_KEY", "k1");
    set("MG_T_NAMED_KEY", "k2");
    let toml = format!(
        "[llm]\nprovider = \"openai\"\nmodel = \"m\"\nbase_url = \"{}\"\napi_key_env = \"MG_T_BASE_KEY\"\n\n[llm.fast]\nprovider = \"anthropic\"\nmodel = \"c\"\napi_key_env = \"MG_T_NAMED_KEY\"\n",
        web.base
    );
    let config = config_for(&toml, |c| fake.apply(c));
    let net = Arc::new(ClientSet::for_config(&config));
    let default = from_config(&config).unwrap().unwrap();
    let models = named_from_config(&config, net.clone());
    let rt = Runtime::with_models(config, Some(default), models, net);
    let source = "agent A\n  goal \"x\"\n  accepts go\n  on go\n    a = think \"1\"\n    b = think \"2\" using fast\n    reply \"{a} {b}\"\n";
    assert_eq!(
        run_source(&rt, source, "go", &[])
            .await
            .unwrap()
            .to_display(),
        "default named"
    );
    assert_eq!(web.seen.lock().unwrap().paths, vec!["/v1/chat/completions"]);
    assert_eq!(fake.seen().len(), 1);
    assert_eq!(fake.seen()[0].host, "api.anthropic.com");
}

// ---------- the variables ----------

#[tokio::test]
async fn model_env_is_read_at_the_first_use_and_kept_and_the_key_is_read_every_call() {
    let fake = Official::start().await;
    fake.script_openai(vec![openai_text("ok")]);
    let toml = "[llm.late]\nprovider = \"openai\"\nmodel_env = \"MG_T_LATE_MODEL\"\napi_key_env = \"MG_T_LATE_KEY\"\n";
    let config = config_for(toml, |c| fake.apply(c));
    let llm = named(&config).remove("late").unwrap();
    // nothing is needed when the file is read; the first use finds the variables unset
    unset("MG_T_LATE_MODEL");
    unset("MG_T_LATE_KEY");
    let err = detailed(&llm).await.unwrap_err();
    assert_eq!(
        err.message,
        "The model late needs the variable MG_T_LATE_KEY, and it is not set."
    );
    // a key set after the start works
    set("MG_T_LATE_KEY", "key-one");
    let err = detailed(&llm).await.unwrap_err();
    assert_eq!(
        err.message,
        "The model late needs the variable MG_T_LATE_MODEL, and it is empty or not set."
    );
    assert_eq!(
        err.fix.as_deref(),
        Some("set the variable, or change model_env")
    );
    set("MG_T_LATE_MODEL", "first-model");
    detailed(&llm).await.unwrap();
    // the model name is kept; the key is read again
    set("MG_T_LATE_MODEL", "second-model");
    set("MG_T_LATE_KEY", "key-two");
    detailed(&llm).await.unwrap();
    let seen = fake.seen();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].body["model"], "first-model");
    assert_eq!(seen[1].body["model"], "first-model");
    assert_eq!(seen[0].authorization.as_deref(), Some("Bearer key-one"));
    assert_eq!(seen[1].authorization.as_deref(), Some("Bearer key-two"));
    // an empty variable is as good as unset
    set("MG_T_LATE_KEY", "  ");
    assert!(
        detailed(&llm)
            .await
            .unwrap_err()
            .message
            .contains("is not set")
    );
}

// ---------- plain failures: our own sentences, never the text of the service ----------

#[tokio::test]
async fn an_answer_with_an_error_is_the_nickname_the_provider_the_status_and_the_fix_of_that_status()
 {
    let fake = Official::start().await;
    set("MG_T_ST_KEY", "k");
    let toml = "[llm.gpt]\nprovider = \"openai\"\nmodel = \"secret-model-name\"\napi_key_env = \"MG_T_ST_KEY\"\n";
    let config = config_for(toml, |c| fake.apply(c));
    let llm = named(&config).remove("gpt").unwrap();
    let cases = [
        (401, "check the key and that it belongs to this provider"),
        (403, "check the key and that it belongs to this provider"),
        (
            400,
            "check the model name and that this model accepts tools",
        ),
        (
            404,
            "check the model name and that this model accepts tools",
        ),
        (429, "wait and try again, or check the plan of the service"),
        (500, "the service has a problem, try again later"),
        (503, "the service has a problem, try again later"),
        (418, "check the settings of the model"),
    ];
    for (status, fix) in cases {
        // the body repeats the model name, a masked key and the address: none of it may reach the message
        fake.script_openai(vec![Reply::Json(
            status,
            json!({"error": {"message": "Incorrect API key sk-abc****wxyz for secret-model-name at https://api.openai.com/v1/chat/completions"}}),
        )]);
        let err = detailed(&llm).await.unwrap_err();
        assert!(err.plain);
        assert_eq!(
            err.message,
            format!("The model gpt (openai) answered with status {}.", status)
        );
        assert_eq!(err.fix.as_deref(), Some(fix), "{}", status);
        for forbidden in ["sk-abc", "secret-model-name", "api.openai.com", "Incorrect"] {
            assert!(!err.message.contains(forbidden), "{}", err.message);
            assert!(!err.fix.clone().unwrap_or_default().contains(forbidden));
        }
    }
}

#[tokio::test]
async fn an_answer_that_cannot_be_read_is_a_plain_sentence() {
    let fake = Official::start().await;
    set("MG_T_BAD_KEY", "k");
    let toml = "[llm.claude]\nprovider = \"anthropic\"\napi_key_env = \"MG_T_BAD_KEY\"\n";
    let config = config_for(toml, |c| fake.apply(c));
    let llm = named(&config).remove("claude").unwrap();
    for reply in [
        Reply::Raw(200, "not json at all".to_string()),
        Reply::Json(200, json!({"unexpected": true})),
    ] {
        fake.script_anthropic(vec![reply]);
        let err = detailed(&llm).await.unwrap_err();
        assert_eq!(
            err.message,
            "The model claude (anthropic) answered with something I could not read."
        );
        assert_eq!(err.fix.as_deref(), Some("try again later"));
    }
}

#[tokio::test]
async fn an_untrusted_certificate_and_a_refusing_proxy_are_plain_sentences() {
    let fake = Official::start().await;
    set("MG_T_NET_KEY", "k");
    let toml = "[llm.gpt]\nprovider = \"openai\"\nmodel = \"m\"\napi_key_env = \"MG_T_NET_KEY\"\n";
    // the proxy routes to the fake, but the authority is not given: a certificate problem
    let config = config_for(toml, |c| c.network.proxy.url = Some(fake.proxy_url.clone()));
    let llm = named(&config).remove("gpt").unwrap();
    let err = detailed(&llm).await.unwrap_err();
    assert_eq!(
        err.message,
        "The model gpt (openai) could not be reached: certificate."
    );
    assert_eq!(
        err.fix.as_deref(),
        Some("add the authority with ca_file in [network]")
    );
    // a proxy that refuses
    let refusing = proxy::start(Mode::Status(403)).await;
    let config = config_for(toml, |c| c.network.proxy.url = Some(refusing.url()));
    let llm = named(&config).remove("gpt").unwrap();
    let err = detailed(&llm).await.unwrap_err();
    assert_eq!(
        err.message,
        "The model gpt (openai) could not be reached: proxy."
    );
    assert_eq!(
        err.fix.as_deref(),
        Some("check the [network.proxy] settings")
    );
    assert!(!err.message.contains("127.0.0.1"), "{}", err.message);
}

#[tokio::test]
async fn a_model_that_never_answers_is_a_timeout_sentence_with_the_nickname() {
    let fake = Official::start().await;
    fake.script_anthropic(vec![Reply::Hang]);
    set("MG_T_HANG_KEY", "k");
    let toml =
        "[llm.live]\nprovider = \"anthropic\"\nmodel = \"m\"\napi_key_env = \"MG_T_HANG_KEY\"\n";
    let rt = runtime(
        config_for(toml, |c| {
            fake.apply(c);
            c.timeout_seconds = 1;
        }),
        None,
    );
    let started = std::time::Instant::now();
    let err = run_source(&rt, WITH_TOOL, "go", &[]).await.unwrap_err();
    assert_eq!(
        err.message,
        "The model live (anthropic) took too long to answer."
    );
    assert_eq!(err.suggestion.as_deref(), Some("try again in a moment"));
    assert!(started.elapsed() < std::time::Duration::from_secs(20));
}

#[tokio::test]
async fn a_failure_inside_think_shows_the_plain_sentence_without_the_llm_wrapper() {
    let fake = Official::start().await;
    fake.script_openai(vec![Reply::Json(
        429,
        json!({"error": {"message": "slow down"}}),
    )]);
    set("MG_T_WRAP_KEY", "k");
    let toml =
        "[llm.live]\nprovider = \"openai\"\nmodel = \"m\"\napi_key_env = \"MG_T_WRAP_KEY\"\n";
    let rt = runtime(config_for(toml, |c| fake.apply(c)), None);
    let err = run_source(&rt, WITH_TOOL, "go", &[]).await.unwrap_err();
    assert_eq!(
        err.message,
        "The model live (openai) answered with status 429."
    );
    assert_eq!(
        err.suggestion.as_deref(),
        Some("wait and try again, or check the plan of the service")
    );
    assert!(!err.message.contains("the language model could not answer"));
}
