// 0.1.3: new file. The Python side of the interop tests: finding Python, starting the Python agent, making certificates.
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

const HOW_TO: &str = "python3 -m venv .venv && . .venv/bin/activate && pip install -r tests/interop/requirements.txt && export METAGENTE_INTEROP_PYTHON=\"$PWD/.venv/bin/python\"  (Windows: .venv\\Scripts\\activate, and METAGENTE_INTEROP_PYTHON=.venv\\Scripts\\python.exe)";

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The Python with the interop packages, or `None` (the test says it was skipped).
/// In CI (the `CI` variable is set) a missing Python is a failure, not a skip.
pub fn python_or_skip() -> Option<String> {
    let found = std::env::var("METAGENTE_INTEROP_PYTHON").ok().filter(|py| {
        Command::new(py)
            .args([
                "-c",
                "import a2a.client, a2a.server.routes, uvicorn, httpx, cryptography",
            ])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    });
    if found.is_none() {
        if std::env::var_os("CI").is_some() {
            panic!(
                "CI has no Python with the interop packages. Create it with: {}",
                HOW_TO
            );
        }
        eprintln!(
            "SKIPPED: no Python with the interop packages. Create it with: {}",
            HOW_TO
        );
    }
    found
}

pub fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// A child process that is stopped when the test ends.
pub struct Guard(pub Child);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Waits for the program to print a line starting with READY.
fn wait_ready(child: &mut Child, what: &str) {
    let stdout = child.stdout.take().expect("piped stdout");
    let mut lines = BufReader::new(stdout).lines();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        while let Some(Ok(line)) = lines.next() {
            if line.starts_with("READY") {
                let _ = tx.send(());
                break;
            }
        }
    });
    rx.recv_timeout(std::time::Duration::from_secs(30))
        .unwrap_or_else(|_| panic!("{} did not become ready in 30 seconds", what));
}

/// A self-signed certificate for 127.0.0.1 and localhost, made by the Python helper. Returns (cert, key).
pub fn make_cert(python: &str, dir: &Path) -> (PathBuf, PathBuf) {
    let out = Command::new(python)
        .arg(root().join("tests/interop/helpers/make_cert.py"))
        .arg(dir)
        .args(["127.0.0.1", "localhost"])
        .output()
        .expect("run make_cert.py");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    (dir.join("cert.pem"), dir.join("key.pem"))
}

/// The Python A2A agent ("Pyra"). Returns the guard and its address.
pub fn start_agent(python: &str, tls: Option<(&Path, &Path)>) -> (Guard, String) {
    let port = free_port();
    let mut cmd = Command::new(python);
    cmd.arg(root().join("tests/interop/python_agent/agent.py"))
        .args(["--port", &port.to_string()]);
    let scheme = if let Some((cert, key)) = tls {
        cmd.arg("--tls-cert").arg(cert).arg("--tls-key").arg(key);
        "https"
    } else {
        "http"
    };
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start the Python agent");
    wait_ready(&mut child, "the Python agent");
    (Guard(child), format!("{}://127.0.0.1:{}", scheme, port))
}

/// A TLS front (fixture) in front of a plain Metagente A2A port. Returns the guard and the https address.
pub fn start_front(python: &str, target_port: u16, cert: &Path, key: &Path) -> (Guard, String) {
    let port = free_port();
    let mut child = Command::new(python)
        .arg(root().join("tests/interop/helpers/tls_front.py"))
        .args([
            "--listen",
            &port.to_string(),
            "--target",
            &target_port.to_string(),
        ])
        .arg("--cert")
        .arg(cert)
        .arg("--key")
        .arg(key)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start the TLS front");
    wait_ready(&mut child, "the TLS front");
    (Guard(child), format!("https://127.0.0.1:{}", port))
}

/// Runs the Python A2A client and returns what it printed as JSON, or its error text.
pub fn client(python: &str, base: &str, extra: &[&str]) -> Result<serde_json::Value, String> {
    let out = Command::new(python)
        .arg(root().join("tests/interop/a2a_sdk_client.py"))
        .arg(base)
        .args(extra)
        .output()
        .expect("run the Python client");
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    serde_json::from_slice(&out.stdout)
        .map_err(|e| format!("{}: {}", e, String::from_utf8_lossy(&out.stdout)))
}
