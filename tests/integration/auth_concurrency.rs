// 0.1.4: many protected requests at once (FR-050, SC-011), and at most 8 validators together (D6).
use crate::common::auth::*;
use futures::future::join_all;
use std::time::{Duration, Instant};

#[tokio::test(flavor = "multi_thread")]
async fn fifty_protected_requests_at_once_finish_quickly_and_at_most_eight_validators_run_together()
{
    let dir = project(BASIC_TOML);
    // 0.1 seconds per run; each run records how many were running when it started, using a directory as a lock
    executable(
        dir.path(),
        "validate.sh",
        "#!/bin/sh\nread -r token\nmkdir running.$$\nn=$(ls -d running.* | wc -l)\necho $n >> concurrent.txt\nsleep 0.1\nrmdir running.$$\ngrep -qxF \"$token\" valid-tokens.txt\n",
    );
    let port = free_port();
    let _server = serve(
        dir.path(),
        "a2a",
        &["planner.ag", "--a2a", &port.to_string()],
        port,
    );
    let started = Instant::now();
    let results =
        join_all((0..50).map(|_| send(port, "/a2a", Some("good-token"), "plan", "x"))).await;
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "took {:?}",
        started.elapsed()
    );
    assert!(results.iter().all(|r| r.status == 200));
    let peak = std::fs::read_to_string(dir.path().join("concurrent.txt"))
        .unwrap()
        .lines()
        .filter_map(|l| l.trim().parse::<usize>().ok())
        .max()
        .unwrap();
    assert!(peak <= 8, "{} validators ran together", peak);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_request_that_waits_too_long_for_a_place_gets_503_never_a_free_pass() {
    let toml = format!("{}timeout_seconds = 1\n", BASIC_TOML);
    let dir = project(&toml);
    executable(
        dir.path(),
        "validate.sh",
        "#!/bin/sh\nread -r token\nsleep 3\nexit 0\n",
    );
    let port = free_port();
    let _server = serve(
        dir.path(),
        "a2a",
        &["planner.ag", "--a2a", &port.to_string()],
        port,
    );
    let results =
        join_all((0..20).map(|_| send(port, "/a2a", Some("good-token"), "plan", "x"))).await;
    assert!(
        results.iter().all(|r| r.status == 503),
        "{:?}",
        results.iter().map(|r| r.status).collect::<Vec<_>>()
    );
    assert!(!agent_ran(dir.path()));
}
