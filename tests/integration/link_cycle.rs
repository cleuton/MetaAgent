use crate::common::*;
use metagente::runtime::run::{RunOptions, run_file};

#[tokio::test]
async fn agents_calling_each_other_stop_with_the_circle_shown() {
    let dir = tempfile::tempdir().unwrap();
    for f in ["cycle_a.ag", "cycle_b.ag"] {
        std::fs::write(dir.path().join(f), project_file(&format!("examples/{}", f))).unwrap();
    }
    let rt = runtime_in(dir.path(), None, |_| {});
    let opts = RunOptions {
        file: dir.path().join("cycle_a.ag"),
        message: Some("go".into()),
        params: vec![],
        agent: None,
    };
    let err = run_file(&opts, rt).await.unwrap_err().render();
    assert!(
        err.contains(
            "CycleA asked CycleB, and CycleB asked CycleA again (CycleA -> CycleB -> CycleA)"
        ),
        "{}",
        err
    );
    assert!(
        err.contains("make one of them answer without calling the other"),
        "{}",
        err
    );
}

#[tokio::test]
async fn an_agent_linking_to_itself_is_a_cycle() {
    let dir = tempfile::tempdir().unwrap();
    let source =
        "agent Loop\n  goal \"l\"\n  link Loop\n  accepts go\n  on go\n    reply Loop.go\n";
    std::fs::write(dir.path().join("loop.ag"), source).unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let opts = RunOptions {
        file: dir.path().join("loop.ag"),
        message: Some("go".into()),
        params: vec![],
        agent: None,
    };
    let err = run_file(&opts, rt).await.unwrap_err().render();
    assert!(err.contains("Loop -> Loop"), "{}", err);
}

#[tokio::test]
async fn a_missing_target_says_where_it_looked() {
    let dir = tempfile::tempdir().unwrap();
    let source =
        "agent Boss\n  goal \"b\"\n  link Ghost\n  accepts go\n  on go\n    reply Ghost.hi\n";
    std::fs::write(dir.path().join("boss.ag"), source).unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let opts = RunOptions {
        file: dir.path().join("boss.ag"),
        message: Some("go".into()),
        params: vec![],
        agent: None,
    };
    let err = run_file(&opts, rt).await.unwrap_err().render();
    assert!(err.contains("Problem on line 3 of boss.ag"), "{}", err);
    assert!(
        err.contains("I could not find an agent called Ghost"),
        "{}",
        err
    );
    assert!(err.contains("I looked:"), "{}", err);
    assert!(err.contains("Ghost.ag"), "{}", err);
    assert!(
        err.contains("link Ghost from \"path/to/file.ag\""),
        "{}",
        err
    );
}

#[tokio::test]
async fn an_error_inside_the_linked_agent_keeps_its_own_file_and_line() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("inner.ag"), "agent Inner\n  goal \"i\"\n  tool file\n  accepts go\n  on go\n    reply file.read path: \"missing.txt\"\n").unwrap();
    std::fs::write(
        dir.path().join("outer.ag"),
        "agent Outer\n  goal \"o\"\n  link Inner\n  accepts go\n  on go\n    reply Inner.go\n",
    )
    .unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let opts = RunOptions {
        file: dir.path().join("outer.ag"),
        message: Some("go".into()),
        params: vec![],
        agent: None,
    };
    let err = run_file(&opts, rt).await.unwrap_err().render();
    assert!(
        err.contains("Problem on line 6 of outer.ag: Inner could not answer `go`"),
        "{}",
        err
    );
    assert!(err.contains("Problem on line 6 of inner.ag"), "{}", err);
    assert!(
        err.contains("the file `missing.txt` does not exist"),
        "{}",
        err
    );
}
