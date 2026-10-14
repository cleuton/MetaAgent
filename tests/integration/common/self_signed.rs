// 0.1.3: new file. Certificates and keys written to files for `metagente serve`, and the serve program as a child
// process of the test. Everything is made on the spot; nothing is committed.
use super::tls_server::{self, TestCert};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The files of a certificate and key, and the authority that signed the certificate (PEM, for the client side).
pub struct Files {
    pub cert: PathBuf,
    pub key: PathBuf,
    pub ca_pem: String,
}

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

/// A certificate valid for `localhost` and `127.0.0.1`, signed by a throwaway authority.
pub fn pair_in(dir: &Path) -> Files {
    let (ca_pem, cert) = tls_server::signed_by_new_authority(&["localhost", "127.0.0.1"]);
    Files {
        cert: write(dir, "cert.pem", &cert.cert_pem),
        key: write(dir, "key.pem", &cert.key_pem),
        ca_pem,
    }
}

/// A real self-signed certificate (it signed itself) for `localhost` and `127.0.0.1`. `ca_pem` is the certificate itself.
pub fn self_signed_pair_in(dir: &Path) -> Files {
    let cert = tls_server::self_signed(&["localhost", "127.0.0.1"]);
    Files {
        cert: write(dir, "cert.pem", &cert.cert_pem),
        key: write(dir, "key.pem", &cert.key_pem),
        ca_pem: cert.cert_pem.clone(),
    }
}

/// The same, but the certificate ran out in the year 2000.
pub fn expired_pair_in(dir: &Path) -> Files {
    let (ca_pem, cert) =
        tls_server::signed_by_new_authority_with(&["localhost", "127.0.0.1"], true);
    Files {
        cert: write(dir, "cert.pem", &cert.cert_pem),
        key: write(dir, "key.pem", &cert.key_pem),
        ca_pem,
    }
}

/// A self-signed certificate that expires in `days` days.
pub fn expiring_pair_in(dir: &Path, days: i64) -> Files {
    use rcgen::{CertificateParams, KeyPair};
    let key = KeyPair::generate().unwrap();
    let mut params =
        CertificateParams::new(vec!["localhost".to_string(), "127.0.0.1".to_string()]).unwrap();
    let day = |offset: i64| {
        let d = chrono::Utc::now() + chrono::Duration::days(offset);
        use chrono::Datelike;
        rcgen::date_time_ymd(d.year(), d.month() as u8, d.day() as u8)
    };
    params.not_before = day(-1);
    params.not_after = day(days);
    let cert = params.self_signed(&key).unwrap();
    Files {
        cert: write(dir, "cert.pem", &cert.pem()),
        key: write(dir, "key.pem", &key.serialize_pem()),
        ca_pem: cert.pem(),
    }
}

/// A certificate with the key of a different certificate.
pub fn mismatched_pair_in(dir: &Path) -> Files {
    let a = tls_server::self_signed(&["localhost"]);
    let b: TestCert = tls_server::self_signed(&["localhost"]);
    Files {
        cert: write(dir, "cert.pem", &a.cert_pem),
        key: write(dir, "key.pem", &b.key_pem),
        ca_pem: a.cert_pem,
    }
}

pub fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// `metagente serve ...` as a child of the test, stopped when the test ends.
pub struct ServeProc {
    child: Child,
    lines: Arc<Mutex<Vec<String>>>,
}

impl ServeProc {
    /// Starts it in `dir` and waits until it has announced `listeners` listeners (lines starting `A2A:` or `MCP:`).
    /// Panics with what it said if it stops first.
    pub fn start(dir: &Path, args: &[String], listeners: usize) -> ServeProc {
        let mut child = Command::new(env!("CARGO_BIN_EXE_metagente"))
            .current_dir(dir)
            .arg("serve")
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start metagente serve");
        let lines: Arc<Mutex<Vec<String>>> = Default::default();
        let sink = lines.clone();
        let stderr = child.stderr.take().unwrap();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                sink.lock().unwrap().push(line);
            }
        });
        let mut proc = ServeProc { child, lines };
        let started = Instant::now();
        loop {
            let ready = proc
                .lines
                .lock()
                .unwrap()
                .iter()
                .filter(|l| l.starts_with("A2A:") || l.starts_with("MCP:"))
                .count();
            if ready >= listeners {
                return proc;
            }
            if let Ok(Some(status)) = proc.child.try_wait() {
                panic!(
                    "serve stopped ({}) before it was ready:\n{}",
                    status,
                    proc.said()
                );
            }
            if started.elapsed() > Duration::from_secs(30) {
                panic!("serve was not ready in 30 seconds:\n{}", proc.said());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Everything it printed on the error output so far.
    pub fn said(&self) -> String {
        self.lines.lock().unwrap().join("\n")
    }
}

impl Drop for ServeProc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Runs `metagente serve ...` expecting it to stop by itself (a refusal). Panics if it keeps running.
pub fn serve_refused(dir: &Path, args: &[String]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_metagente"))
        .current_dir(dir)
        .arg("serve")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start metagente serve");
    let started = Instant::now();
    loop {
        if let Ok(Some(_)) = child.try_wait() {
            return child.wait_with_output().unwrap();
        }
        if started.elapsed() > Duration::from_secs(20) {
            let _ = child.kill();
            let out = child.wait_with_output().unwrap();
            panic!(
                "serve kept running, it should have refused:\n{}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// What a plain http request to this port gets back (empty when the server just hangs up).
pub async fn plain_request(port: u16) -> Vec<u8> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let Ok(mut stream) = tokio::net::TcpStream::connect(("127.0.0.1", port)).await else {
        return Vec::new();
    };
    let _ = stream
        .write_all(b"GET /.well-known/agent-card.json HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .await;
    let mut got = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(3), stream.read_to_end(&mut got)).await;
    got
}

pub fn is_http_answer(bytes: &[u8]) -> bool {
    bytes.starts_with(b"HTTP/")
}

/// Splits a command line the way a shell would, enough for double quotes.
fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut has = false;
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                has = true;
            }
            c if c.is_whitespace() && !quoted => {
                if has {
                    out.push(std::mem::take(&mut current));
                    has = false;
                }
            }
            c => {
                current.push(c);
                has = true;
            }
        }
    }
    if has {
        out.push(current);
    }
    out
}

/// Makes `certs/dev-cert.pem` and `certs/dev-key.pem` in `dir` with the very `openssl` command that the README of the
/// sample shows (so the README is proved to work). Without `openssl` on the computer, an equivalent pair is made.
pub fn sample_certificate(dir: &Path, readme: &str) -> Files {
    std::fs::create_dir_all(dir.join("certs")).unwrap();
    let command = readme
        .lines()
        .find(|l| l.starts_with("openssl req -x509"))
        .expect("the README shows the openssl command");
    let args = words(command);
    let made = Command::new(&args[0])
        .args(&args[1..])
        .current_dir(dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !made {
        // no openssl here: a certificate that is not an authority, valid for the same names
        use rcgen::{CertificateParams, IsCa, KeyPair};
        let key = KeyPair::generate().unwrap();
        let mut params =
            CertificateParams::new(vec!["localhost".to_string(), "127.0.0.1".to_string()]).unwrap();
        params.is_ca = IsCa::NoCa;
        let cert = params.self_signed(&key).unwrap();
        std::fs::write(dir.join("certs/dev-cert.pem"), cert.pem()).unwrap();
        std::fs::write(dir.join("certs/dev-key.pem"), key.serialize_pem()).unwrap();
    }
    let cert = dir.join("certs/dev-cert.pem");
    let ca_pem = std::fs::read_to_string(&cert).unwrap();
    Files {
        cert,
        key: dir.join("certs/dev-key.pem"),
        ca_pem,
    }
}
