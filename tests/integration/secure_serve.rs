// 0.1.3: new file. `metagente serve` answers over https itself, with no helper in front of it (User Story 3).
use crate::common::a2a::WEATHER;
use crate::common::self_signed::{ServeProc, free_port, is_http_answer, pair_in, plain_request};
use crate::common::*;
use std::path::Path;
use std::process::Command;

const TWO_AGENTS: &str = "agent Weather\n  goal \"Answer questions about the weather\"\n  accepts ask city\n  on ask\n    reply \"sunny in {city}\"\nagent Other\n  goal \"Another one\"\n  accepts hi\n  on hi\n    reply \"hello\"\n";

fn serve_args(file: &str, a2a: Option<u16>, mcp: Option<u16>) -> Vec<String> {
    let mut args = vec![file.to_string()];
    if let Some(p) = a2a {
        args.push("--a2a".into());
        args.push(p.to_string());
    }
    if let Some(p) = mcp {
        args.push("--mcp".into());
        args.push(p.to_string());
    }
    args
}

/// A folder with the served agents, a certificate and key, and a metagente.toml.
fn served_folder(
    source: &str,
    secure: &[&str],
    with_pair: bool,
) -> (tempfile::TempDir, Option<crate::common::self_signed::Files>) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("weather.ag"), source).unwrap();
    let files = with_pair.then(|| pair_in(dir.path()));
    let mut toml = String::new();
    if !secure.is_empty() {
        toml.push_str(&format!(
            "[secure]\nagents = [{}]\n",
            secure
                .iter()
                .map(|n| format!("\"{}\"", n))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if with_pair {
        toml.push_str("[serve]\ntls_cert = \"cert.pem\"\ntls_key = \"key.pem\"\n");
    }
    std::fs::write(dir.path().join("metagente.toml"), toml).unwrap();
    (dir, files)
}

fn trusting(ca_pem: &str) -> reqwest::Client {
    reqwest::Client::builder()
        .add_root_certificate(reqwest::Certificate::from_pem(ca_pem.as_bytes()).unwrap())
        .build()
        .unwrap()
}

fn base(dir: &Path) -> std::path::PathBuf {
    dir.to_path_buf()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_card_and_the_task_calls_work_over_tls() {
    let (dir, files) = served_folder(WEATHER, &["Weather"], true);
    let files = files.unwrap();
    let port = free_port();
    let _serve = ServeProc::start(
        &base(dir.path()),
        &serve_args("weather.ag", Some(port), None),
        1,
    );
    let client = trusting(&files.ca_pem);
    let url = format!("https://localhost:{}", port);
    let card: serde_json::Value = client
        .get(format!("{}/.well-known/agent-card.json", url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        card["supportedInterfaces"][0]["url"]
            .as_str()
            .unwrap()
            .starts_with("https://"),
        "{}",
        card
    );
    let body = serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "SendMessage", "params": crate::common::a2a::user_text("ask city=Lisbon")});
    let reply: serde_json::Value = client
        .post(format!("{}/a2a", url))
        .header("A2A-Version", "1.0")
        .body(body.to_string())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        reply["result"]["task"]["status"]["state"], "TASK_STATE_COMPLETED",
        "{}",
        reply
    );
    let task_id = reply["result"]["task"]["id"].as_str().unwrap().to_string();
    let got: serde_json::Value = client
        .post(format!("{}/a2a", url))
        .header("A2A-Version", "1.0")
        .body(serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "GetTask", "params": {"id": task_id}}).to_string())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        got["result"]["status"]["state"], "TASK_STATE_COMPLETED",
        "{}",
        got
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn metagente_calls_metagente_over_https_for_a2a_and_for_a_served_mcp_tool() {
    let (dir, files) = served_folder(WEATHER, &["Weather"], true);
    let files = files.unwrap();
    let (a2a_port, mcp_port) = (free_port(), free_port());
    let _serve = ServeProc::start(
        &base(dir.path()),
        &serve_args("weather.ag", Some(a2a_port), Some(mcp_port)),
        2,
    );
    let caller = tempfile::tempdir().unwrap();
    let ca = caller.path().join("ca.pem");
    std::fs::write(&ca, &files.ca_pem).unwrap();
    // only the toml options: the caller trusts the self-signed certificate through ca_file
    let rt = runtime_in(caller.path(), None, |c| {
        c.network.ca_file = Some(ca.clone());
        c.secure.agents = vec!["Bob".to_string()];
        c.secure.tools = vec!["kb".to_string()];
    });
    let trip = project_file("examples/remote_weather.ag").replace(
        "http://127.0.0.1:8080",
        &format!("https://localhost:{}", a2a_port),
    );
    let reply = run_source(&rt, &trip, "plan", &[("city", "Lisbon")])
        .await
        .unwrap();
    assert_eq!(reply.to_display(), "Bob says: sunny in Lisbon");
    let mcp = format!(
        "agent Caller\n  goal \"c\"\n  tool kb from mcp \"https://localhost:{}/mcp\"\n  accepts go city\n  on go\n    r = kb.Weather.ask city: city\n    reply r\n",
        mcp_port
    );
    let reply = run_source(&rt, &mcp, "go", &[("city", "Lisbon")])
        .await
        .unwrap();
    assert_eq!(reply.to_display(), "sunny in Lisbon");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_secure_agent_has_no_plain_http_listener_on_any_port() {
    let (dir, _files) = served_folder(WEATHER, &["Weather"], true);
    let (a2a_port, mcp_port) = (free_port(), free_port());
    let _serve = ServeProc::start(
        &base(dir.path()),
        &serve_args("weather.ag", Some(a2a_port), Some(mcp_port)),
        2,
    );
    for port in [a2a_port, mcp_port] {
        let got = plain_request(port).await;
        assert!(
            !is_http_answer(&got),
            "port {} answered plain http: {}",
            port,
            String::from_utf8_lossy(&got)
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_url_in_the_card_of_a_secure_agent_is_https_whatever_the_headers_say() {
    let (dir, files) = served_folder(WEATHER, &["Weather"], true);
    let files = files.unwrap();
    let port = free_port();
    let _serve = ServeProc::start(
        &base(dir.path()),
        &serve_args("weather.ag", Some(port), None),
        1,
    );
    let client = trusting(&files.ca_pem);
    for path in [
        "/.well-known/agent-card.json",
        "/agents/Weather/.well-known/agent-card.json",
    ] {
        let card: serde_json::Value = client
            .get(format!("https://localhost:{}{}", port, path))
            .header("X-Forwarded-Proto", "http")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let mut urls = Vec::new();
        collect_urls(&card, &mut urls);
        assert!(!urls.is_empty(), "{}", card);
        for u in urls {
            assert!(u.starts_with("https://"), "{} in {}", u, card);
        }
    }
}

fn collect_urls(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::Object(m) => {
            for (k, x) in m {
                if let (true, Some(s)) = (k.to_ascii_lowercase().contains("url"), x.as_str()) {
                    out.push(s.to_string());
                }
                collect_urls(x, out);
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| collect_urls(x, out)),
        _ => {}
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_secure_agent_makes_the_whole_process_https_only_and_says_so() {
    let (dir, files) = served_folder(TWO_AGENTS, &["Weather"], true);
    let files = files.unwrap();
    let port = free_port();
    let serve = ServeProc::start(
        &base(dir.path()),
        &serve_args("weather.ag", Some(port), None),
        1,
    );
    assert!(
        serve.said().contains(
            "Weather is declared secure, so every agent in this process is served over https only"
        ),
        "{}",
        serve.said()
    );
    assert!(!is_http_answer(&plain_request(port).await));
    let client = trusting(&files.ca_pem);
    let other = client
        .get(format!(
            "https://localhost:{}/agents/Other/.well-known/agent-card.json",
            port
        ))
        .send()
        .await
        .unwrap();
    assert!(other.status().is_success());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_certificate_makes_even_an_agent_that_is_not_secure_answer_https_only() {
    let (dir, files) = served_folder(WEATHER, &[], true);
    let files = files.unwrap();
    let port = free_port();
    let _serve = ServeProc::start(
        &base(dir.path()),
        &serve_args("weather.ag", Some(port), None),
        1,
    );
    assert!(!is_http_answer(&plain_request(port).await));
    let ok = trusting(&files.ca_pem)
        .get(format!(
            "https://localhost:{}/.well-known/agent-card.json",
            port
        ))
        .send()
        .await
        .unwrap();
    assert!(ok.status().is_success());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_a_certificate_and_without_a_secure_agent_serve_answers_plain_http_as_in_0_1_2() {
    let (dir, _) = served_folder(WEATHER, &[], false);
    let port = free_port();
    let _serve = ServeProc::start(
        &base(dir.path()),
        &serve_args("weather.ag", Some(port), None),
        1,
    );
    let got = plain_request(port).await;
    assert!(is_http_answer(&got), "{}", String::from_utf8_lossy(&got));
    assert!(
        String::from_utf8_lossy(&got).contains("http://127.0.0.1"),
        "plain card"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_flags_serve_over_https_without_any_serve_section() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("weather.ag"), WEATHER).unwrap();
    let files = pair_in(dir.path());
    let port = free_port();
    let mut args = serve_args("weather.ag", Some(port), None);
    args.extend([
        "--tls-cert".to_string(),
        files.cert.display().to_string(),
        "--tls-key".to_string(),
        files.key.display().to_string(),
    ]);
    let _serve = ServeProc::start(dir.path(), &args, 1);
    let ok = trusting(&files.ca_pem)
        .get(format!(
            "https://localhost:{}/.well-known/agent-card.json",
            port
        ))
        .send()
        .await
        .unwrap();
    assert!(ok.status().is_success());
}

// 0.1.3: an HTTP/2 client (curl, a browser) sends no Host header, only the authority of the request. The card must
// still name the host the caller used, not the address the server listens on.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_card_names_the_host_the_caller_used_also_for_http2_clients() {
    if Command::new("curl").arg("--version").output().is_err() {
        eprintln!("SKIPPED: curl is not installed");
        return;
    }
    // a real self-signed certificate that curl (OpenSSL) trusts directly
    let (dir, _) = served_folder(WEATHER, &["Weather"], false);
    std::fs::write(
        dir.path().join("metagente.toml"),
        "[secure]\nagents = [\"Weather\"]\n[serve]\ntls_cert = \"cert.pem\"\ntls_key = \"key.pem\"\n",
    )
    .unwrap();
    let files = crate::common::self_signed::self_signed_pair_in(dir.path());
    let port = free_port();
    let _serve = ServeProc::start(
        &base(dir.path()),
        &serve_args("weather.ag", Some(port), None),
        1,
    );
    let ca = dir.path().join("ca.pem");
    std::fs::write(&ca, &files.ca_pem).unwrap();
    let out = tokio::task::spawn_blocking(move || {
        Command::new("curl")
            .args(["-sS", "--http2", "--cacert"])
            .arg(&ca)
            .arg(format!(
                "https://localhost:{}/.well-known/agent-card.json",
                port
            ))
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    let card: serde_json::Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|_| panic!("curl said: {}", String::from_utf8_lossy(&out.stderr)));
    assert_eq!(
        card["supportedInterfaces"][0]["url"],
        format!("https://localhost:{}/a2a", port),
        "{}",
        card
    );
}
