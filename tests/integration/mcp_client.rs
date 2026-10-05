use crate::common::fake_mcp;
use crate::common::*;
use metagente::runtime::value::Value;

fn weather_source(url: &str) -> String {
    project_file("examples/weather.ag").replace("npx -y weather-mcp", url)
}

#[tokio::test]
async fn weather_example_calls_an_external_mcp_tool() {
    let server = fake_mcp::start().await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let reply = run_source(
        &rt,
        &weather_source(&server.url),
        "ask",
        &[("city", "Lisbon")],
    )
    .await
    .unwrap();
    assert_eq!(reply, Value::text("In Lisbon it will be sunny, 24 degrees"));
}

#[tokio::test]
async fn wrong_parameter_name_lists_what_the_tool_expects() {
    let server = fake_mcp::start().await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let source = format!(
        "agent W\n  goal \"w\"\n  tool weather from mcp \"{}\"\n  accepts go\n  on go\n    x = weather.forecast town: \"Lisbon\"\n    reply x\n",
        server.url
    );
    let err = run_source(&rt, &source, "go", &[])
        .await
        .unwrap_err()
        .render();
    assert!(
        err.contains("`forecast` needs a value for `city`"),
        "{}",
        err
    );
    assert!(err.contains("city: string"), "{}", err);
    assert!(err.contains("line 6"), "{}", err);
}

#[tokio::test]
async fn unknown_tool_action_suggests_the_closest() {
    let server = fake_mcp::start().await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let source = format!(
        "agent W\n  goal \"w\"\n  tool weather from mcp \"{}\"\n  accepts go\n  on go\n    x = weather.forcast city: \"Lisbon\"\n    reply x\n",
        server.url
    );
    let err = run_source(&rt, &source, "go", &[])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("did you mean `weather.forecast`?"), "{}", err);
}

#[tokio::test]
async fn a_tool_reporting_an_error_is_relayed_in_plain_words() {
    let server = fake_mcp::start().await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let source = format!(
        "agent W\n  goal \"w\"\n  tool weather from mcp \"{}\"\n  accepts go\n  on go\n    x = weather.boom\n    reply x\n",
        server.url
    );
    let err = run_source(&rt, &source, "go", &[])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("the weather station is on fire"), "{}", err);
}

#[tokio::test]
async fn server_lost_between_calls_gives_a_clear_message() {
    let server = fake_mcp::start().await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let source = weather_source(&server.url);
    let agent = agent_from(&rt, &source, None);
    assert!(ask(&agent, "ask", &[("city", "Porto")]).await.is_ok());
    server.stop();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let err = ask(&agent, "ask", &[("city", "Porto")])
        .await
        .unwrap_err()
        .render();
    assert!(!err.contains("connection-lost"), "{}", err);
    assert!(err.contains("Problem on line"), "{}", err);
    assert!(err.contains("Fix:"), "{}", err);
}

#[tokio::test]
async fn a_server_that_cannot_start_is_explained() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let source = "agent W\n  goal \"w\"\n  tool weather from mcp \"definitely-not-a-real-program --x\"\n  accepts go\n  on go\n    x = weather.forecast city: \"a\"\n    reply x\n";
    let err = run_source(&rt, source, "go", &[])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("could not start the tool server"), "{}", err);
    assert!(err.contains("installed"), "{}", err);
}
