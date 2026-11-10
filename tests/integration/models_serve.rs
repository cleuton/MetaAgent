// 0.1.5: new file (D8, FR-006). `[llm.NAME]` is read at start by `metagente serve`: a later edit is not applied and one
// plain line asks for a restart, once per new state; an agent edited to use a nickname that was not read at start is a
// broken edit, so the last good version keeps being served and nothing is printed for that agent.
use crate::common::self_signed::{ServeProc, free_port};
use serde_json::json;

const NOTE: &str = "Note: metagente.toml changed the models under [llm]. I keep serving as I started; restart `metagente serve` for the change to apply.";

fn toml(fast: &str) -> String {
    format!(
        "[version]\nminimum = \"{}\"\n\n[llm]\nprovider = \"anthropic\"\nmodel = \"m\"\n\n[llm.fast]\n{}\n",
        env!("CARGO_PKG_VERSION"),
        fast
    )
}

const FAST_ANTHROPIC: &str =
    "provider = \"anthropic\"\nmodel = \"m\"\napi_key_env = \"SERVE_FAST_KEY\"";
const FAST_OPENAI: &str = "provider = \"openai\"\nmodel = \"m2\"\napi_key_env = \"SERVE_FAST_KEY\"";
const FAST_OPENAI_B: &str =
    "provider = \"openai\"\nmodel = \"m3\"\napi_key_env = \"SERVE_FAST_KEY\"";

fn agent(reply: &str) -> String {
    format!(
        "agent A\n  goal \"x\"\n  accepts ask\n  on ask\n    reply {}\n",
        reply
    )
}

struct Served {
    dir: tempfile::TempDir,
    serve: ServeProc,
    base: String,
    client: reqwest::Client,
}

fn start(toml: &str, agent: &str) -> Served {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("metagente.toml"), toml).unwrap();
    std::fs::write(dir.path().join("a.ag"), agent).unwrap();
    let port = free_port();
    let serve = ServeProc::start(
        dir.path(),
        &["a.ag".to_string(), "--a2a".to_string(), port.to_string()],
        1,
    );
    Served {
        dir,
        serve,
        base: format!("http://127.0.0.1:{}", port),
        client: reqwest::Client::new(),
    }
}

impl Served {
    async fn card(&self) {
        self.client
            .get(format!("{}/.well-known/agent-card.json", self.base))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
    }

    async fn ask(&self) -> String {
        let body = json!({"jsonrpc": "2.0", "id": 1, "method": "SendMessage", "params": crate::common::a2a::user_text("ask")});
        self.client
            .post(format!("{}/a2a", self.base))
            .header("A2A-Version", "1.0")
            .body(body.to_string())
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
    }

    fn write(&self, name: &str, text: &str) {
        std::fs::write(self.dir.path().join(name), text).unwrap();
    }

    fn notes(&self) -> usize {
        self.serve.said().matches(NOTE).count()
    }

    /// The log of the child is read by another thread, so a line may arrive a moment after the answer. Wait until the
    /// wanted number of lines is there, then a little longer to catch an extra one, and compare.
    async fn expect_notes(&self, wanted: usize, why: &str) {
        for _ in 0..60 {
            if self.notes() >= wanted {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        assert_eq!(self.notes(), wanted, "{}: {}", why, self.serve.said());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn editing_the_models_while_serving_prints_one_line_once_per_new_state_and_applies_nothing() {
    let s = start(&toml(FAST_ANTHROPIC), &agent("\"v1\""));
    s.card().await;
    s.expect_notes(0, "").await;
    s.write("metagente.toml", &toml(FAST_OPENAI));
    s.card().await;
    s.expect_notes(1, "").await;
    s.card().await;
    s.ask().await;
    s.expect_notes(1, "once per new state").await;
    s.write("metagente.toml", &toml(FAST_OPENAI_B));
    s.card().await;
    s.expect_notes(2, "a different new state is told again")
        .await;
    // putting it back is the state of the start: nothing to tell
    s.write("metagente.toml", &toml(FAST_ANTHROPIC));
    s.card().await;
    s.expect_notes(2, "").await;
    assert!(s.ask().await.contains("v1"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_edit_of_llm_itself_prints_no_new_line() {
    let s = start(&toml(FAST_ANTHROPIC), &agent("\"v1\""));
    s.card().await;
    s.write(
        "metagente.toml",
        &format!(
            "[version]\nminimum = \"{}\"\n\n[llm]\nprovider = \"openai\"\nmodel = \"other\"\n\n[llm.fast]\n{}\n",
            env!("CARGO_PKG_VERSION"),
            FAST_ANTHROPIC
        ),
    );
    s.card().await;
    s.ask().await;
    s.expect_notes(0, "[llm] itself has no restart line").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_agent_edited_to_use_a_model_not_read_at_start_keeps_the_last_good_version() {
    let s = start(&toml(FAST_ANTHROPIC), &agent("\"v1\""));
    assert!(s.ask().await.contains("v1"));
    // the person adds [llm.newone] and uses it in the agent, but the models are read at start
    s.write(
        "metagente.toml",
        &format!(
            "{}\n[llm.newone]\nprovider = \"openai\"\nmodel = \"m\"\n",
            toml(FAST_ANTHROPIC)
        ),
    );
    s.write("a.ag", &agent("think \"hi\" using newone"));
    let reply = s.ask().await;
    assert!(
        reply.contains("v1"),
        "the last good version keeps answering: {}",
        reply
    );
    s.expect_notes(1, "the restart line tells the person").await;
    // an edit that uses a model that was read at start is applied
    s.write("a.ag", &agent("\"v2\""));
    assert!(s.ask().await.contains("v2"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_bad_nickname_edit_prints_nothing_for_that_agent() {
    let s = start(&toml(FAST_ANTHROPIC), &agent("\"v1\""));
    s.card().await;
    s.write("a.ag", &agent("think \"hi\" using fats"));
    let reply = s.ask().await;
    assert!(reply.contains("v1"), "{}", reply);
    assert!(!s.serve.said().contains("fats"), "{}", s.serve.said());
}

#[test]
fn serve_refuses_to_start_when_an_agent_uses_an_unknown_model() {
    use crate::common::self_signed::serve_refused;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("metagente.toml"), toml(FAST_ANTHROPIC)).unwrap();
    std::fs::write(dir.path().join("a.ag"), agent("think \"hi\" using fats")).unwrap();
    let out = serve_refused(dir.path(), &["a.ag".to_string()]);
    let text = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(!out.status.success());
    assert!(
        text.contains("There is no model called fats. The models are: fast."),
        "{}",
        text
    );
    assert!(text.contains("did you mean fast?"), "{}", text);
}
