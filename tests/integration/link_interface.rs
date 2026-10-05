use crate::common::*;
use metagente::runtime::run::{RunOptions, run_file};

const TARGET: &str = r#"agent Writer
  goal "Write a marker file"
  tool file
  accepts note text
  on note
    file.write path: "marker.txt" text: text
    reply "written"
"#;

fn caller(call: &str) -> String {
    format!(
        "agent Boss\n  goal \"b\"\n  link Writer\n  accepts go\n  on go\n    r = {}\n    reply r\n",
        call
    )
}

async fn run_boss(dir: &std::path::Path, call: &str) -> String {
    std::fs::write(dir.join("writer.ag"), TARGET).unwrap();
    std::fs::write(dir.join("boss.ag"), caller(call)).unwrap();
    let rt = runtime_in(dir, None, |_| {});
    let opts = RunOptions {
        file: dir.join("boss.ag"),
        message: Some("go".into()),
        params: vec![],
        agent: None,
    };
    run_file(&opts, rt).await.unwrap_err().render()
}

#[tokio::test]
async fn a_call_the_target_does_not_accept_is_rejected_before_it_runs() {
    for (call, expected) in [
        ("Writer.nots text: \"x\"", "did you mean `note`?"),
        ("Writer.note", "needs a value for `text`"),
        (
            "Writer.note text: \"x\" extra: \"y\"",
            "does not take `extra`",
        ),
        ("Writer.note txt: \"x\"", "needs a value for `text`"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let err = run_boss(dir.path(), call).await;
        assert!(err.contains(expected), "{} -> {}", call, err);
        assert!(err.contains("Problem on line 6"), "{}", err);
        assert!(
            !dir.path().join("marker.txt").exists(),
            "the target must not have run for {}",
            call
        );
    }
}

#[tokio::test]
async fn the_same_mismatch_is_caught_at_run_time_when_the_file_is_changed_after_checking() {
    // Ask through the runtime directly (no static check) to prove the runtime check exists too.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("writer.ag"), TARGET).unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let source = caller("Writer.nots text: \"x\"");
    let err = run_source(&rt, &source, "go", &[])
        .await
        .unwrap_err()
        .render();
    assert!(
        err.contains("does not accept the message `nots`"),
        "{}",
        err
    );
    assert!(!dir.path().join("marker.txt").exists());
}
