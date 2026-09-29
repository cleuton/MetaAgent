use crate::common::a2a::WEATHER;
use rmcp::ServiceExt;
use rmcp::model::CallToolRequestParams;
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};

fn params(name: &'static str, args: serde_json::Value) -> CallToolRequestParams {
    CallToolRequestParams::new(name).with_arguments(args.as_object().cloned().unwrap_or_default())
}

fn text_of(result: &rmcp::model::CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|c| serde_json::to_value(c).ok())
        .filter_map(|j| j.get("text").and_then(|t| t.as_str()).map(String::from))
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn an_mcp_client_lists_and_calls_agents_served_over_stdio() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("weather.ag"), WEATHER).unwrap();
    let transport = TokioChildProcess::new(
        tokio::process::Command::new(env!("CARGO_BIN_EXE_metagente")).configure(|cmd| {
            cmd.current_dir(dir.path())
                .args(["serve", "weather.ag", "--mcp", "stdio"]);
            cmd.stderr(std::process::Stdio::null());
        }),
    )
    .unwrap();
    let client = ().serve(transport).await.expect("connect");

    let tools = client.list_all_tools().await.unwrap();
    let names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
    assert_eq!(names, vec!["Weather.ask", "Weather.broken"]);
    let ask = &tools[0];
    assert_eq!(ask.description.as_deref(), Some("weather for a city"));
    assert_eq!(ask.input_schema["properties"]["city"]["type"], "string");
    assert_eq!(ask.input_schema["required"][0], "city");

    let ok = client
        .call_tool(params("Weather.ask", serde_json::json!({"city": "Lisbon"})))
        .await
        .unwrap();
    assert_eq!(text_of(&ok), "sunny in Lisbon");
    assert_ne!(ok.is_error, Some(true));

    // An agent failing is a tool error the caller can read, not a broken connection.
    let failed = client
        .call_tool(params("Weather.broken", serde_json::json!({})))
        .await
        .unwrap();
    assert_eq!(failed.is_error, Some(true));
    assert!(
        text_of(&failed).contains("the barometer exploded"),
        "{}",
        text_of(&failed)
    );

    // A missing value is explained.
    let missing = client
        .call_tool(params("Weather.ask", serde_json::json!({})))
        .await
        .unwrap();
    assert_eq!(missing.is_error, Some(true));
    assert!(
        text_of(&missing).contains("needs a value for `city`"),
        "{}",
        text_of(&missing)
    );

    // A tool that does not exist is not supported.
    let err = client
        .call_tool(params("Weather.dance", serde_json::json!({})))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Not supported"), "{}", err);
    let _ = client.cancel().await;
}

#[tokio::test]
async fn a_metagente_agent_can_use_another_one_served_over_mcp_http() {
    use crate::common::*;
    use std::process::{Command, Stdio};
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("weather.ag"), WEATHER).unwrap();
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mut server = Command::new(env!("CARGO_BIN_EXE_metagente"))
        .current_dir(dir.path())
        .args(["serve", "weather.ag", "--mcp", &port.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let start = std::time::Instant::now();
    while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(
            start.elapsed().as_secs() < 10,
            "the MCP server did not start"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let rt = runtime_in(dir.path(), None, |_| {});
    // The tool is called `Weather.ask`, so the action has a dot in it.
    let caller = format!(
        "agent Client\n  goal \"Use the served weather agent\"\n  tool weather from mcp \"http://127.0.0.1:{}/mcp\"\n  accepts go\n  on go\n    reply weather.Weather.ask city: \"Lisbon\"\n",
        port
    );
    let reply = run_source(&rt, &caller, "go", &[]).await;
    let _ = server.kill();
    let _ = server.wait();
    assert_eq!(reply.unwrap().to_display(), "sunny in Lisbon");
}

#[tokio::test]
async fn the_mcp_client_can_start_a_server_program_and_talk_to_it_over_stdio() {
    use crate::common::*;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("weather.ag"), WEATHER).unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    // `tool ... from mcp "<command>"` starts the program itself; here the program is Metagente serving an agent.
    let caller = format!(
        "agent Client\n  goal \"Use the served weather agent\"\n  tool weather from mcp \"{} serve {} --mcp stdio\"\n  accepts go\n  on go\n    reply weather.Weather.ask city: \"Porto\"\n",
        env!("CARGO_BIN_EXE_metagente"),
        dir.path().join("weather.ag").display()
    );
    let reply = run_source(&rt, &caller, "go", &[]).await.unwrap();
    assert_eq!(reply.to_display(), "sunny in Porto");
}
