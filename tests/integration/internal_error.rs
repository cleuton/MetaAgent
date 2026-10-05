use std::process::Command;

fn metagente() -> Command {
    Command::new(env!("CARGO_BIN_EXE_metagente"))
}

#[test]
fn an_internal_failure_prints_one_plain_sentence_and_a_log_path() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("a.ag"),
        "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply \"x\"\n",
    )
    .unwrap();
    let out = metagente()
        .current_dir(dir.path())
        .env("METAGENTE_INTERNAL_TEST_PANIC", "1")
        .args(["run", "a.ag", "go"])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(
        err.contains("Something went wrong inside Metagente"),
        "{}",
        err
    );
    assert!(err.contains("Details were saved in"), "{}", err);
    for word in [
        "panicked",
        "RUST_BACKTRACE",
        "stack backtrace",
        "src/",
        "thread '",
    ] {
        assert!(!err.contains(word), "found `{}` in:\n{}", word, err);
    }
    // The technical details went to the log file, not to the screen.
    let log = std::fs::read_to_string(metagente::diagnostics::internal::log_path()).unwrap();
    assert!(log.contains("deliberate internal failure"), "{}", log);
}

#[test]
fn cli_new_check_and_run_work_together() {
    let dir = tempfile::tempdir().unwrap();
    let ok = metagente()
        .current_dir(dir.path())
        .args(["new", "hello"])
        .output()
        .unwrap();
    assert!(ok.status.success());
    assert!(dir.path().join("hello.ag").exists());
    assert!(dir.path().join("metagente.toml").exists());
    let again = metagente()
        .current_dir(dir.path())
        .args(["new", "hello"])
        .output()
        .unwrap();
    assert!(!again.status.success());
    assert!(String::from_utf8_lossy(&again.stderr).contains("already exists"));
    let check = metagente()
        .current_dir(dir.path())
        .args(["check", "hello.ag"])
        .output()
        .unwrap();
    assert!(check.status.success());
    let run = metagente()
        .current_dir(dir.path())
        .args(["run", "hello.ag", "greet", "name=Ana"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "Hello, Ana!");
    // With one accepted message the message can be left out.
    let short = metagente()
        .current_dir(dir.path())
        .args(["run", "hello.ag", "name=Ana"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&short.stdout).trim(), "Hello, Ana!");
}

#[test]
fn run_stops_on_problems_and_says_how_many_more() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("bad.ag"),
        "agent Bad\n  accepts go\n  on go\n    x = nme\n    reply q\n",
    )
    .unwrap();
    let out = metagente()
        .current_dir(dir.path())
        .args(["run", "bad.ag", "go"])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1));
    assert!(err.contains("more problem"), "{}", err);
    assert!(err.contains("metagente check bad.ag"), "{}", err);
}
