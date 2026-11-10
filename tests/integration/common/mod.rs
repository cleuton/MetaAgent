#![allow(dead_code)]
//! Helpers shared by the integration tests: a runtime, an agent runner, and fake servers.

pub mod a2a;
pub mod a2a_tls; // 0.1.3: the same server over TLS
pub mod auth; // 0.1.4: validators, `metagente serve` children and hand written calls for [auth]
pub mod fake_a2a; // 0.1.4: a fake A2A agent that records the Authorization header
pub mod fake_mcp;
pub mod official; // 0.1.5: fake Anthropic and OpenAI for their official host names, through [network] only
pub mod proxy; // 0.1.3: test proxy
pub mod python; // 0.1.3: the Python side of the interop tests
pub mod self_signed; // 0.1.3: certificate files and `metagente serve` as a child process
pub mod tls_server; // 0.1.3: test HTTPS server
pub mod web;

use metagente::diagnostics::Diagnostic;
use metagente::llm::Llm;
use metagente::runtime::Runtime;
use metagente::runtime::config::Config;
use metagente::runtime::interpreter::Agent;
use metagente::runtime::task::{CallContext, new_task_id};
use metagente::runtime::value::Value;
use metagente::tools::Args;
use std::path::Path;
use std::sync::Arc;

pub fn project_file(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e))
}

pub fn runtime_in(
    root: &Path,
    llm: Option<Arc<dyn Llm>>,
    tweak: impl FnOnce(&mut Config),
) -> Arc<Runtime> {
    let mut config = Config {
        root: root.to_path_buf(),
        ..Config::default()
    };
    tweak(&mut config);
    Runtime::new(config, llm)
}

pub fn args(pairs: &[(&str, &str)]) -> Args {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), Value::Text(v.to_string())))
        .collect()
}

pub fn agent_from(rt: &Arc<Runtime>, source: &str, name: Option<&str>) -> Agent {
    let agents = metagente::lang::parse_file("test.ag", None, source)
        .unwrap_or_else(|d| panic!("{}", d.render()));
    let def = match name {
        Some(n) => agents
            .into_iter()
            .find(|a| a.name == n)
            .expect("agent not found"),
        None => agents.into_iter().next().expect("no agent"),
    };
    Agent::new(rt.clone(), Arc::new(def)).unwrap_or_else(|d| panic!("{}", d.render()))
}

pub async fn ask(
    agent: &Agent,
    message: &str,
    params: &[(&str, &str)],
) -> Result<Value, Diagnostic> {
    agent
        .handle(message, args(params), &CallContext::new(new_task_id()))
        .await
}

pub async fn run_source(
    rt: &Arc<Runtime>,
    source: &str,
    message: &str,
    params: &[(&str, &str)],
) -> Result<Value, Diagnostic> {
    let agent = agent_from(rt, source, None);
    ask(&agent, message, params).await
}

/// 0.1.5: a runtime whose `[llm.NAME]` models are the ones in `toml` (read the way `run` reads them), each answered by
/// the scripted model given for its nickname; `default` answers a `think` without `using`.
pub fn runtime_with_models(
    root: &Path,
    toml: &str,
    default: Option<Arc<dyn Llm>>,
    named: Vec<(&str, Arc<dyn Llm>)>,
) -> Arc<Runtime> {
    let table: toml::Table = toml.parse().expect("metagente.toml");
    let llm = table
        .get("llm")
        .and_then(|v| v.as_table())
        .cloned()
        .unwrap_or_default();
    let mut config = Config {
        root: root.to_path_buf(),
        ..Config::default()
    };
    config.models =
        metagente::runtime::models::read_models(&llm, toml, Path::new("metagente.toml"))
            .unwrap_or_else(|d| panic!("{}", d.render()));
    let net = Arc::new(metagente::runtime::net::ClientSet::for_config(&config));
    let models = named.into_iter().map(|(n, m)| (n.to_string(), m)).collect();
    Runtime::with_models(config, default, models, net)
}
