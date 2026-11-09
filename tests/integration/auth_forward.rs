// 0.1.4: passing the token on, only to the names listed (US4; FR-044). A chain of three real Metagente processes, plus
// fake agents (not Metagente) that record what they receive.
use crate::common::auth::*;
use crate::common::fake_a2a::{self, Mode};
use futures::future::join_all;

const FACTS: &str = "agent Facts\n  goal \"Know a few facts\"\n  accepts lookup topic\n  on lookup\n    if topic is \"rust\"\n      reply \"Rust is a systems language\"\n    otherwise\n      reply \"No fact about {topic}\"\n";

fn researcher_ag(facts_port: u16) -> String {
    format!(
        "agent Researcher\n  goal \"Look up a fact\"\n  tool kb from mcp \"http://127.0.0.1:{}/mcp\"\n  accepts research topic\n  on research\n    fact = kb.Facts.lookup topic: topic\n    reply fact\n",
        facts_port
    )
}

fn planner_ag(researcher_port: u16) -> String {
    format!(
        "agent Planner\n  goal \"Ask the Researcher\"\n  remote Researcher at \"http://127.0.0.1:{}\"\n  accepts plan topic\n  accepts open topic\n  on plan\n    answer = Researcher.research topic: topic\n    reply \"Planner got: {{answer}}\"\n  on open\n    answer = Researcher.research topic: topic\n    reply \"Open got: {{answer}}\"\n",
        researcher_port
    )
}

pub struct Chain {
    pub planner: u16,
    pub facts_dir: tempfile::TempDir,
    pub researcher_dir: tempfile::TempDir,
    pub planner_dir: tempfile::TempDir,
    pub procs: Vec<Proc>,
}

/// Facts (MCP) <- Researcher (A2A) <- Planner (A2A). `tokens` are the tokens each stage accepts.
pub fn chain(
    planner_forward: &str,
    researcher_forward: &str,
    planner_tokens: &str,
    researcher_tokens: &str,
    facts_tokens: &str,
    cache: &str,
) -> Chain {
    let (fp, rp, pp) = (free_port(), free_port(), free_port());
    let facts_dir = stage(
        "facts.ag",
        FACTS,
        &format!("{}{}", toml_for("\"Facts.lookup\"", ""), cache),
        facts_tokens,
    );
    let researcher_dir = stage(
        "researcher.ag",
        &researcher_ag(fp),
        &format!(
            "{}{}",
            toml_for("\"Researcher.research\"", researcher_forward),
            cache
        ),
        researcher_tokens,
    );
    let planner_dir = stage(
        "planner.ag",
        &planner_ag(rp),
        &format!("{}{}", toml_for("\"Planner.plan\"", planner_forward), cache),
        planner_tokens,
    );
    let procs = vec![
        serve(
            facts_dir.path(),
            "facts",
            &["facts.ag", "--mcp", &fp.to_string()],
            fp,
        ),
        serve(
            researcher_dir.path(),
            "researcher",
            &["researcher.ag", "--a2a", &rp.to_string()],
            rp,
        ),
        serve(
            planner_dir.path(),
            "planner",
            &["planner.ag", "--a2a", &pp.to_string()],
            pp,
        ),
    ];
    Chain {
        planner: pp,
        facts_dir,
        researcher_dir,
        planner_dir,
        procs,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_listed_agent_and_a_listed_tool_receive_the_callers_token() {
    let c = chain(
        "\"Researcher\"",
        "\"kb\"",
        "good-token\n",
        "good-token\n",
        "good-token\n",
        "",
    );
    let r = send(c.planner, "/a2a", Some("good-token"), "plan", "rust").await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(answer(&r), "Planner got: Rust is a systems language");
    assert_eq!(seen_tokens(c.planner_dir.path()), vec!["good-token"]);
    assert_eq!(
        seen_tokens(c.researcher_dir.path()),
        vec!["good-token"],
        "the Researcher judged the caller's token"
    );
    assert_eq!(
        seen_tokens(c.facts_dir.path()),
        vec!["good-token"],
        "so did the tool"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_name_that_is_not_listed_gets_nothing_and_the_error_says_how_to_fix_it() {
    let c = chain(
        "",
        "\"kb\"",
        "good-token\n",
        "good-token\n",
        "good-token\n",
        "",
    );
    let r = send(c.planner, "/a2a", Some("good-token"), "plan", "rust").await;
    let text = answer(&r);
    assert!(
        text.contains("Researcher refused the call: it needs a token. If it needs the one this agent received, add Researcher to forward in [auth]."),
        "{}",
        text
    );
    assert_eq!(
        runs(c.researcher_dir.path()),
        0,
        "nothing at all reached the Researcher's validator"
    );
    // and a tool that is not listed
    let c = chain(
        "\"Researcher\"",
        "",
        "good-token\n",
        "good-token\n",
        "good-token\n",
        "",
    );
    let r = send(c.planner, "/a2a", Some("good-token"), "plan", "rust").await;
    let text = answer(&r);
    assert!(
        text.contains("kb refused the call: it needs a token. If it needs the one this agent received, add kb to forward in [auth]."),
        "{}",
        text
    );
    assert_eq!(runs(c.facts_dir.path()), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_request_without_a_token_sends_no_header_and_says_it_had_none_to_pass_on() {
    let c = chain(
        "\"Researcher\"",
        "\"kb\"",
        "good-token\n",
        "good-token\n",
        "good-token\n",
        "",
    );
    // `open` is public, but the Researcher is not: there is no token to pass on
    let r = send(c.planner, "/a2a", None, "open", "rust").await;
    let text = answer(&r);
    assert!(
        text.contains("it needs a token, and this request had none to pass on."),
        "{}",
        text
    );
    assert_eq!(runs(c.researcher_dir.path()), 0);
    // a token on a public message is not judged, but it is passed on
    let r = send(c.planner, "/a2a", Some("good-token"), "open", "rust").await;
    assert_eq!(
        answer(&r),
        "Open got: Rust is a systems language",
        "{}",
        r.text
    );
    assert_eq!(
        runs(c.planner_dir.path()),
        0,
        "a public message is not judged"
    );
    assert_eq!(seen_tokens(c.researcher_dir.path()), vec!["good-token"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn two_requests_at_the_same_time_never_see_each_others_token() {
    // the last stage accepts only tok-a: every tok-b request must fail there, and every tok-a request must work
    let c = chain(
        "\"Researcher\"",
        "\"kb\"",
        "tok-a\ntok-b\n",
        "tok-a\ntok-b\n",
        "tok-a\n",
        "",
    );
    let port = c.planner;
    let calls = (0..40).map(|i| {
        let token = if i % 2 == 0 { "tok-a" } else { "tok-b" };
        async move { (token, send(port, "/a2a", Some(token), "plan", "rust").await) }
    });
    for (token, r) in join_all(calls).await {
        let text = answer(&r);
        if token == "tok-a" {
            assert_eq!(
                text, "Planner got: Rust is a systems language",
                "{}",
                r.text
            );
        } else {
            assert!(
                text.contains("did not accept the token this agent received"),
                "{}",
                r.text
            );
        }
    }
    let seen = seen_tokens(c.facts_dir.path());
    assert!(seen.iter().all(|t| t == "tok-a" || t == "tok-b"));
    assert_eq!(seen.iter().filter(|t| *t == "tok-a").count(), 20);
    assert_eq!(seen.iter().filter(|t| *t == "tok-b").count(), 20);
}

fn planner_with_fake(fake_port: u16, forward: &str) -> (tempfile::TempDir, u16, Proc) {
    let dir = stage(
        "planner.ag",
        &planner_ag(fake_port),
        &toml_for("\"Planner.plan\"", forward),
        "good-token\n",
    );
    // `plan` calls Researcher.research
    let port = free_port();
    let server = serve(
        dir.path(),
        "planner",
        &["planner.ag", "--a2a", &port.to_string()],
        port,
    );
    (dir, port, server)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_fake_agent_sees_the_token_on_tasks_but_not_on_the_card() {
    let fake = fake_a2a::start(Mode::Normal, "").await;
    let (_dir, port, _server) = planner_with_fake(fake.port, "\"Researcher\"");
    let r = send(port, "/a2a", Some("good-token"), "plan", "x").await;
    assert_eq!(answer(&r), "Planner got: fake-answer", "{}", r.text);
    assert!(
        fake.authorizations("/card").iter().all(|a| a.is_none()),
        "the public card carries no token"
    );
    assert_eq!(
        fake.authorizations("/a2a"),
        vec![Some("Bearer good-token".to_string())]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_fake_agent_that_is_not_listed_sees_no_authorization_header_at_all() {
    let fake = fake_a2a::start(Mode::Normal, "").await;
    let (_dir, port, _server) = planner_with_fake(fake.port, "");
    let r = send(port, "/a2a", Some("good-token"), "plan", "x").await;
    assert_eq!(
        answer(&r),
        "Planner got: fake-answer",
        "a public destination keeps working"
    );
    assert!(fake.count() >= 2);
    assert!(fake.seen.lock().unwrap().iter().all(|(_, a)| a.is_none()));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_destination_that_answers_401_is_explained() {
    let fake = fake_a2a::start(Mode::Unauthorized, "").await;
    let (_dir, port, _server) = planner_with_fake(fake.port, "\"Researcher\"");
    let r = send(port, "/a2a", Some("good-token"), "plan", "x").await;
    assert!(
        answer(&r).contains(
            "Researcher refused the call: it did not accept the token this agent received."
        ),
        "{}",
        r.text
    );
    let (_dir, port, _server) = planner_with_fake(fake.port, "");
    let r = send(port, "/a2a", Some("good-token"), "plan", "x").await;
    assert!(
        answer(&r).contains("Researcher refused the call: it needs a token. If it needs the one this agent received, add Researcher to forward in [auth]."),
        "{}",
        r.text
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_redirect_never_carries_the_token() {
    let elsewhere = fake_a2a::start(Mode::Normal, "").await;
    let target = format!("http://127.0.0.1:{}/a2a", elsewhere.port);
    let fake = fake_a2a::start(Mode::Redirect, &target).await;
    let (_dir, port, _server) = planner_with_fake(fake.port, "\"Researcher\"");
    let r = send(port, "/a2a", Some("good-token"), "plan", "x").await;
    assert!(
        answer(&r).contains("answered with a redirect, and I do not send a token along a redirect"),
        "{}",
        r.text
    );
    assert_eq!(
        elsewhere.authorizations("/a2a").len(),
        0,
        "nothing followed the redirect"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_listed_destination_in_the_open_is_refused_before_any_connection() {
    let dir = stage(
        "planner.ag",
        "agent Planner\n  goal \"x\"\n  remote Researcher at \"http://203.0.113.9:8081\"\n  accepts plan topic\n  on plan\n    answer = Researcher.research topic: topic\n    reply \"got {answer}\"\n",
        &toml_for("\"Planner.plan\"", "\"Researcher\""),
        "good-token\n",
    );
    let port = free_port();
    let _server = serve(
        dir.path(),
        "planner",
        &["planner.ag", "--a2a", &port.to_string()],
        port,
    );
    let started = std::time::Instant::now();
    let r = send(port, "/a2a", Some("good-token"), "plan", "x").await;
    assert!(
        answer(&r).contains("Researcher is in [auth] forward, but its address is plain http and not on this computer: the token would travel in the open."),
        "{}",
        r.text
    );
    assert!(
        started.elapsed().as_secs() < 3,
        "refused before trying to connect"
    );
    // check says so too
    let (_, _, err) = run(dir.path(), &["check", "planner.ag"]);
    assert!(
        err.contains("Researcher is in [auth] forward, but its address is plain http"),
        "{}",
        err
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tool_server_that_answers_401_is_explained_and_sees_the_token_only_when_listed() {
    let fake = fake_a2a::start(Mode::Normal, "").await;
    let source = format!(
        "agent Researcher\n  goal \"x\"\n  tool kb from mcp \"http://127.0.0.1:{}/mcp\"\n  accepts research topic\n  on research\n    fact = kb.Facts.lookup topic: topic\n    reply fact\n",
        fake.port
    );
    for (forward, want) in [
        (
            "\"kb\"",
            "kb refused the call: it did not accept the token this agent received.",
        ),
        (
            "",
            "kb refused the call: it needs a token. If it needs the one this agent received, add kb to forward in [auth].",
        ),
    ] {
        let dir = stage(
            "researcher.ag",
            &source,
            &toml_for("\"Researcher.research\"", forward),
            "good-token\n",
        );
        let port = free_port();
        let _server = serve(
            dir.path(),
            "r",
            &["researcher.ag", "--a2a", &port.to_string()],
            port,
        );
        let r = send(port, "/a2a", Some("good-token"), "research", "x").await;
        assert!(answer(&r).contains(want), "{}", r.text);
    }
    let listed: Vec<_> = fake.authorizations("/other");
    assert!(
        listed.contains(&Some("Bearer good-token".to_string())),
        "{:?}",
        listed
    );
    assert!(
        listed.contains(&None),
        "the unlisted run sent nothing: {:?}",
        listed
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tool_the_model_asks_for_gets_the_token_too() {
    use metagente::llm::fake::FakeLlm;
    use metagente::llm::{LlmReply, ToolCallReq};
    use metagente::runtime::auth::{AuthPolicy, RequestAuth, parse_token};
    use metagente::runtime::task::{CallContext, new_task_id};
    use std::sync::Arc;
    let fp = free_port();
    let facts_dir = stage(
        "facts.ag",
        FACTS,
        &toml_for("\"Facts.lookup\"", ""),
        "good-token\n",
    );
    let _facts = serve(
        facts_dir.path(),
        "facts",
        &["facts.ag", "--mcp", &fp.to_string()],
        fp,
    );
    let fake = Arc::new(FakeLlm::scripted(
        vec![LlmReply::ToolCall(ToolCallReq {
            id: "c1".into(),
            name: "kb__Facts_lookup".into(),
            args: serde_json::json!({"topic": "rust"}),
        })],
        LlmReply::Text("done".into()),
    ));
    let dir = tempfile::tempdir().unwrap();
    let policy = Arc::new(AuthPolicy {
        validator: Some("v".into()),
        protect: vec![],
        forward: vec!["kb".into()],
        cache_seconds: 0,
        timeout_seconds: 5,
        base_dir: dir.path().to_path_buf(),
        file: "metagente.toml".into(),
        forward_line: 0,
        cache: Default::default(),
    });
    let rt = crate::common::runtime_in(dir.path(), Some(fake.clone()), |c| {
        c.auth = Some(policy.clone())
    });
    let source = format!(
        "agent Thinker\n  goal \"x\"\n  tool kb from mcp \"http://127.0.0.1:{}/mcp\"\n  accepts ask\n  on ask\n    reply think \"look it up\"\n",
        fp
    );
    let agent = crate::common::agent_from(&rt, &source, None);
    let mut ctx = CallContext::new(new_task_id());
    ctx.auth = Some(RequestAuth::network(
        policy,
        Some(parse_token("good-token").unwrap()),
        true,
    ));
    let reply = agent.handle("ask", Default::default(), &ctx).await.unwrap();
    assert_eq!(reply.to_display(), "done");
    use metagente::llm::ChatMsg;
    let requests = fake.requests();
    match requests[1].messages.last().unwrap() {
        ChatMsg::ToolResult { content, .. } => assert!(
            content.contains("Rust is a systems language"),
            "{}",
            content
        ),
        other => panic!("{:?}", other),
    }
    assert_eq!(seen_tokens(facts_dir.path()), vec!["good-token"]);
}
