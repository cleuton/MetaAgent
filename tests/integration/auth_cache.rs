// 0.1.4: the short memory of accepted tokens (US3; FR-043). The validator counts its own runs.
use crate::common::auth::*;
use std::time::Duration;

fn toml_with(cache: &str) -> String {
    format!("{}{}", BASIC_TOML, cache)
}

#[tokio::test(flavor = "multi_thread")]
async fn ten_requests_with_the_same_accepted_token_run_the_validator_once() {
    let dir = project(&toml_with("cache_seconds = 30\n"));
    let port = free_port();
    let _server = serve(
        dir.path(),
        "a2a",
        &["planner.ag", "--a2a", &port.to_string()],
        port,
    );
    for _ in 0..10 {
        assert_eq!(
            send(port, "/a2a", Some("good-token"), "plan", "x")
                .await
                .status,
            200
        );
    }
    assert_eq!(runs(dir.path()), 1);
    // the cache is shared by every protected message: another token still asks
    assert_eq!(
        send(port, "/a2a", Some("other-token"), "plan", "x")
            .await
            .status,
        200
    );
    assert_eq!(runs(dir.path()), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_entry_ends_after_cache_seconds_and_use_does_not_extend_it() {
    let dir = project(&toml_with("cache_seconds = 2\n"));
    let port = free_port();
    let _server = serve(
        dir.path(),
        "a2a",
        &["planner.ag", "--a2a", &port.to_string()],
        port,
    );
    assert_eq!(
        send(port, "/a2a", Some("good-token"), "plan", "x")
            .await
            .status,
        200
    );
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert_eq!(
        send(port, "/a2a", Some("good-token"), "plan", "x")
            .await
            .status,
        200
    );
    assert_eq!(runs(dir.path()), 1, "still inside the 2 seconds");
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert_eq!(
        send(port, "/a2a", Some("good-token"), "plan", "x")
            .await
            .status,
        200
    );
    assert_eq!(
        runs(dir.path()),
        2,
        "2.4 seconds after the acceptance, even though it was used at 1.2"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refusal_is_never_remembered() {
    let dir = project(&toml_with("cache_seconds = 30\n"));
    let port = free_port();
    let _server = serve(
        dir.path(),
        "a2a",
        &["planner.ag", "--a2a", &port.to_string()],
        port,
    );
    for _ in 0..2 {
        assert_eq!(
            send(port, "/a2a", Some("wrong"), "plan", "x").await.status,
            401
        );
    }
    assert_eq!(runs(dir.path()), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn without_cache_seconds_nothing_is_remembered() {
    for extra in ["", "cache_seconds = 0\n"] {
        let dir = project(&toml_with(extra));
        let port = free_port();
        let _server = serve(
            dir.path(),
            "a2a",
            &["planner.ag", "--a2a", &port.to_string()],
            port,
        );
        for _ in 0..2 {
            assert_eq!(
                send(port, "/a2a", Some("good-token"), "plan", "x")
                    .await
                    .status,
                200
            );
        }
        assert_eq!(runs(dir.path()), 2);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_validator_that_could_not_answer_is_not_remembered() {
    let dir = project(
        &toml_with("cache_seconds = 30\ntimeout_seconds = 1\n")
            .replace("./validate.sh", "./once.sh"),
    );
    // slow the first time only
    executable(
        dir.path(),
        "once.sh",
        "#!/bin/sh\nif [ ! -f seen ]; then touch seen; sleep 20; fi\nexit 0\n",
    );
    let port = free_port();
    let _server = serve(
        dir.path(),
        "a2a",
        &["planner.ag", "--a2a", &port.to_string()],
        port,
    );
    assert_eq!(
        send(port, "/a2a", Some("good-token"), "plan", "x")
            .await
            .status,
        503
    );
    assert_eq!(
        send(port, "/a2a", Some("good-token"), "plan", "x")
            .await
            .status,
        200
    );
}

#[test]
fn a_cache_that_is_too_long_stops_every_command() {
    let dir = project(&toml_with("cache_seconds = 301\n"));
    for args in [
        vec!["check", "planner.ag"],
        vec!["run", "planner.ag", "ping"],
    ] {
        let (ok, _, err) = run(dir.path(), &args);
        assert!(!ok);
        assert!(
            err.contains("`cache_seconds` in [auth] must be a whole number from 0 to 300"),
            "{}",
            err
        );
    }
}
