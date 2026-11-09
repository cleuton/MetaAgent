// 0.1.4: helpers for the tests of [version] and [auth]: validator programs written for the tests, `metagente serve` as
// a child process with its messages in a file, and hand written calls. Never the product itself (FR-041).
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// A `metagente` child process; its standard error is a file, so the test can read the operator lines.
pub struct Proc {
    child: Child,
    pub log: PathBuf,
}

impl Proc {
    pub fn log(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn metagente(dir: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_metagente"));
    cmd.current_dir(dir)
        .env_remove("HTTPS_PROXY")
        .env_remove("HTTP_PROXY")
        .env_remove("https_proxy")
        .env_remove("http_proxy");
    cmd
}

/// Starts `metagente serve <args>` in `dir` and waits until `port` answers (or the process stops).
pub fn serve(dir: &Path, name: &str, args: &[&str], port: u16) -> Proc {
    serve_env(dir, name, args, port, &[])
}

pub fn serve_env(dir: &Path, name: &str, args: &[&str], port: u16, envs: &[(&str, &str)]) -> Proc {
    let log = dir.join(format!("{}.log", name));
    let file = std::fs::File::create(&log).unwrap();
    let mut child = metagente(dir)
        .arg("serve")
        .args(args)
        .envs(envs.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(file)
        .spawn()
        .unwrap();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if std::net::TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", port).parse().unwrap(),
            Duration::from_millis(200),
        )
        .is_ok()
        {
            return Proc { child, log };
        }
        if let Ok(Some(_)) = child.try_wait() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    panic!("metagente serve did not start: {}", text);
}

/// Runs `metagente <args>` to the end; returns (success, standard output, standard error).
pub fn run(dir: &Path, args: &[&str]) -> (bool, String, String) {
    let out = metagente(dir).args(args).output().unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

pub fn write(dir: &Path, name: &str, text: &str) {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, text).unwrap();
}

#[cfg(unix)]
pub fn executable(dir: &Path, name: &str, text: &str) {
    use std::os::unix::fs::PermissionsExt;
    write(dir, name, text);
    let path = dir.join(name);
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms).unwrap();
}

#[cfg(not(unix))]
pub fn executable(dir: &Path, name: &str, text: &str) {
    write(dir, name, text);
}

/// A validator that accepts the tokens listed in `valid-tokens.txt` and counts its runs in `runs.log`.
pub fn counting_validator(dir: &Path) {
    executable(
        dir,
        "validate.sh",
        "#!/bin/sh\nread -r token\necho run >> runs.log\necho \"$token\" >> seen.txt\ngrep -qxF \"$token\" valid-tokens.txt\n",
    );
}

pub fn runs(dir: &Path) -> usize {
    std::fs::read_to_string(dir.join("runs.log"))
        .map(|t| t.lines().count())
        .unwrap_or(0)
}

/// The agent used by most tests: it leaves `agent-ran.txt` behind when its handler runs.
pub const PLANNER: &str = "agent Planner\n  goal \"Plan\"\n  tool file\n  accepts plan topic\n  accepts ping\n  on plan\n    file.write path: \"agent-ran.txt\" text: topic\n    reply \"planned {topic}\"\n  on ping\n    reply \"pong\"\n";

pub fn agent_ran(dir: &Path) -> bool {
    dir.join("agent-ran.txt").exists()
}

/// A project folder: the agent, the toml, the validator and one accepted token.
pub fn project(toml: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "planner.ag", PLANNER);
    write(dir.path(), "metagente.toml", toml);
    write(dir.path(), "valid-tokens.txt", "good-token\nother-token\n");
    counting_validator(dir.path());
    dir
}

pub const BASIC_TOML: &str = "[version]\nminimum = \"0.1.4\"\n\n[auth]\nvalidator = \"./validate.sh\"\nprotect = [\"Planner.plan\"]\n";

pub struct Reply {
    pub status: u16,
    pub www_authenticate: Option<String>,
    pub body: serde_json::Value,
    pub text: String,
}

fn parse_reply(status: u16, www: Option<String>, text: String) -> Reply {
    Reply {
        status,
        www_authenticate: www,
        body: serde_json::from_str(&text).unwrap_or(serde_json::Value::Null),
        text,
    }
}

/// A hand written A2A `SendMessage` for the skill `skill` with one value `topic` (the body of tests/contract/a2a_wire.rs).
pub async fn send(
    port: u16,
    agent_path: &str,
    token: Option<&str>,
    skill: &str,
    topic: &str,
) -> Reply {
    let body = serde_json::json!({"jsonrpc": "2.0", "id": 7, "method": "SendMessage", "params": {"message": {
        "messageId": "m-1", "role": "ROLE_USER",
        "parts": [{"data": {"skill": skill, "arguments": arguments_of(topic)}, "mediaType": "application/json"}],
        "metadata": {"skill": skill}}}});
    post(port, agent_path, token, body).await
}

pub async fn post(port: u16, path: &str, token: Option<&str>, body: serde_json::Value) -> Reply {
    let mut request = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{}{}", port, path))
        .header("A2A-Version", "1.0")
        .header("Content-Type", "application/json");
    if let Some(t) = token {
        request = request.header("Authorization", format!("Bearer {}", t));
    }
    let response = request.body(body.to_string()).send().await.unwrap();
    let status = response.status().as_u16();
    let www = response
        .headers()
        .get("www-authenticate")
        .map(|v| v.to_str().unwrap().to_string());
    parse_reply(status, www, response.text().await.unwrap())
}

/// The text a completed task carries, or the error message.
pub fn answer(reply: &Reply) -> String {
    reply
        .body
        .pointer("/result/task/artifacts/0/parts/0/text")
        .or_else(|| {
            reply
                .body
                .pointer("/result/task/status/message/parts/0/text")
        })
        .or_else(|| reply.body.pointer("/error/message"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// An MCP `tools/call` over streamable http, with an optional bearer token. Returns the reply of the call only.
pub async fn mcp_call(port: u16, token: Option<&str>, tool: &str, topic: &str) -> Reply {
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/mcp", port);
    let headers = |b: reqwest::RequestBuilder| {
        let b = b
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream");
        match token {
            Some(t) => b.header("Authorization", format!("Bearer {}", t)),
            None => b,
        }
    };
    let init = serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "test", "version": "1"}}});
    let first = headers(client.post(&url))
        .body(init.to_string())
        .send()
        .await
        .unwrap();
    let session = first
        .headers()
        .get("mcp-session-id")
        .map(|v| v.to_str().unwrap().to_string());
    let _ = first.text().await;
    let mut notify = headers(client.post(&url)).body(
        serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string(),
    );
    if let Some(s) = &session {
        notify = notify.header("mcp-session-id", s);
    }
    let _ = notify.send().await.unwrap().text().await;
    let call = serde_json::json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": {"name": tool, "arguments": arguments_of(topic)}});
    let mut request = headers(client.post(&url)).body(call.to_string());
    if let Some(s) = &session {
        request = request.header("mcp-session-id", s);
    }
    let response = request.send().await.unwrap();
    let status = response.status().as_u16();
    let www = response
        .headers()
        .get("www-authenticate")
        .map(|v| v.to_str().unwrap().to_string());
    let text = response.text().await.unwrap();
    // an event stream answer carries the JSON on a `data:` line
    let json_text = text
        .lines()
        .filter_map(|l| l.strip_prefix("data:").map(|d| d.trim().to_string()))
        .find(|d| !d.is_empty())
        .unwrap_or_else(|| text.clone());
    parse_reply(status, www, json_text)
}

/// Like `post`, with the exact header lines given (to send two, or a strange one).
pub async fn post_raw(
    port: u16,
    path: &str,
    authorization: &[&str],
    body: serde_json::Value,
) -> Reply {
    let mut request = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{}{}", port, path))
        .header("A2A-Version", "1.0")
        .header("Content-Type", "application/json");
    for value in authorization {
        request = request.header("Authorization", *value);
    }
    let response = request.body(body.to_string()).send().await.unwrap();
    let status = response.status().as_u16();
    let www = response
        .headers()
        .get("www-authenticate")
        .map(|v| v.to_str().unwrap().to_string());
    parse_reply(status, www, response.text().await.unwrap())
}

pub fn plan_body(topic: &str) -> serde_json::Value {
    serde_json::json!({"jsonrpc": "2.0", "id": 7, "method": "SendMessage", "params": {"message": {
        "messageId": "m-1", "role": "ROLE_USER",
        "parts": [{"data": {"skill": "plan", "arguments": {"topic": topic}}, "mediaType": "application/json"}],
        "metadata": {"skill": "plan"}}}})
}

/// No values for an empty topic (a message such as `ping` takes none).
fn arguments_of(topic: &str) -> serde_json::Value {
    if topic.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::json!({"topic": topic})
    }
}

/// The tokens a validator saw, in order (`seen.txt` is written by `counting_validator`).
pub fn seen_tokens(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join("seen.txt"))
        .map(|t| t.lines().map(String::from).collect())
        .unwrap_or_default()
}

/// One stage of a chain of agents: its own folder, validator, toml and agent file.
pub fn stage(file: &str, source: &str, toml: &str, tokens: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), file, source);
    write(dir.path(), "metagente.toml", toml);
    write(dir.path(), "valid-tokens.txt", tokens);
    counting_validator(dir.path());
    dir
}

pub fn toml_for(protect: &str, forward: &str) -> String {
    format!(
        "[version]\nminimum = \"0.1.4\"\n\n[auth]\nvalidator = \"./validate.sh\"\nprotect = [{}]\nforward = [{}]\n",
        protect, forward
    )
}
