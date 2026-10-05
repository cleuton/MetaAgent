//! The City Briefing sample (samples/city-briefing) run offline: a scripted model instead of Claude,
//! a fake MCP server instead of `uvx mcp-server-fetch`, and the real sample files.

use crate::common::a2a;
use crate::common::web::{self, text_reply, tool_use};
use crate::common::*;
use metagente::runtime::Runtime;
use metagente::runtime::config::Config;
use metagente::runtime::interpreter::Agent;
use metagente::runtime::run::load_agents;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The variable the staged copies read their key from, so tests never touch the real one.
pub const TEST_KEY: &str = "MG_SAMPLE_TEST_KEY";

pub fn sample_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/city-briefing")
}

/// Copies the sample into `dir`, changing only the addresses that point at real services:
/// the MCP command, the Researcher's address, and where the model lives.
pub fn staged(
    dir: &Path,
    mcp_command: &str,
    researcher_url: &str,
    model_base_url: &str,
) -> PathBuf {
    for file in [
        "README.md",
        "metagente.toml",
        "concierge.ag",
        "researcher.ag",
    ] {
        // Copy what exists; the test that checks the four files are there is separate.
        let Ok(text) = std::fs::read_to_string(sample_dir().join(file)) else {
            continue;
        };
        let text = match file {
            "researcher.ag" => text.replace("uvx mcp-server-fetch", mcp_command),
            "concierge.ag" => text.replace("http://127.0.0.1:8080", researcher_url),
            "metagente.toml" => text
                .replace("\"ANTHROPIC_API_KEY\"", &format!("\"{}\"", TEST_KEY))
                .replace(
                    &format!("api_key_env = \"{}\"", TEST_KEY),
                    &format!(
                        "api_key_env = \"{}\"\nbase_url = \"{}\"",
                        TEST_KEY, model_base_url
                    ),
                ),
            _ => text,
        };
        std::fs::write(dir.join(file), text).unwrap();
    }
    dir.to_path_buf()
}

/// A runtime configured by the staged `metagente.toml`, like `metagente run` would build.
pub fn runtime_for(dir: &Path) -> Arc<Runtime> {
    unsafe { std::env::set_var(TEST_KEY, "test-key") };
    let config = Config::load(dir).unwrap_or_else(|d| panic!("{}", d.render()));
    let llm = metagente::llm::providers::from_config(&config)
        .unwrap_or_else(|d| panic!("{}", d.render()));
    Runtime::new(config, llm)
}

pub fn agent_named(rt: &Arc<Runtime>, file: &Path) -> Agent {
    let defs = load_agents(file).unwrap_or_else(|d| panic!("{}", d.render()));
    Agent::new(rt.clone(), defs[0].clone()).unwrap_or_else(|d| panic!("{}", d.render()))
}

/// What the model says when asked to fetch the page of `city`, then to write the facts.
pub fn researcher_script(city: &str) -> Vec<serde_json::Value> {
    vec![
        tool_use(
            "call-1",
            "fetch__fetch",
            json!({"url": format!("https://en.wikipedia.org/wiki/{}", city)}),
        ),
        text_reply(&format!(
            "{} is a city with a river. It has old streets. People love it.",
            city
        )),
    ]
}

#[allow(dead_code)]
pub async fn model(replies: Vec<serde_json::Value>) -> web::Web {
    web::start_scripted(replies).await
}

#[allow(dead_code)]
pub async fn serve_researcher(rt: Arc<Runtime>, dir: &Path) -> a2a::A2aServer {
    a2a::start_with_runtime(&dir.join("researcher.ag"), rt).await
}

// ---------- User Story 3: the Researcher gets facts through MCP ----------

use crate::common::fake_mcp;
use metagente::lang::ToolKind;

#[tokio::test]
async fn the_researcher_reads_a_page_through_mcp_and_stamps_the_time() {
    let mcp = fake_mcp::start().await;
    let model = web::start_scripted(researcher_script("Lisbon")).await;
    let dir = tempfile::tempdir().unwrap();
    staged(dir.path(), &mcp.url, "http://127.0.0.1:1", &model.base);
    let rt = runtime_for(dir.path());
    let researcher = agent_named(&rt, &dir.path().join("researcher.ag"));

    let reply = ask(&researcher, "research", &[("city", "Lisbon")])
        .await
        .unwrap()
        .to_display();

    assert!(
        reply.starts_with("Lisbon is a city with a river."),
        "{}",
        reply
    );
    assert!(
        reply.contains("(checked 20"),
        "the reply should carry a time stamp: {}",
        reply
    );
    // The model chose the page, and the call reached the MCP server with the city in the address.
    assert!(
        fake_mcp::fetched().iter().any(|u| u.ends_with("/Lisbon")),
        "{:?}",
        fake_mcp::fetched()
    );
    // The model was offered only what the Researcher declared.
    let seen = model.seen.lock().unwrap();
    let offered: Vec<String> = seen.bodies[0]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    assert!(
        offered.contains(&"fetch__fetch".to_string()),
        "{:?}",
        offered
    );
    assert!(offered.contains(&"clock__now".to_string()), "{:?}", offered);
    // Nothing outside the two things the Researcher declared (the fake server has a few extra tools of
    // its own, which all belong to `fetch`).
    assert!(
        offered
            .iter()
            .all(|n| n.starts_with("fetch__") || n.starts_with("clock__")),
        "{:?}",
        offered
    );
}

#[tokio::test]
async fn an_mcp_server_that_cannot_start_is_explained_with_the_command_that_failed() {
    let model = web::start_scripted(researcher_script("Lisbon")).await;
    let dir = tempfile::tempdir().unwrap();
    staged(
        dir.path(),
        "definitely-not-a-real-program --serve",
        "http://127.0.0.1:1",
        &model.base,
    );
    let rt = runtime_for(dir.path());
    let researcher = agent_named(&rt, &dir.path().join("researcher.ag"));

    let err = ask(&researcher, "research", &[("city", "Lisbon")])
        .await
        .unwrap_err()
        .render();

    assert!(
        err.contains("could not start the tool server `definitely-not-a-real-program --serve`"),
        "{}",
        err
    );
    assert!(err.contains("installed"), "{}", err);
    assert!(
        model.seen.lock().unwrap().bodies.is_empty(),
        "the model must not be called without its tools"
    );
}

#[tokio::test]
async fn the_researcher_declares_one_mcp_tool_and_the_clock_and_nothing_else() {
    let dir = tempfile::tempdir().unwrap();
    let defs = load_agents(&sample_dir().join("researcher.ag")).unwrap();
    let researcher = &defs[0];
    let declared: Vec<(&str, bool)> = researcher
        .tools
        .iter()
        .map(|t| (t.name.as_str(), matches!(t.kind, ToolKind::Mcp { .. })))
        .collect();
    assert_eq!(declared, vec![("fetch", true), ("clock", false)]);
    assert!(researcher.links.is_empty() && researcher.remotes.is_empty());

    // A model that asks for a tool the Researcher did not declare is turned down, not obeyed.
    std::fs::write(dir.path().join("secret.txt"), "top secret").unwrap();
    let mcp = fake_mcp::start().await;
    let model = web::start_scripted(vec![
        tool_use("call-1", "file__read", json!({"path": "secret.txt"})),
        text_reply("No facts were found."),
    ])
    .await;
    staged(dir.path(), &mcp.url, "http://127.0.0.1:1", &model.base);
    let rt = runtime_for(dir.path());
    let agent = agent_named(&rt, &dir.path().join("researcher.ag"));
    let reply = ask(&agent, "research", &[("city", "Lisbon")])
        .await
        .unwrap()
        .to_display();
    assert!(reply.starts_with("No facts were found."), "{}", reply);
    let seen = model.seen.lock().unwrap();
    let second = serde_json::to_string(&seen.bodies[1]).unwrap();
    assert!(second.contains("no tool called `file`"), "{}", second);
    assert!(
        !second.contains("top secret"),
        "the file must not have been read"
    );
}

#[tokio::test]
async fn an_empty_city_stops_early_and_a_missing_page_becomes_no_facts_found() {
    let mcp = fake_mcp::start().await;
    let model = web::start_scripted(vec![
        tool_use(
            "call-1",
            "fetch__fetch",
            json!({"url": "https://en.wikipedia.org/wiki/Xyzzyplugh"}),
        ),
        text_reply("No facts were found for Xyzzyplugh."),
    ])
    .await;
    let dir = tempfile::tempdir().unwrap();
    staged(dir.path(), &mcp.url, "http://127.0.0.1:1", &model.base);
    let rt = runtime_for(dir.path());
    let researcher = agent_named(&rt, &dir.path().join("researcher.ag"));

    let err = ask(&researcher, "research", &[("city", "")])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("I need a city"), "{}", err);
    assert!(err.contains("researcher.ag"), "{}", err);
    assert!(err.contains("line "), "{}", err);
    assert!(
        model.seen.lock().unwrap().bodies.is_empty(),
        "no model call for an empty city"
    );

    let reply = ask(&researcher, "research", &[("city", "Xyzzyplugh")])
        .await
        .unwrap()
        .to_display();
    assert!(
        reply.starts_with("No facts were found for Xyzzyplugh."),
        "{}",
        reply
    );
    // The failed fetch was handed to the model as text, not shown to the person as an error.
    let seen = model.seen.lock().unwrap();
    let second = serde_json::to_string(&seen.bodies[1]).unwrap();
    assert!(
        second.contains("Failed to fetch") && second.contains("404"),
        "{}",
        second
    );
}

// ---------- User Story 2: the Concierge delegates over A2A ----------

/// The model's three answers in the whole flow: the Researcher's fetch and facts, then the briefing.
fn full_script(city: &str) -> Vec<serde_json::Value> {
    let mut script = researcher_script(city);
    script.push(text_reply(&format!(
        "Welcome to {city}! It has a river and old streets. Enjoy your visit."
    )));
    script
}

#[tokio::test]
async fn the_concierge_asks_the_researcher_over_a2a_and_writes_the_briefing() {
    let mcp = fake_mcp::start().await;
    let model = web::start_scripted(full_script("Lisbon")).await;
    let dir = tempfile::tempdir().unwrap();
    staged(dir.path(), &mcp.url, "http://127.0.0.1:1", &model.base);
    let rt = runtime_for(dir.path());
    let researcher = serve_researcher(rt.clone(), dir.path()).await;
    // Point the staged Concierge at the Researcher that is now listening.
    let concierge_file = dir.path().join("concierge.ag");
    let text = std::fs::read_to_string(&concierge_file)
        .unwrap()
        .replace("http://127.0.0.1:1", &researcher.base);
    std::fs::write(&concierge_file, text).unwrap();
    let concierge = agent_named(&runtime_for(dir.path()), &concierge_file);

    let briefing = ask(&concierge, "brief", &[("city", "Lisbon")])
        .await
        .unwrap()
        .to_display();

    assert_eq!(
        briefing,
        "Welcome to Lisbon! It has a river and old streets. Enjoy your visit."
    );
    // The Concierge wrote its briefing from what the Researcher sent back over A2A.
    let seen = model.seen.lock().unwrap();
    assert_eq!(
        seen.bodies.len(),
        3,
        "researcher: fetch + facts, concierge: briefing"
    );
    let last = serde_json::to_string(&seen.bodies[2]).unwrap();
    assert!(last.contains("Lisbon is a city with a river."), "{}", last);
    assert!(
        last.contains("(checked 20"),
        "the Researcher's time stamp should travel too: {}",
        last
    );
}

#[tokio::test]
async fn with_the_researcher_stopped_the_error_names_it_and_says_how_to_start_it() {
    let mcp = fake_mcp::start().await;
    let model = web::start_scripted(full_script("Lisbon")).await;
    let dir = tempfile::tempdir().unwrap();
    // Nothing listens on port 1.
    staged(dir.path(), &mcp.url, "http://127.0.0.1:1", &model.base);
    let rt = runtime_for(dir.path());
    let concierge = agent_named(&rt, &dir.path().join("concierge.ag"));

    let err = ask(&concierge, "brief", &[("city", "Lisbon")])
        .await
        .unwrap_err()
        .render();

    assert!(err.contains("Researcher"), "{}", err);
    assert!(err.contains("http://127.0.0.1:1"), "{}", err);
    assert!(err.contains("metagente serve"), "{}", err);
    assert!(err.contains("Problem on line "), "{}", err);
    assert!(
        model.seen.lock().unwrap().bodies.is_empty(),
        "the model must not be asked to invent facts"
    );
}

#[test]
fn the_concierge_declares_only_the_researcher() {
    let defs = load_agents(&sample_dir().join("concierge.ag")).unwrap();
    let concierge = &defs[0];
    assert!(concierge.tools.is_empty(), "no tools");
    assert!(concierge.links.is_empty(), "no links");
    let remotes: Vec<&str> = concierge.remotes.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(remotes, vec!["Researcher"]);
    assert_eq!(concierge.remotes[0].url, "http://127.0.0.1:8080");
    assert_eq!(concierge.accepts.len(), 1);
    assert_eq!(concierge.accepts[0].message, "brief");
    assert_eq!(concierge.accepts[0].params, vec!["city"]);
}

mod local_only {
    use super::*;
    use std::net::{IpAddr, SocketAddr, TcpStream, UdpSocket};
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    struct Serving(Child);
    impl Drop for Serving {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn outside_ip() -> Option<IpAddr> {
        let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
        socket.connect("192.0.2.1:9").ok()?;
        let ip = socket.local_addr().ok()?.ip();
        if ip.is_loopback() || ip.is_unspecified() {
            None
        } else {
            Some(ip)
        }
    }

    #[test]
    fn serving_the_sample_researcher_is_local_only_and_needs_no_public_option() {
        // Nothing in the sample asks for the public option.
        for file in ["researcher.ag", "concierge.ag", "metagente.toml"] {
            let text = std::fs::read_to_string(sample_dir().join(file)).unwrap();
            assert!(
                !text.contains("public"),
                "{} mentions the public option",
                file
            );
        }
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let dir = tempfile::tempdir().unwrap();
        staged(
            dir.path(),
            "http://127.0.0.1:1/mcp",
            "http://127.0.0.1:1",
            "http://127.0.0.1:1",
        );
        let toml = std::fs::read_to_string(dir.path().join("metagente.toml")).unwrap();
        assert!(toml.contains("a2a_port = 8080"));
        std::fs::write(
            dir.path().join("metagente.toml"),
            toml.replace("a2a_port = 8080", &format!("a2a_port = {}", port)),
        )
        .unwrap();

        // Exactly the command from the README: no flags.
        let server = Serving(
            Command::new(env!("CARGO_BIN_EXE_metagente"))
                .current_dir(dir.path())
                .args(["serve", "researcher.ag"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let local = SocketAddr::from(([127, 0, 0, 1], port));
        let start = Instant::now();
        while TcpStream::connect_timeout(&local, Duration::from_millis(200)).is_err() {
            assert!(
                start.elapsed().as_secs() < 10,
                "the Researcher did not start"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        if let Some(ip) = outside_ip() {
            assert!(
                TcpStream::connect_timeout(&SocketAddr::new(ip, port), Duration::from_millis(500))
                    .is_err(),
                "{} could reach the Researcher without --public",
                ip
            );
        }
        drop(server);
    }
}

// ---------- User Story 1: run the demo from the samples folder ----------

/// Runs the whole demo offline and returns the model's mock so tests can look at its requests.
async fn whole_flow(dir: &Path, model: &web::Web, mcp: &fake_mcp::Running) -> String {
    staged(dir, &mcp.url, "http://127.0.0.1:1", &model.base);
    let rt = runtime_for(dir);
    let researcher = serve_researcher(rt.clone(), dir).await;
    let concierge_file = dir.join("concierge.ag");
    let text = std::fs::read_to_string(&concierge_file)
        .unwrap()
        .replace("http://127.0.0.1:1", &researcher.base);
    std::fs::write(&concierge_file, text).unwrap();
    let concierge = agent_named(&runtime_for(dir), &concierge_file);
    ask(&concierge, "brief", &[("city", "Lisbon")])
        .await
        .unwrap()
        .to_display()
}

#[tokio::test]
async fn both_agents_use_claude_sonnet_5_5_and_no_agent_file_names_a_provider_or_model() {
    let mcp = fake_mcp::start().await;
    let model = web::start_scripted(full_script("Lisbon")).await;
    let dir = tempfile::tempdir().unwrap();
    whole_flow(dir.path(), &model, &mcp).await;

    let seen = model.seen.lock().unwrap();
    assert_eq!(seen.bodies.len(), 3);
    for body in &seen.bodies {
        assert_eq!(body["model"], "claude-sonnet-5-5");
    }
    // Requests go to the Anthropic messages endpoint, with the key in the header the toml named.
    assert!(seen.paths.iter().all(|p| p == "/v1/messages"));
    for headers in &seen.headers {
        assert_eq!(headers.get("x-api-key").unwrap(), "test-key");
    }
    for file in ["researcher.ag", "concierge.ag"] {
        let text = std::fs::read_to_string(sample_dir().join(file))
            .unwrap()
            .to_lowercase();
        for word in ["anthropic", "claude", "sonnet", "model", "provider"] {
            assert!(!text.contains(word), "{} names `{}`", file, word);
        }
    }
    let toml = std::fs::read_to_string(sample_dir().join("metagente.toml")).unwrap();
    assert!(toml.contains("provider = \"anthropic\""));
    assert!(toml.contains("model = \"claude-sonnet-5-5\""));
}

// The test starts the program while the fake servers run in this same process, so it needs a second thread.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_the_key_either_agent_says_which_variable_is_not_set() {
    let mcp = fake_mcp::start().await;
    let model = web::start_scripted(full_script("Lisbon")).await;

    // The Researcher alone, run from its folder the way the README says, with the key variable removed.
    let dir = tempfile::tempdir().unwrap();
    staged(dir.path(), &mcp.url, "http://127.0.0.1:1", &model.base);
    let toml = std::fs::read_to_string(dir.path().join("metagente.toml")).unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        toml.replace(TEST_KEY, "ANTHROPIC_API_KEY"),
    )
    .unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_metagente"))
        .current_dir(dir.path())
        .env_remove("ANTHROPIC_API_KEY")
        .args(["run", "researcher.ag", "city=Lisbon"])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(!out.status.success());
    assert!(err.contains("ANTHROPIC_API_KEY is not set"), "{}", err);
    for word in ["panicked", "Some(", "Err(", "unwrap"] {
        assert!(!err.contains(word), "{}", err);
    }
    assert!(model.seen.lock().unwrap().bodies.is_empty());

    // The Concierge: the Researcher (with a key) answers over A2A, then the Concierge has no key.
    let researcher_dir = tempfile::tempdir().unwrap();
    let model2 = web::start_scripted(full_script("Lisbon")).await;
    staged(
        researcher_dir.path(),
        &mcp.url,
        "http://127.0.0.1:1",
        &model2.base,
    );
    let researcher =
        serve_researcher(runtime_for(researcher_dir.path()), researcher_dir.path()).await;
    let concierge_dir = tempfile::tempdir().unwrap();
    staged(
        concierge_dir.path(),
        &mcp.url,
        &researcher.base,
        &model2.base,
    );
    let toml = std::fs::read_to_string(concierge_dir.path().join("metagente.toml")).unwrap();
    std::fs::write(
        concierge_dir.path().join("metagente.toml"),
        toml.replace(TEST_KEY, "ANTHROPIC_API_KEY"),
    )
    .unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_metagente"))
        .current_dir(concierge_dir.path())
        .env_remove("ANTHROPIC_API_KEY")
        .args(["run", "concierge.ag", "city=Lisbon"])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(!out.status.success());
    assert!(err.contains("ANTHROPIC_API_KEY is not set"), "{}", err);
    assert!(err.contains("Fix: "), "{}", err);
}

#[tokio::test]
async fn changing_only_the_model_line_changes_the_model_of_both_agents() {
    let mcp = fake_mcp::start().await;
    let mut agent_files = Vec::new();
    let mut models_seen = Vec::new();
    for model_name in ["claude-sonnet-5-5", "claude-haiku-4-5"] {
        let model = web::start_scripted(full_script("Lisbon")).await;
        let dir = tempfile::tempdir().unwrap();
        staged(dir.path(), &mcp.url, "http://127.0.0.1:1", &model.base);
        let toml = std::fs::read_to_string(dir.path().join("metagente.toml")).unwrap();
        assert!(toml.contains("model = \"claude-sonnet-5-5\""));
        std::fs::write(
            dir.path().join("metagente.toml"),
            toml.replace(
                "model = \"claude-sonnet-5-5\"",
                &format!("model = \"{}\"", model_name),
            ),
        )
        .unwrap();
        whole_flow_reuse(dir.path(), &model).await;
        let files: Vec<String> = ["researcher.ag", "concierge.ag"]
            .iter()
            .map(|f| std::fs::read_to_string(dir.path().join(f)).unwrap())
            .collect();
        agent_files.push(files);
        let seen = model.seen.lock().unwrap();
        models_seen.push(
            seen.bodies
                .iter()
                .map(|b| b["model"].as_str().unwrap().to_string())
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(models_seen[0], vec!["claude-sonnet-5-5"; 3]);
    assert_eq!(models_seen[1], vec!["claude-haiku-4-5"; 3]);
    // The agent files did not change (apart from the address the test points at its own server).
    let strip = |s: &str| {
        s.split("http://127.0.0.1:")
            .next()
            .unwrap_or("")
            .to_string()
    };
    assert_eq!(agent_files[0][0], agent_files[1][0]);
    assert_eq!(strip(&agent_files[0][1]), strip(&agent_files[1][1]));
}

/// Like `whole_flow` but for a folder that is already staged.
async fn whole_flow_reuse(dir: &Path, _model: &web::Web) {
    let rt = runtime_for(dir);
    let researcher = serve_researcher(rt.clone(), dir).await;
    let concierge_file = dir.join("concierge.ag");
    let text = std::fs::read_to_string(&concierge_file)
        .unwrap()
        .replace("http://127.0.0.1:1", &researcher.base);
    std::fs::write(&concierge_file, text).unwrap();
    let concierge = agent_named(&runtime_for(dir), &concierge_file);
    ask(&concierge, "brief", &[("city", "Lisbon")])
        .await
        .unwrap();
}

#[test]
fn the_sample_has_all_its_files_and_the_readme_covers_everything() {
    for file in [
        "README.md",
        "metagente.toml",
        "concierge.ag",
        "researcher.ag",
    ] {
        assert!(
            sample_dir().join(file).is_file(),
            "samples/city-briefing/{} is missing",
            file
        );
    }
    let index = std::fs::read_to_string(sample_dir().join("../README.md")).unwrap();
    assert!(
        index.contains("city-briefing/"),
        "samples/README.md should list the sample"
    );

    let readme = std::fs::read_to_string(sample_dir().join("README.md")).unwrap();
    let headings: Vec<String> = readme
        .lines()
        .filter(|l| l.starts_with("## "))
        .map(|l| l.to_lowercase())
        .collect();
    for needed in [
        "prerequisites",
        "configur",
        "running",
        "question",
        "a2a",
        "troubleshoot",
    ] {
        assert!(
            headings.iter().any(|h| h.contains(needed)),
            "the README has no `{}` section: {:?}",
            needed,
            headings
        );
    }
    assert!(
        readme.contains("You should see something like"),
        "the README should show the shape of the answer"
    );
    assert!(readme.contains("metagente serve researcher.ag"));
    assert!(readme.contains("metagente run concierge.ag"));
    // The README shows the two agents exactly as they are in the files.
    for file in ["researcher.ag", "concierge.ag"] {
        let body = std::fs::read_to_string(sample_dir().join(file)).unwrap();
        assert!(
            readme.contains(body.trim_end()),
            "the README does not show {} as it is",
            file
        );
    }
    assert!(readme.contains("uv"));
    assert!(readme.contains("ANTHROPIC_API_KEY"));
    // The demo never needs the public option, so no command in the README may use it.
    let mut in_code = false;
    for line in readme.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
        } else if in_code {
            assert!(
                !line.contains("--public"),
                "a command uses --public: {}",
                line
            );
        }
    }
}

#[test]
fn sizes_stay_small_and_no_key_is_in_any_file() {
    let count = |f: &str| {
        std::fs::read_to_string(sample_dir().join(f))
            .unwrap()
            .lines()
            .count()
    };
    assert!(
        count("researcher.ag") < 20,
        "researcher.ag has {} lines",
        count("researcher.ag")
    );
    assert!(
        count("concierge.ag") < 10,
        "concierge.ag has {} lines",
        count("concierge.ag")
    );

    let real_key = std::env::var("ANTHROPIC_API_KEY")
        .ok()
        .filter(|k| k.len() >= 8);
    let mut checked = 0;
    let mut stack = vec![sample_dir().join("..")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            checked += 1;
            // Something that looks like a key: `sk-` followed by ten or more key characters.
            for (i, _) in text.match_indices("sk-") {
                let tail: String = text[i + 3..]
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                    .collect();
                assert!(
                    tail.len() < 10,
                    "{} looks like it holds a key",
                    path.display()
                );
            }
            if let Some(key) = &real_key {
                assert!(
                    !text.contains(key.as_str()),
                    "{} holds the real key",
                    path.display()
                );
            }
        }
    }
    assert!(
        checked >= 5,
        "expected to scan the sample files, scanned {}",
        checked
    );
}
