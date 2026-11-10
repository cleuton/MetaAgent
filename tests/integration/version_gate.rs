// 0.1.4: [version] minimum (US6; FR-047).
use crate::common::auth::*;

const HELLO: &str = "agent Hello\n  goal \"x\"\n  accepts hi\n  on hi\n    reply \"hello\"\n";

fn folder(toml: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "hello.ag", HELLO);
    write(dir.path(), "metagente.toml", toml);
    dir
}

fn all_three(dir: &std::path::Path) -> Vec<(bool, String, String)> {
    vec![
        run(dir, &["check", "hello.ag"]),
        run(dir, &["run", "hello.ag", "hi"]),
        run(dir, &["serve", "hello.ag", "--a2a", "0"]),
    ]
}

#[test]
fn the_same_or_a_newer_program_works() {
    for minimum in ["0.1.4", "0.1.0", "0.0.1"] {
        let dir = folder(&format!("[version]\nminimum = \"{}\"\n", minimum));
        let (ok, out, err) = run(dir.path(), &["run", "hello.ag", "hi"]);
        assert!(ok, "{}{}", out, err);
        assert_eq!(out.trim(), "hello");
        assert!(run(dir.path(), &["check", "hello.ag"]).0);
    }
}

#[test]
fn a_higher_minimum_stops_run_check_and_serve_with_the_same_message() {
    let dir = folder("[version]\nminimum = \"99.0.0\"\n");
    for (ok, out, err) in all_three(dir.path()) {
        assert!(!ok);
        assert!(!out.contains("hello"));
        // 0.1.5: the version of the program is not a constant of this test (F14: the tests of the version text change in every release)
        assert!(
            err.contains(&format!(
                "This project needs Metagente 99.0.0 or newer, and this is {}. Nothing was run.",
                env!("CARGO_PKG_VERSION")
            )),
            "{}",
            err
        );
        assert!(err.contains("https://github.com/cleuton/MetaAgent/releases/latest"));
    }
}

#[test]
fn the_version_message_comes_before_any_list_of_unknown_settings() {
    let dir = folder(
        "[version]\nminimum = \"99.0.0\"\n[future_section]\nx = 1\n[auth]\nnew_key_from_the_future = true\n[llm]\nsome_new_thing = 1\n",
    );
    for (ok, _, err) in all_three(dir.path()) {
        assert!(!ok);
        assert!(err.contains("needs Metagente 99.0.0 or newer"), "{}", err);
        assert!(
            !err.contains("future_section") && !err.contains("new_key_from_the_future"),
            "{}",
            err
        );
    }
}

#[test]
fn every_wrong_shape_stops_with_the_right_shape_shown() {
    for bad in [
        ">=0.1.4",
        "0.1",
        "0.1.4-beta",
        "00.1.4",
        "0.1.4.1",
        "latest",
        "",
    ] {
        let dir = folder(&format!("[version]\nminimum = \"{}\"\n", bad));
        for (ok, _, err) in all_three(dir.path()) {
            assert!(!ok, "{:?}", bad);
            assert!(
                err.contains(&format!(
                    "[version] minimum must be three numbers like 0.1.4, but it says \"{}\".",
                    bad
                )),
                "{:?}: {}",
                bad,
                err
            );
            assert!(err.contains("write minimum = \"0.1.4\""));
        }
    }
    let dir = folder("[version]\nminimum = 4\n");
    assert!(!run(dir.path(), &["check", "hello.ag"]).0);
}

#[test]
fn another_key_in_version_is_a_warning_and_no_section_means_no_check() {
    let dir = folder("[version]\nminimum = \"0.1.4\"\nmaximum = \"1.0.0\"\n");
    let (ok, _, err) = run(dir.path(), &["run", "hello.ag", "hi"]);
    assert!(ok);
    assert!(
        err.contains("unknown setting `maximum` in [version]"),
        "{}",
        err
    );
    let dir = folder("[runtime]\ntimeout_seconds = 5\n");
    assert!(run(dir.path(), &["run", "hello.ag", "hi"]).0);
}

#[test]
fn auth_without_version_warns_on_check_run_and_serve() {
    let dir = project("[auth]\nvalidator = \"./validate.sh\"\nprotect = [\"Planner.plan\"]\n");
    let want = "Warning: [auth] is set but [version] is not. Metagente 0.1.3 and older ignore [auth] and would serve every message without protection.";
    let (ok, _, err) = run(dir.path(), &["check", "planner.ag"]);
    assert!(ok);
    assert!(err.contains(want), "{}", err);
    assert!(err.contains("minimum = \"0.1.4\""));
    let (_, _, err) = run(dir.path(), &["run", "planner.ag", "ping"]);
    assert!(err.contains(want), "{}", err);
    let port = free_port();
    let server = serve(
        dir.path(),
        "a2a",
        &["planner.ag", "--a2a", &port.to_string()],
        port,
    );
    assert!(server.log().contains(want), "{}", server.log());
    // with [version] the warning is gone
    let dir = project(BASIC_TOML);
    let (_, _, err) = run(dir.path(), &["check", "planner.ag"]);
    assert!(!err.contains("[version] is not"), "{}", err);
}

#[test]
fn a_linked_agent_is_checked_only_by_the_file_of_the_agent_that_loaded_it() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "metagente.toml",
        "[version]\nminimum = \"0.1.4\"\n",
    );
    write(
        dir.path(),
        "main.ag",
        "agent Main\n  goal \"x\"\n  link Helper from \"sub/helper.ag\"\n  accepts go\n  on go\n    reply Helper.hi\n",
    );
    write(
        dir.path(),
        "sub/helper.ag",
        "agent Helper\n  goal \"x\"\n  accepts hi\n  on hi\n    reply \"from helper\"\n",
    );
    write(
        dir.path(),
        "sub/metagente.toml",
        "[version]\nminimum = \"99.0.0\"\n",
    );
    let (ok, out, err) = run(dir.path(), &["run", "main.ag", "go"]);
    assert!(ok, "{}{}", out, err);
    assert_eq!(out.trim(), "from helper");
}

#[tokio::test(flavor = "multi_thread")]
async fn raising_the_minimum_while_serving_changes_nothing_and_says_a_restart_is_needed() {
    let dir = folder("[version]\nminimum = \"0.1.4\"\n");
    let port = free_port();
    let server = serve(
        dir.path(),
        "a2a",
        &["hello.ag", "--a2a", &port.to_string()],
        port,
    );
    write(
        dir.path(),
        "metagente.toml",
        "[version]\nminimum = \"99.0.0\"\n",
    );
    let r = send(port, "/a2a", None, "hi", "").await;
    assert_eq!(r.status, 200);
    assert_eq!(answer(&r), "hello");
    let r = send(port, "/a2a", None, "hi", "").await;
    assert_eq!(answer(&r), "hello");
    let log = server.log();
    assert_eq!(
        log.matches("Note: metagente.toml changed [auth] (or [version]).")
            .count(),
        1,
        "{}",
        log
    );
}

#[test]
fn metagente_new_writes_version_and_a_commented_auth_example() {
    let dir = tempfile::tempdir().unwrap();
    let (ok, _, err) = run(dir.path(), &["new", "hello"]);
    assert!(ok, "{}", err);
    let toml = std::fs::read_to_string(dir.path().join("metagente.toml")).unwrap();
    // 0.1.5: `metagente new` writes the version of the program that runs it (F14, F16)
    assert!(
        toml.contains(&format!(
            "[version]\nminimum = \"{}\"",
            env!("CARGO_PKG_VERSION")
        )),
        "{}",
        toml
    );
    assert!(toml.contains("# [auth]"));
    assert!(
        !toml.lines().any(|l| l.trim() == "[auth]"),
        "the example must stay commented"
    );
    let (ok, out, err) = run(dir.path(), &["check", "hello.ag"]);
    assert!(ok, "{}{}", out, err);
}
