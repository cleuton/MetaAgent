// 0.1.4: no token in any output, log, card or stored task (FR-048, SC-004). Every token used is searched for in
// everything the processes printed and everything a client could read back.
use crate::auth_forward::chain;
use crate::common::auth::*;

#[tokio::test(flavor = "multi_thread")]
async fn no_token_appears_in_any_log_card_task_or_output() {
    let good = "tok-GOOD-8f3a9c1d2b";
    let bad = "tok-BAD-77aa11bb22cc";
    let c = chain(
        "\"Researcher\"",
        "\"kb\"",
        &format!("{}\n", good),
        &format!("{}\n", good),
        &format!("{}\n", good),
        "cache_seconds = 30\n",
    );
    let mut readable: Vec<String> = Vec::new();
    // calls: accepted, refused, malformed, none; over A2A and over MCP
    let ok = send(c.planner, "/a2a", Some(good), "plan", "rust").await;
    readable.push(ok.text.clone());
    readable.push(
        send(c.planner, "/a2a", Some(bad), "plan", "rust")
            .await
            .text,
    );
    readable.push(
        post_raw(
            c.planner,
            "/a2a",
            &[&format!("Bearer {} extra", bad)],
            plan_body("x"),
        )
        .await
        .text,
    );
    readable.push(send(c.planner, "/a2a", None, "plan", "rust").await.text);
    let id = ok
        .body
        .pointer("/result/task/id")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    let task = post(
        c.planner,
        "/a2a",
        Some(good),
        serde_json::json!({"jsonrpc": "2.0", "id": 9, "method": "GetTask", "params": {"id": id}}),
    )
    .await;
    readable.push(task.text);
    readable.push(
        reqwest::get(format!(
            "http://127.0.0.1:{}/.well-known/agent-card.json",
            c.planner
        ))
        .await
        .unwrap()
        .text()
        .await
        .unwrap(),
    );
    // what the operator can read: every process's messages, and `check` for each stage
    for p in &c.procs {
        readable.push(p.log());
    }
    for (dir, file) in [
        (&c.planner_dir, "planner.ag"),
        (&c.researcher_dir, "researcher.ag"),
        (&c.facts_dir, "facts.ag"),
    ] {
        let (_, out, err) = run(dir.path(), &["check", file]);
        readable.push(out);
        readable.push(err);
    }
    for text in &readable {
        for token in [good, bad] {
            assert!(!text.contains(token), "a token leaked into: {}", text);
        }
    }
    // the validator's own output is thrown away: it never reaches a log
    let all = readable.join("\n");
    assert!(
        !all.contains("valid-tokens"),
        "the validator's work must stay hidden"
    );
}
