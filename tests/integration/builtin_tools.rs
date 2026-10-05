use crate::common::*;
use metagente::runtime::value::Value;

#[tokio::test]
async fn read_file_example_reads_a_temp_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("notes.txt"), "hello from notes").unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let reply = run_source(
        &rt,
        &project_file("examples/read_file.ag"),
        "summarize",
        &[],
    )
    .await
    .unwrap();
    assert_eq!(reply, Value::text("hello from notes"));
}

#[tokio::test]
async fn missing_file_gives_a_plain_message_with_the_line() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let err = run_source(
        &rt,
        &project_file("examples/read_file.ag"),
        "summarize",
        &[],
    )
    .await
    .unwrap_err();
    let text = err.render();
    assert!(
        text.contains("the file `notes.txt` does not exist"),
        "{}",
        text
    );
    assert!(text.contains("line 6"), "{}", text);
}

#[tokio::test]
async fn file_write_then_read_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let source = r#"agent Notes
  goal "Keep notes"
  tool file
  accepts save text
  on save
    file.write path: "out/a.txt" text: text
    back = file.read path: "out/a.txt"
    reply back
"#;
    let reply = run_source(&rt, source, "save", &[("text", "remember me")])
        .await
        .unwrap();
    assert_eq!(reply, Value::text("remember me"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("out/a.txt")).unwrap(),
        "remember me"
    );
}

#[tokio::test]
async fn clock_example_reports_the_time_and_waits() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let started = std::time::Instant::now();
    let reply = run_source(&rt, &project_file("examples/clock.ag"), "now", &[])
        .await
        .unwrap();
    assert!(started.elapsed().as_secs_f64() >= 1.0);
    let text = reply.to_display();
    assert!(text.starts_with("It was 20"), "{}", text);
    assert!(text.ends_with("a second ago"), "{}", text);
}

#[tokio::test]
async fn clock_now_has_text_and_unix() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let source = "agent T\n  goal \"t\"\n  tool clock\n  accepts go\n  on go\n    t = clock.now\n    reply t.unix\n";
    let reply = run_source(&rt, source, "go", &[]).await.unwrap();
    assert!(reply.as_number().unwrap() > 1_700_000_000.0);
}

#[tokio::test]
async fn control_flow_and_lists_work() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let source = r#"agent Flow
  goal "Try the language"
  tool state
  accepts go n
  on go
    total = 0
    for item in [1, 2, 3]
      state.set key: "last" value: item
    last = state.get key: "last"
    if n is more than 5 and last is 3
      reply "big {last}"
    otherwise
      reply "small {last}"
"#;
    let big = run_source(&rt, source, "go", &[("n", "9")]).await.unwrap();
    assert_eq!(big, Value::text("big 3"));
    let small = run_source(&rt, source, "go", &[("n", "2")]).await.unwrap();
    assert_eq!(small, Value::text("small 3"));
}
