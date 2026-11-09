// 0.1.4: a project with no [version] and no [auth] behaves as in 0.1.3 (FR-051, SC-006).
use crate::common::auth::*;

#[tokio::test(flavor = "multi_thread")]
async fn a_project_with_no_new_section_serves_everything_in_the_open_as_before() {
    for toml in [
        "",
        "[runtime]\ntimeout_seconds = 5\n",
        "[version]\nminimum = \"0.1.4\"\n",
    ] {
        let dir = project(toml);
        let a2a_port = free_port();
        let mcp_port = free_port();
        let a2a = serve(
            dir.path(),
            "a2a",
            &["planner.ag", "--a2a", &a2a_port.to_string()],
            a2a_port,
        );
        let _mcp = serve(
            dir.path(),
            "mcp",
            &["planner.ag", "--mcp", &mcp_port.to_string()],
            mcp_port,
        );
        // no token, a token, a malformed header: all the same, because nothing is protected
        for token in [None, Some("anything")] {
            let r = send(a2a_port, "/a2a", token, "plan", "x").await;
            assert_eq!(
                (r.status, answer(&r).as_str()),
                (200, "planned x"),
                "{}",
                toml
            );
            assert!(r.www_authenticate.is_none());
        }
        let bad = post_raw(
            a2a_port,
            "/a2a",
            &["Basic zzz", "Bearer a b"],
            plan_body("y"),
        )
        .await;
        assert_eq!(answer(&bad), "planned y");
        let m = mcp_call(mcp_port, None, "Planner.plan", "z").await;
        assert_eq!(m.status, 200, "{}", m.text);
        assert!(m.text.contains("planned z"));
        assert_eq!(runs(dir.path()), 0, "no validator exists");
        let log = a2a.log();
        assert!(
            !log.contains("Refused") && !log.contains("protected") && !log.contains("[auth]"),
            "{}",
            log
        );
        let card: serde_json::Value = reqwest::get(format!(
            "http://127.0.0.1:{}/.well-known/agent-card.json",
            a2a_port
        ))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
        assert!(card.get("securitySchemes").is_none());
    }
}

#[test]
fn run_and_check_say_nothing_new_without_the_new_sections() {
    let dir = project("");
    let (ok, out, err) = run(dir.path(), &["check", "planner.ag"]);
    assert!(ok);
    assert_eq!(out.trim(), "No problems found in planner.ag (1 agent).");
    assert_eq!(err, "");
    let (ok, out, err) = run(dir.path(), &["run", "planner.ag", "ping"]);
    assert!(ok);
    assert_eq!((out.trim(), err.as_str()), ("pong", ""));
}
