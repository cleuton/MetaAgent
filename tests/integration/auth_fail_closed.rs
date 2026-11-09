// 0.1.4: every mistake refuses, nothing is served open (US7; FR-046).
use crate::common::auth::*;
use crate::common::self_signed::pair_in;

fn all_commands_stop(dir: &std::path::Path, expect: &[&str]) {
    for args in [
        vec!["check", "planner.ag"],
        vec!["run", "planner.ag", "ping"],
        vec!["serve", "planner.ag", "--a2a", "0"],
    ] {
        let (ok, _, err) = run(dir, &args);
        assert!(!ok, "{:?} should stop", args);
        for want in expect {
            assert!(err.contains(want), "{:?}: {}", args, err);
        }
    }
}

#[test]
fn a_misspelled_key_stops_everything_and_suggests_the_right_one() {
    let dir = project(&BASIC_TOML.replace("protect", "protec"));
    all_commands_stop(
        dir.path(),
        &["[auth] has the setting protec", "did you mean `protect`?"],
    );
}

#[test]
fn a_name_that_matches_nothing_stops_everything_and_suggests_the_closest() {
    let dir = project(&BASIC_TOML.replace("Planner.plan", "Planner.plann"));
    all_commands_stop(
        dir.path(),
        &[
            "[auth] protect lists Planner.plann, but no such message is declared.",
            "did you mean `Planner.plan`?",
        ],
    );
    let dir = project(&BASIC_TOML.replace("Planner.plan", "Planer.plan"));
    all_commands_stop(dir.path(), &["did you mean `Planner.plan`?"]);
}

#[test]
fn protect_without_a_validator_and_wrong_values_stop_everything() {
    let dir = project("[auth]\nprotect = [\"Planner.plan\"]\n");
    all_commands_stop(
        dir.path(),
        &["[auth] protects messages but has no validator."],
    );
    let dir = project(&format!("{}timeout_seconds = 0\n", BASIC_TOML));
    all_commands_stop(
        dir.path(),
        &["`timeout_seconds` in [auth] must be a whole number from 1 to 60"],
    );
    let dir = project(&format!("{}timeout_seconds = \"5\"\n", BASIC_TOML));
    all_commands_stop(
        dir.path(),
        &["`timeout_seconds` in [auth] must be a whole number"],
    );
    let dir = project("[auth]\nvalidator = \"./validate.sh\"\nprotect = \"Planner.plan\"\n");
    all_commands_stop(dir.path(), &["`protect` in [auth] must be a list of texts"]);
    let dir = project("[auth]\nvalidator = \"./validate.sh\"\nprotect = [\"Planner\"]\n");
    all_commands_stop(dir.path(), &["which is not Agent.message"]);
}

#[test]
fn a_name_declared_in_a_sibling_file_is_not_a_mistake() {
    let dir = project(&BASIC_TOML.replace("Planner.plan", "Other.go"));
    write(
        dir.path(),
        "other.ag",
        "agent Other\n  goal \"x\"\n  accepts go\n  on go\n    reply \"ok\"\n",
    );
    let (ok, out, err) = run(dir.path(), &["check", "planner.ag"]);
    assert!(ok, "{}{}", out, err);
}

#[test]
fn open_plain_http_and_stdio_are_refused_for_a_protected_message() {
    let dir = project(BASIC_TOML);
    let (ok, _, err) = run(
        dir.path(),
        &["serve", "planner.ag", "--a2a", "0", "--public"],
    );
    assert!(!ok);
    assert!(
        err.contains("Planner.plan is protected, and this address is plain http and open to other computers: 0.0.0.0:"),
        "{}",
        err
    );
    assert!(err.contains("A token would travel in the open."));
    assert!(
        err.contains("serve with tls_cert and tls_key, or remove Planner.plan from [auth] protect")
    );
    let (ok, _, err) = run(dir.path(), &["serve", "planner.ag", "--mcp", "stdio"]);
    assert!(!ok);
    assert!(
        err.contains("Planner.plan is protected, and stdio has no place for a token."),
        "{}",
        err
    );
    assert!(err.contains("serve it with --mcp PORT"));
}

#[tokio::test(flavor = "multi_thread")]
async fn this_computer_starts_with_a_warning_and_a_certificate_starts_even_when_public() {
    let dir = project(BASIC_TOML);
    let port = free_port();
    let server = serve(
        dir.path(),
        "local",
        &["planner.ag", "--a2a", &port.to_string()],
        port,
    );
    assert!(
        server
            .log()
            .contains("Warning: protected messages are served over plain http on this computer only. The token is not encrypted."),
        "{}",
        server.log()
    );
    drop(server);
    let dir = project(&format!(
        "{}\n[serve]\ntls_cert = \"cert.pem\"\ntls_key = \"key.pem\"\n",
        BASIC_TOML
    ));
    pair_in(dir.path());
    let port = free_port();
    let server = serve(
        dir.path(),
        "tls",
        &["planner.ag", "--a2a", &port.to_string(), "--public"],
        port,
    );
    assert!(!server.log().contains("not encrypted"), "{}", server.log());
}

#[tokio::test(flavor = "multi_thread")]
async fn deleting_auth_while_serving_changes_nothing_and_says_a_restart_is_needed() {
    let dir = project(BASIC_TOML);
    let port = free_port();
    let server = serve(
        dir.path(),
        "a2a",
        &["planner.ag", "--a2a", &port.to_string()],
        port,
    );
    write(
        dir.path(),
        "metagente.toml",
        "[version]\nminimum = \"0.1.4\"\n",
    );
    // each request reads the file again; the protection must not move
    for _ in 0..2 {
        assert_eq!(send(port, "/a2a", None, "plan", "x").await.status, 401);
    }
    let log = server.log();
    assert_eq!(
        log.matches(
            "Note: metagente.toml changed [auth] (or [version]). I keep serving as I started"
        )
        .count(),
        1,
        "{}",
        log
    );
    assert!(!agent_ran(dir.path()));
}

#[test]
fn a_validator_that_cannot_be_found_is_a_warning_in_check_and_never_run() {
    let dir = project(&BASIC_TOML.replace("./validate.sh", "no-such-program-here"));
    let (ok, out, err) = run(dir.path(), &["check", "planner.ag"]);
    assert!(ok, "{}{}", out, err);
    assert!(
        err.contains("Warning: the validator program in [auth] was not found"),
        "{}",
        err
    );
    assert_eq!(runs(dir.path()), 0);
}

#[test]
fn check_notes_an_auth_that_protects_nothing() {
    let dir = project("[version]\nminimum = \"0.1.4\"\n[auth]\nvalidator = \"./validate.sh\"\n");
    let (ok, out, _) = run(dir.path(), &["check", "planner.ag"]);
    assert!(ok);
    assert!(out.contains("Note: [auth] protects no message"), "{}", out);
}

#[test]
fn forward_names_that_cannot_receive_a_token_are_errors_and_unknown_ones_warnings() {
    let agent = "agent Fwd\n  goal \"x\"\n  tool file\n  tool http\n  tool local from mcp \"some-program --stdio\"\n  tool web from mcp \"https://tools.example.com/mcp\"\n  remote Bob at \"https://bob.example.com\"\n  accepts go\n  on go\n    reply \"ok\"\n";
    for (name, expect) in [
        (
            "file",
            "file is a built in tool, so it cannot receive a token.",
        ),
        (
            "http",
            "http is a built in tool, so it cannot receive a token.",
        ),
        (
            "local",
            "local runs as a local program, so it cannot receive a token.",
        ),
        (
            "llm",
            "llm is the language model, so it cannot receive a token.",
        ),
    ] {
        let dir = project(&format!("{}forward = [\"{}\"]\n", BASIC_TOML, name));
        write(dir.path(), "fwd.ag", agent);
        let (ok, _, err) = run(dir.path(), &["check", "fwd.ag"]);
        assert!(!ok, "{}", name);
        assert!(err.contains(expect), "{}: {}", name, err);
        assert!(err.contains("remove it from [auth] forward"));
    }
    // a remote and an https tool are fine; a name that matches nothing is only a warning with the closest name
    let dir = project(&format!(
        "{}forward = [\"Bob\", \"web\", \"Bobb\"]\n",
        BASIC_TOML
    ));
    write(dir.path(), "fwd.ag", agent);
    let (ok, _, err) = run(dir.path(), &["check", "fwd.ag"]);
    assert!(ok, "{}", err);
    assert!(
        err.contains("Warning: [auth] forward lists Bobb, but nothing with that name is declared"),
        "{}",
        err
    );
    assert!(err.contains("did you mean `Bob`?"));
}
