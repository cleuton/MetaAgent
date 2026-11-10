// 0.1.5: new file (US4). Each named model keeps every security rule of the single model: its key variable is hidden from
// `tool env`, its address is always https so [secure] llm and a secure agent are satisfied, and the network settings
// ([network] proxy, ca_file, llm_allow_self_signed) apply to it.
use crate::common::official::{Official, anthropic_text, openai_text};
use crate::common::*;
use metagente::llm::providers::named_from_config;
use metagente::runtime::Runtime;
use metagente::runtime::config::Config;
use metagente::runtime::models::read_models;
use metagente::runtime::net::ClientSet;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;

fn set(name: &str, value: &str) {
    unsafe { std::env::set_var(name, value) };
}

const READ_ENV: &str = "agent E\n  goal \"x\"\n  tool env \"MG_SEC_FAST_KEY\" \"ANTHROPIC_API_KEY\" \"OPENAI_API_KEY\" \"MG_SEC_OK\"\n  accepts get name\n  on get\n    reply env.get name: name\n";

fn runtime_for(toml: &str, tweak: impl FnOnce(&mut Config)) -> Arc<Runtime> {
    let table: toml::Table = toml.parse().unwrap();
    let llm = table
        .get("llm")
        .and_then(|v| v.as_table())
        .cloned()
        .unwrap_or_default();
    let mut config = Config {
        models: read_models(&llm, toml, Path::new("metagente.toml")).unwrap(),
        ..Config::default()
    };
    tweak(&mut config);
    let net = Arc::new(ClientSet::for_config(&config));
    let models = named_from_config(&config, net.clone());
    Runtime::with_models(config, None, models, net)
}

#[tokio::test]
async fn the_key_variable_of_a_named_model_is_hidden_from_tool_env_even_when_declared() {
    set("MG_SEC_FAST_KEY", "value-that-must-never-show");
    set("MG_SEC_OK", "fine");
    set("ANTHROPIC_API_KEY", "usual-anthropic-must-never-show");
    set("OPENAI_API_KEY", "usual-openai-must-never-show");
    let rt = runtime_for(
        "[llm.fast]\nprovider = \"anthropic\"\napi_key_env = \"MG_SEC_FAST_KEY\"\n",
        |_| {},
    );
    let agent = agent_from(&rt, READ_ENV, None);
    assert_eq!(
        ask(&agent, "get", &[("name", "MG_SEC_OK")])
            .await
            .unwrap()
            .to_display(),
        "fine"
    );
    for hidden in ["MG_SEC_FAST_KEY", "ANTHROPIC_API_KEY", "OPENAI_API_KEY"] {
        let err = ask(&agent, "get", &[("name", hidden)])
            .await
            .unwrap_err()
            .render();
        assert!(
            err.contains(&format!(
                "`{}` holds the key for the language model, and agents can never read it",
                hidden
            )),
            "{}",
            err
        );
        assert!(!err.contains("must-never-show"), "{}", err);
    }
}

#[tokio::test]
async fn a_model_with_no_api_key_env_hides_the_usual_variable_of_its_provider() {
    set("OPENAI_API_KEY", "usual-openai-must-never-show");
    let rt = runtime_for("[llm.gpt]\nprovider = \"openai\"\nmodel = \"m\"\n", |_| {});
    let agent = agent_from(&rt, READ_ENV, None);
    let err = ask(&agent, "get", &[("name", "OPENAI_API_KEY")])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("agents can never read it"), "{}", err);
    assert!(!err.contains("must-never-show"));
}

const FAST_THINK: &str =
    "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply think \"hi\" using fast\n";

#[tokio::test]
async fn secure_llm_and_a_secure_agent_are_satisfied_by_a_named_model() {
    let fake = Official::start().await;
    fake.script_anthropic(vec![anthropic_text("hello")]);
    set("MG_SEC_NAMED_KEY", "k");
    let toml =
        "[llm.fast]\nprovider = \"anthropic\"\nmodel = \"m\"\napi_key_env = \"MG_SEC_NAMED_KEY\"\n";
    let rt = runtime_for(toml, |c| {
        fake.apply(c);
        c.secure.llm = true;
        c.secure.agents = vec!["A".to_string()];
    });
    assert_eq!(
        run_source(&rt, FAST_THINK, "go", &[])
            .await
            .unwrap()
            .to_display(),
        "hello"
    );
    assert_eq!(fake.seen().len(), 1);
}

#[tokio::test]
async fn an_http_address_in_llm_does_not_stop_a_named_model_but_still_stops_a_plain_think() {
    let fake = Official::start().await;
    fake.script_anthropic(vec![anthropic_text("hello")]);
    set("MG_SEC_NAMED_KEY2", "k");
    let toml = "[llm.fast]\nprovider = \"anthropic\"\nmodel = \"m\"\napi_key_env = \"MG_SEC_NAMED_KEY2\"\n";
    let table: toml::Table = toml.parse().unwrap();
    let mut config = Config {
        models: read_models(
            table["llm"].as_table().unwrap(),
            toml,
            Path::new("metagente.toml"),
        )
        .unwrap(),
        ..Config::default()
    };
    fake.apply(&mut config);
    config.secure.llm = true;
    config.llm.provider = Some("openai".to_string());
    config.llm.model = Some("x".to_string());
    config.llm.base_url = Some("http://gateway.internal:8080".to_string());
    let net = Arc::new(ClientSet::for_config(&config));
    let default = metagente::llm::providers::from_config_with(&config, net.clone()).unwrap();
    let models = named_from_config(&config, net.clone());
    let rt = Runtime::with_models(config, default, models, net);
    assert_eq!(
        run_source(&rt, FAST_THINK, "go", &[])
            .await
            .unwrap()
            .to_display(),
        "hello"
    );
    let plain = "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply think \"hi\"\n";
    let err = run_source(&rt, plain, "go", &[])
        .await
        .unwrap_err()
        .render();
    assert!(
        err.contains(
            "The model address in [llm] uses http, and [secure] llm is on. Nothing was sent."
        ),
        "{}",
        err
    );
}

#[test]
fn check_passes_with_secure_llm_and_only_named_models() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.ag"), FAST_THINK).unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        format!(
            "[version]\nminimum = \"{}\"\n\n[secure]\nllm = true\n\n[llm.fast]\nprovider = \"anthropic\"\nmodel = \"m\"\napi_key_env = \"MG_SEC_CHECK_KEY\"\n",
            env!("CARGO_PKG_VERSION")
        ),
    )
    .unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_metagente"))
        .current_dir(dir.path())
        .args(["check", "a.ag"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[tokio::test]
async fn a_proxy_in_network_carries_the_call_of_a_named_model() {
    let fake = Official::start().await;
    fake.script_openai(vec![openai_text("hello")]);
    set("MG_SEC_PROXY_KEY", "k");
    let toml =
        "[llm.fast]\nprovider = \"openai\"\nmodel = \"m\"\napi_key_env = \"MG_SEC_PROXY_KEY\"\n";
    let rt = runtime_for(toml, |c| fake.apply(c));
    run_source(&rt, FAST_THINK, "go", &[]).await.unwrap();
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
async fn ca_file_applies_and_llm_allow_self_signed_opens_only_the_model_connection() {
    let fake = Official::start().await;
    fake.script_openai(vec![openai_text("hello")]);
    set("MG_SEC_CA_KEY", "k");
    let toml =
        "[llm.fast]\nprovider = \"openai\"\nmodel = \"m\"\napi_key_env = \"MG_SEC_CA_KEY\"\n";
    // without the authority the certificate is not trusted
    let rt = runtime_for(toml, |c| c.network.proxy.url = Some(fake.proxy_url.clone()));
    let err = run_source(&rt, FAST_THINK, "go", &[]).await.unwrap_err();
    assert_eq!(
        err.message,
        "The model fast (openai) could not be reached: certificate."
    );
    // llm_allow_self_signed is the switch of the model connection
    let rt = runtime_for(toml, |c| {
        c.network.proxy.url = Some(fake.proxy_url.clone());
        c.network.llm_allow_self_signed = true;
    });
    assert_eq!(
        run_source(&rt, FAST_THINK, "go", &[])
            .await
            .unwrap()
            .to_display(),
        "hello"
    );
    // allow_self_signed (agents and tools) does not open the model connection
    let rt = runtime_for(toml, |c| {
        c.network.proxy.url = Some(fake.proxy_url.clone());
        c.network.allow_self_signed = true;
    });
    assert!(run_source(&rt, FAST_THINK, "go", &[]).await.is_err());
}
