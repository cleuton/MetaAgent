use crate::common::a2a::{self, WEATHER};
use crate::common::*;

fn trip(base: &str) -> String {
    project_file("examples/remote_weather.ag").replace("http://127.0.0.1:8080", base)
}

#[tokio::test]
async fn an_agent_calls_a_remote_agent_served_by_another_metagente() {
    let served_dir = tempfile::tempdir().unwrap();
    let server = a2a::start(served_dir.path(), WEATHER, None).await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let reply = run_source(&rt, &trip(&server.base), "plan", &[("city", "Lisbon")])
        .await
        .unwrap();
    assert_eq!(reply.to_display(), "Bob says: sunny in Lisbon");
}

#[tokio::test]
async fn a_remote_agent_that_fails_is_reported_with_its_own_words() {
    let served_dir = tempfile::tempdir().unwrap();
    let server = a2a::start(served_dir.path(), WEATHER, None).await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let source = trip(&server.base).replace("Bob.ask city: city", "Bob.broken");
    let err = run_source(&rt, &source, "plan", &[("city", "Lisbon")])
        .await
        .unwrap_err()
        .render();
    assert!(
        err.contains("agent Bob could not answer `broken`"),
        "{}",
        err
    );
    assert!(err.contains("the barometer exploded"), "{}", err);
}

#[tokio::test]
async fn an_action_the_remote_agent_does_not_offer_is_explained() {
    let served_dir = tempfile::tempdir().unwrap();
    let server = a2a::start(served_dir.path(), WEATHER, None).await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let source = trip(&server.base).replace("Bob.ask city: city", "Bob.asc city: city");
    let err = run_source(&rt, &source, "plan", &[("city", "Lisbon")])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("does not handle `asc`"), "{}", err);
    assert!(err.contains("did you mean `Bob.ask`?"), "{}", err);
}

#[tokio::test]
async fn a_remote_that_is_not_there_is_explained() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let source = trip("http://127.0.0.1:1");
    let err = run_source(&rt, &source, "plan", &[("city", "Lisbon")])
        .await
        .unwrap_err()
        .render();
    assert!(
        err.contains("I could not reach http://127.0.0.1:1/.well-known/agent-card.json"),
        "{}",
        err
    );
    assert!(err.contains("metagente serve"), "{}", err);
}

#[tokio::test]
async fn structured_replies_come_back_as_records() {
    let served_dir = tempfile::tempdir().unwrap();
    let source = "agent Data\n  goal \"Give data\"\n  tool clock\n  accepts now\n  on now\n    reply clock.now\n";
    let server = a2a::start(served_dir.path(), source, None).await;
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let caller = format!(
        "agent C\n  goal \"c\"\n  remote D at \"{}\"\n  accepts go\n  on go\n    t = D.now\n    reply t.unix\n",
        server.base
    );
    let reply = run_source(&rt, &caller, "go", &[]).await.unwrap();
    assert!(reply.as_number().unwrap() > 1_700_000_000.0);
}
