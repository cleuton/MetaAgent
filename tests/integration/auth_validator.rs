// 0.1.4: protecting a message and the validator contract (US1, US2; FR-041, FR-042).
use crate::common::auth::*;

#[tokio::test(flavor = "multi_thread")]
async fn no_token_is_401_and_neither_the_validator_nor_the_agent_ran() {
    let dir = project(BASIC_TOML);
    let port = free_port();
    let server = serve(
        dir.path(),
        "a2a",
        &["planner.ag", "--a2a", &port.to_string()],
        port,
    );
    let r = send(port, "/a2a", None, "plan", "x").await;
    assert_eq!(r.status, 401);
    assert_eq!(r.www_authenticate.as_deref(), Some("Bearer"));
    assert!(
        answer(&r).contains("Planner.plan needs a token"),
        "{}",
        r.text
    );
    assert_eq!(
        runs(dir.path()),
        0,
        "the validator must not run without a token"
    );
    assert!(!agent_ran(dir.path()));
    assert!(server.log().contains("Refused Planner.plan: no token."));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_good_token_runs_the_handler_and_a_refused_one_does_not() {
    let dir = project(BASIC_TOML);
    let port = free_port();
    let server = serve(
        dir.path(),
        "a2a",
        &["planner.ag", "--a2a", &port.to_string()],
        port,
    );
    let bad = send(port, "/a2a", Some("wrong-token"), "plan", "x").await;
    assert_eq!(bad.status, 401);
    assert!(answer(&bad).contains("The token was not accepted for Planner.plan"));
    assert!(!agent_ran(dir.path()));
    let log = server.log();
    assert!(
        log.contains("token not accepted (the validator exited with code 1)"),
        "{}",
        log
    );
    assert!(
        !log.contains("wrong-token"),
        "the token must never be logged"
    );
    let good = send(port, "/a2a", Some("good-token"), "plan", "rust").await;
    assert_eq!(good.status, 200, "{}", good.text);
    assert_eq!(answer(&good), "planned rust");
    assert!(agent_ran(dir.path()));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_validator_that_is_missing_or_too_slow_is_503_and_nothing_runs() {
    for (validator, timeout, expect) in [
        ("./missing.sh", 5, "was not found"),
        ("./sleepy.sh", 1, "did not answer in 1 second"),
    ] {
        let toml = format!(
            "[version]\nminimum = \"0.1.4\"\n[auth]\nvalidator = \"{}\"\nprotect = [\"Planner.plan\"]\ntimeout_seconds = {}\n",
            validator, timeout
        );
        let dir = project(&toml);
        executable(dir.path(), "sleepy.sh", "#!/bin/sh\nsleep 20\n");
        let port = free_port();
        let _server = serve(
            dir.path(),
            "a2a",
            &["planner.ag", "--a2a", &port.to_string()],
            port,
        );
        let r = send(port, "/a2a", Some("good-token"), "plan", "x").await;
        assert_eq!(r.status, 503, "{}", r.text);
        assert!(r.www_authenticate.is_none());
        let message = answer(&r);
        assert!(
            message.contains("I could not ask the validator"),
            "{}",
            message
        );
        assert!(message.contains(expect), "{}", message);
        assert!(!agent_ran(dir.path()));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn other_messages_and_other_agents_are_served_as_before() {
    let dir = project(BASIC_TOML);
    let port = free_port();
    let _server = serve(
        dir.path(),
        "a2a",
        &["planner.ag", "--a2a", &port.to_string()],
        port,
    );
    let r = send(port, "/a2a", None, "ping", "").await;
    assert_eq!(r.status, 200);
    assert_eq!(answer(&r), "pong");
    assert_eq!(runs(dir.path()), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn star_protects_every_message_of_the_agent() {
    let dir = project(&BASIC_TOML.replace("Planner.plan", "Planner.*"));
    let port = free_port();
    let _server = serve(
        dir.path(),
        "a2a",
        &["planner.ag", "--a2a", &port.to_string()],
        port,
    );
    assert_eq!(send(port, "/a2a", None, "ping", "").await.status, 401);
    assert_eq!(
        send(port, "/a2a", Some("good-token"), "ping", "")
            .await
            .status,
        200
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_message_is_not_supported_before_any_token_is_read() {
    let dir = project(BASIC_TOML);
    let port = free_port();
    let _server = serve(
        dir.path(),
        "a2a",
        &["planner.ag", "--a2a", &port.to_string()],
        port,
    );
    let r = send(port, "/a2a", None, "nothing", "").await;
    assert_eq!(r.status, 200);
    assert!(answer(&r).starts_with("Not supported"), "{}", r.text);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_agent_card_is_public_and_says_a_bearer_token_is_needed() {
    let dir = project(BASIC_TOML);
    let port = free_port();
    let _server = serve(
        dir.path(),
        "a2a",
        &["planner.ag", "--a2a", &port.to_string()],
        port,
    );
    let card: serde_json::Value = reqwest::get(format!(
        "http://127.0.0.1:{}/.well-known/agent-card.json",
        port
    ))
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(
        card.pointer("/securitySchemes/bearer/httpAuthSecurityScheme/scheme")
            .unwrap(),
        "Bearer"
    );
    let skills = card["skills"].as_array().unwrap();
    let plan = skills.iter().find(|s| s["id"] == "plan").unwrap();
    let ping = skills.iter().find(|s| s["id"] == "ping").unwrap();
    assert!(plan.get("securityRequirements").is_some());
    assert!(ping.get("securityRequirements").is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn over_mcp_the_same_message_needs_the_same_token() {
    let dir = project(BASIC_TOML);
    let port = free_port();
    let _server = serve(
        dir.path(),
        "mcp",
        &["planner.ag", "--mcp", &port.to_string()],
        port,
    );
    let none = mcp_call(port, None, "Planner.plan", "x").await;
    assert_eq!(none.status, 401, "{}", none.text);
    assert_eq!(none.www_authenticate.as_deref(), Some("Bearer"));
    assert!(!agent_ran(dir.path()));
    let bad = mcp_call(port, Some("wrong"), "Planner.plan", "x").await;
    assert_eq!(bad.status, 401);
    let good = mcp_call(port, Some("good-token"), "Planner.plan", "rust").await;
    assert_eq!(good.status, 200, "{}", good.text);
    assert!(good.text.contains("planned rust"), "{}", good.text);
    assert!(agent_ran(dir.path()));
    // a public message and an unknown tool need no token
    let ping = mcp_call(port, None, "Planner.ping", "").await;
    assert_eq!(ping.status, 200);
    assert!(ping.text.contains("pong"));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_validator_gets_the_token_on_stdin_and_nothing_else() {
    let dir = project(BASIC_TOML);
    executable(
        dir.path(),
        "validate.sh",
        "#!/bin/sh\ncat > stdin.txt\nenv > env.txt\necho \"$# $*\" > args.txt\npwd > cwd.txt\nexit 0\n",
    );
    let port = free_port();
    let _server = serve_env(
        dir.path(),
        "a2a",
        &["planner.ag", "--a2a", &port.to_string()],
        port,
        &[
            ("ANTHROPIC_API_KEY", "sk-model-key-123"),
            ("SOME_OTHER", "visible"),
        ],
    );
    let r = send(port, "/a2a", Some("tok-123.abc_DEF~+/="), "plan", "x").await;
    assert_eq!(r.status, 200, "{}", r.text);
    let stdin = std::fs::read_to_string(dir.path().join("stdin.txt")).unwrap();
    assert_eq!(
        stdin, "tok-123.abc_DEF~+/=\n",
        "the token and one line break, then the input is closed"
    );
    let args = std::fs::read_to_string(dir.path().join("args.txt")).unwrap();
    assert_eq!(args.trim(), "0", "no argument at all");
    let env = std::fs::read_to_string(dir.path().join("env.txt")).unwrap();
    for line in env.lines() {
        let name = line.split('=').next().unwrap();
        assert!(
            ["PATH", "PWD", "SHLVL", "_", "OLDPWD"].contains(&name),
            "unexpected variable in the validator: {}",
            name
        );
    }
    assert!(!env.contains("sk-model-key-123") && !env.contains("SOME_OTHER"));
    assert!(
        !env.contains("tok-123"),
        "the token is not in the environment"
    );
    let cwd = std::fs::read_to_string(dir.path().join("cwd.txt")).unwrap();
    assert_eq!(
        std::fs::canonicalize(cwd.trim()).unwrap(),
        std::fs::canonicalize(dir.path()).unwrap(),
        "the validator runs in the folder of metagente.toml"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_token_that_could_be_read_as_a_command_is_refused_before_any_program_starts() {
    let dir = project(BASIC_TOML);
    let port = free_port();
    let _server = serve(
        dir.path(),
        "a2a",
        &["planner.ag", "--a2a", &port.to_string()],
        port,
    );
    let long = format!("Bearer {}", "a".repeat(16385));
    let cases: Vec<Vec<&str>> = vec![
        vec!["Bearer a b"],
        vec!["Bearer a;rm -rf x"],
        vec!["Bearer $(touch pwned)"],
        vec!["Bearer a\"b"],
        vec!["Bearer 'a'"],
        vec!["Bearer a=b"],
        vec!["Bearer "],
        vec!["Bearer"],
        vec!["Basic dXNlcjpwYXNz"],
        vec!["Bearer good-token", "Bearer good-token"],
        vec![&long],
    ];
    for headers in cases {
        let r = post_raw(port, "/a2a", &headers, plan_body("x")).await;
        assert_eq!(r.status, 401, "{:?}", headers);
        assert!(
            answer(&r).contains("not a bearer token I can read"),
            "{:?}: {}",
            headers,
            answer(&r)
        );
    }
    assert_eq!(
        runs(dir.path()),
        0,
        "no validator may start for a malformed header"
    );
    assert!(!dir.path().join("pwned").exists());
    // the word Bearer in any letter case, and the largest token, are fine
    let ok = post_raw(port, "/a2a", &["bEaReR good-token"], plan_body("x")).await;
    assert_eq!(ok.status, 200, "{}", ok.text);
}

// 0.1.4: a validator that misbehaves never hangs a request and never lets one through by accident (FR-041, edge cases)
#[tokio::test(flavor = "multi_thread")]
async fn a_validator_that_crashes_prints_a_lot_ignores_its_input_or_leaves_a_child_behind() {
    let cases = [
        // killed by a signal: no exit code, so not accepted
        ("crash", "#!/bin/sh\nkill -9 $$\n", 401),
        // an error code is a refusal
        ("exit2", "#!/bin/sh\nexit 2\n", 401),
        // output is thrown away and never shown
        ("loud", "#!/bin/sh\nyes secret-output | head -c 3000000\nexit 0\n", 200),
        // never reads its input
        ("deaf", "#!/bin/sh\nexit 0\n", 200),
        // starts a child that keeps running: not waited for
        ("child", "#!/bin/sh\nsleep 30 &\nexit 0\n", 200),
    ];
    for (name, script, expect) in cases {
        let dir = project(&BASIC_TOML.replace("./validate.sh", &format!("./{}.sh", name)));
        executable(dir.path(), &format!("{}.sh", name), script);
        let port = free_port();
        let server = serve(dir.path(), "a2a", &["planner.ag", "--a2a", &port.to_string()], port);
        let started = std::time::Instant::now();
        let r = send(port, "/a2a", Some("good-token"), "plan", "x").await;
        assert_eq!(r.status, expect, "{}: {}", name, r.text);
        assert!(started.elapsed().as_secs() < 4, "{} took {:?}", name, started.elapsed());
        assert!(!r.text.contains("secret-output") && !server.log().contains("secret-output"));
        assert_eq!(agent_ran(dir.path()), expect == 200, "{}", name);
    }
}
