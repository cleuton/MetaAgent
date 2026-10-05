use crate::common::fake_mcp;
use crate::common::*;

#[tokio::test]
async fn a_slow_tool_times_out_with_a_plain_message_and_the_agent_keeps_working() {
    let server = fake_mcp::start().await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let source = format!(
        r#"agent Slow
  goal "Be slow"
  tool srv from mcp "{}"
  accepts wait
  accepts quick
  on wait
    r = srv.sleepy seconds: 5 within 1 seconds
    reply r
  on quick
    f = srv.forecast city: "Faro"
    reply f.summary
"#,
        server.url
    );
    let agent = agent_from(&rt, &source, None);
    let started = std::time::Instant::now();
    let err = ask(&agent, "wait", &[]).await.unwrap_err().render();
    assert!(
        started.elapsed().as_secs_f64() < 4.0,
        "took {:?}",
        started.elapsed()
    );
    assert!(err.contains("did not finish within 1 seconds"), "{}", err);
    assert!(err.contains("within 60 seconds"), "{}", err);
    let ok = ask(&agent, "quick", &[]).await.unwrap();
    assert_eq!(ok.to_display(), "sunny, 24 degrees");
}

#[tokio::test]
async fn the_default_timeout_comes_from_configuration() {
    let server = fake_mcp::start().await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |c| c.timeout_seconds = 1);
    let source = format!(
        "agent S\n  goal \"s\"\n  tool srv from mcp \"{}\"\n  accepts go\n  on go\n    r = srv.sleepy seconds: 5\n    reply r\n",
        server.url
    );
    let err = run_source(&rt, &source, "go", &[])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("did not finish within 1 seconds"), "{}", err);
}
