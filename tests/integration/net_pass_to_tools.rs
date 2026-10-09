// 0.1.3: new file. pass_to_tools gives the proxy settings to tool programs started over stdio, and only to them.
use metagente::mcp::client::McpConnection;
use metagente::runtime::config::NetworkConfig;
use metagente::runtime::net::ClientSet;
use metagente::tools::Args;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

const REPORTER: &str = "agent Reporter\n  goal \"report the proxy variables I was started with\"\n  tool env \"HTTPS_PROXY\" \"https_proxy\" \"HTTP_PROXY\" \"http_proxy\" \"NO_PROXY\" \"no_proxy\"\n  accepts report\n  on report\n    a = env.get name: \"HTTPS_PROXY\"\n    b = env.get name: \"https_proxy\"\n    c = env.get name: \"HTTP_PROXY\"\n    d = env.get name: \"NO_PROXY\"\n    reply \"https={a} lower={b} http={c} no={d}\"\n";

fn network(pass: bool) -> NetworkConfig {
    let mut n = NetworkConfig::default();
    n.proxy.url = Some("http://proxy.example:3128".to_string());
    n.proxy.username_env = Some("PROXY_USER".to_string());
    n.proxy.password_env = Some("PROXY_PASSWORD".to_string());
    n.proxy.no_proxy = vec!["localhost".to_string()];
    n.proxy.pass_to_tools = pass;
    n
}

fn net_with(cfg: NetworkConfig) -> Arc<ClientSet> {
    let env: HashMap<String, String> = [("PROXY_USER", "ann"), ("PROXY_PASSWORD", "pw")]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    Arc::new(ClientSet::with_env(cfg, Duration::from_secs(5), move |n| {
        env.get(n).cloned()
    }))
}

async fn what_the_tool_program_saw(pass: bool) -> String {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("reporter.ag");
    std::fs::write(&file, REPORTER).unwrap();
    let command = format!(
        "{} serve {} --mcp stdio",
        env!("CARGO_BIN_EXE_metagente"),
        file.display()
    );
    let connection = McpConnection::new(command, net_with(network(pass)));
    let value = connection
        .call("rep", "Reporter.report", Args::new())
        .await
        .unwrap_or_else(|e| panic!("{} {:?}", e.message, e.fix));
    value.to_display()
}

#[tokio::test]
async fn with_pass_to_tools_the_tool_program_gets_the_proxy_and_its_login() {
    let before = (
        std::env::var("HTTPS_PROXY").ok(),
        std::env::var("NO_PROXY").ok(),
    );
    let seen = what_the_tool_program_saw(true).await;
    assert!(
        seen.contains("https=http://ann:pw@proxy.example:3128"),
        "{}",
        seen
    );
    assert!(
        seen.contains("lower=http://ann:pw@proxy.example:3128"),
        "{}",
        seen
    );
    assert!(
        seen.contains("http=http://ann:pw@proxy.example:3128"),
        "{}",
        seen
    );
    assert!(seen.contains("no=localhost"), "{}", seen);
    // this program's own environment is not touched
    assert_eq!(
        before,
        (
            std::env::var("HTTPS_PROXY").ok(),
            std::env::var("NO_PROXY").ok()
        )
    );
}

#[tokio::test]
async fn without_pass_to_tools_nothing_is_added() {
    let seen = what_the_tool_program_saw(false).await;
    assert!(!seen.contains("proxy.example"), "{}", seen);
    assert!(!seen.contains("ann:pw"), "{}", seen);
}

#[test]
fn the_variables_are_only_built_when_the_option_is_on() {
    assert!(net_with(network(false)).child_environment().is_empty());
    let on = net_with(network(true)).child_environment();
    assert_eq!(on.len(), 6);
}

#[tokio::test]
async fn a_tool_server_over_http_has_no_child_process_to_give_anything_to() {
    // the option is on, and an http tool server still works: nothing is passed because nothing is started
    let server = crate::common::fake_mcp::start().await;
    let mut cfg = network(true);
    cfg.proxy.url = None;
    cfg.proxy.username_env = None;
    cfg.proxy.password_env = None;
    let connection = McpConnection::new(server.url.clone(), net_with(cfg));
    let mut args = Args::new();
    args.insert(
        "city".to_string(),
        metagente::runtime::value::Value::text("Lisbon"),
    );
    assert!(connection.call("weather", "forecast", args).await.is_ok());
}
