// 0.1.4: a protected message cannot be reached by a side door (US5; FR-045).
use crate::common::auth::*;

const ADMIN: &str = "agent Admin\n  goal \"Dangerous things\"\n  tool file\n  accepts delete\n  on delete\n    file.write path: \"admin-ran.txt\" text: \"yes\"\n    reply \"deleted\"\n";
const ENTRY: &str = "agent Entry\n  goal \"Public door\"\n  link Admin from \"admin.ag\"\n  accepts plan topic\n  on plan\n    reply Admin.delete\n";

fn folder(protect: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "admin.ag", ADMIN);
    write(dir.path(), "entry.ag", ENTRY);
    write(
        dir.path(),
        "metagente.toml",
        &format!(
            "[version]\nminimum = \"0.1.4\"\n[auth]\nvalidator = \"./validate.sh\"\nprotect = [{}]\n",
            protect
        ),
    );
    write(dir.path(), "valid-tokens.txt", "good-token\nother-token\n");
    counting_validator(dir.path());
    dir
}

#[tokio::test(flavor = "multi_thread")]
async fn a_public_message_cannot_link_to_a_protected_one_over_the_network() {
    let dir = folder("\"Admin.delete\"");
    let port = free_port();
    let server = serve(
        dir.path(),
        "a2a",
        &["entry.ag", "--a2a", &port.to_string()],
        port,
    );
    for token in [None, Some("good-token")] {
        // `plan` is public, so even a good token on it is not judged: the request never carried an accepted one
        let r = send(port, "/a2a", token, "plan", "x").await;
        let text = answer(&r);
        assert!(
            text.contains("Admin.delete is protected, and the request that reached Entry did not carry an accepted token."),
            "{}",
            text
        );
        assert!(
            text.contains("protect the first message too, or call Admin.delete over the network"),
            "{}",
            text
        );
    }
    assert!(!dir.path().join("admin-ran.txt").exists());
    assert!(server.log().contains("Refused Admin.delete"));
}

#[tokio::test(flavor = "multi_thread")]
async fn when_the_first_message_is_protected_too_an_accepted_token_opens_the_linked_one() {
    let dir = folder("\"Admin.delete\", \"Entry.plan\"");
    let port = free_port();
    let _server = serve(
        dir.path(),
        "a2a",
        &["entry.ag", "--a2a", &port.to_string()],
        port,
    );
    let none = send(port, "/a2a", None, "plan", "x").await;
    assert_eq!(none.status, 401);
    let good = send(port, "/a2a", Some("good-token"), "plan", "x").await;
    assert_eq!(answer(&good), "deleted", "{}", good.text);
    assert!(dir.path().join("admin-ran.txt").exists());
}

#[test]
fn at_the_terminal_nothing_is_judged() {
    let dir = folder("\"Admin.delete\", \"Entry.plan\"");
    let (ok, out, err) = run(dir.path(), &["run", "entry.ag", "plan", "topic=x"]);
    assert!(ok, "{}{}", out, err);
    assert_eq!(out.trim(), "deleted");
    assert_eq!(
        runs(dir.path()),
        0,
        "no validator, no token: the person at the terminal owns the machine"
    );
}

fn task_id(r: &Reply) -> String {
    r.body
        .pointer("/result/task/id")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

async fn get_task(port: u16, token: Option<&str>, id: &str) -> Reply {
    post(
        port,
        "/a2a",
        token,
        serde_json::json!({"jsonrpc": "2.0", "id": 9, "method": "GetTask", "params": {"id": id}}),
    )
    .await
}

#[tokio::test(flavor = "multi_thread")]
async fn a_protected_task_is_read_only_with_the_token_that_started_it() {
    let dir = project(BASIC_TOML);
    let port = free_port();
    let _server = serve(
        dir.path(),
        "a2a",
        &["planner.ag", "--a2a", &port.to_string()],
        port,
    );
    let made = send(port, "/a2a", Some("good-token"), "plan", "secret-result").await;
    assert_eq!(made.status, 200);
    let id = task_id(&made);
    for (what, token) in [
        ("no token", None),
        ("a refused token", Some("wrong")),
        ("another accepted token", Some("other-token")),
    ] {
        let r = get_task(port, token, &id).await;
        assert_eq!(r.status, 401, "{}", what);
        assert_eq!(r.www_authenticate.as_deref(), Some("Bearer"));
        assert!(
            answer(&r).contains("This task needs the token that started it."),
            "{}: {}",
            what,
            r.text
        );
        assert!(
            !r.text.contains("secret-result"),
            "{} must not leak the result",
            what
        );
    }
    let same = get_task(port, Some("good-token"), &id).await;
    assert_eq!(same.status, 200, "{}", same.text);
    assert!(same.text.contains("planned secret-result"));
    // an id that does not exist answers as before, token or not
    let missing = get_task(port, None, "task-0-0").await;
    assert_eq!(missing.status, 200);
    assert!(
        answer(&missing).contains("Task not found"),
        "{}",
        missing.text
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_task_from_a_public_message_is_read_without_a_token_as_before() {
    let dir = project(BASIC_TOML);
    let port = free_port();
    let _server = serve(
        dir.path(),
        "a2a",
        &["planner.ag", "--a2a", &port.to_string()],
        port,
    );
    let made = send(port, "/a2a", None, "ping", "").await;
    let r = get_task(port, None, &task_id(&made)).await;
    assert_eq!(r.status, 200);
    assert!(r.text.contains("pong"));
}
