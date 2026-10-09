// 0.1.3: new file. A small test proxy: HTTP CONNECT tunnels and plain forwarding, optional Basic login.
// It records the host of every request and never records a header, so a password cannot end up in its log.
use base64::Engine as _;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Clone, Default)]
pub struct ProxyLog(pub Arc<Mutex<Vec<String>>>);

impl ProxyLog {
    pub fn lines(&self) -> Vec<String> {
        self.0.lock().map(|l| l.clone()).unwrap_or_default()
    }
}

pub struct TestProxy {
    pub port: u16,
    pub log: ProxyLog,
    pub scheme: &'static str,
}

impl TestProxy {
    pub fn url(&self) -> String {
        format!("{}://127.0.0.1:{}", self.scheme, self.port)
    }
}

/// What the proxy does with every request.
#[derive(Clone)]
pub enum Mode {
    /// Needs this user and password (Basic), otherwise answers 407.
    Login(String, String),
    /// Needs no login.
    Open,
    /// Answers 407 and offers another scheme.
    OfferScheme(&'static str),
    /// Answers this status to every CONNECT.
    Status(u16),
    /// Accepts the connection and never answers.
    Silent,
}

/// A proxy that is itself reached over https, with this certificate.
pub async fn start_tls(mode: Mode, cert: &super::tls_server::TestCert) -> TestProxy {
    let acceptor = super::tls_server::acceptor(cert);
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind proxy");
    let port = listener.local_addr().expect("addr").port();
    let log = ProxyLog::default();
    let shared = log.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                continue;
            };
            let (mode, log, acceptor) = (mode.clone(), shared.clone(), acceptor.clone());
            tokio::spawn(async move {
                if let Ok(tls) = acceptor.accept(stream).await {
                    let _ = handle(tls, mode, log).await;
                }
            });
        }
    });
    TestProxy {
        port,
        log,
        scheme: "https",
    }
}

pub async fn start(mode: Mode) -> TestProxy {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind proxy");
    let port = listener.local_addr().expect("addr").port();
    let log = ProxyLog::default();
    let shared = log.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                continue;
            };
            let mode = mode.clone();
            let log = shared.clone();
            tokio::spawn(async move {
                let _ = handle(stream, mode, log).await;
            });
        }
    });
    TestProxy {
        port,
        log,
        scheme: "http",
    }
}

async fn read_head<S: AsyncRead + Unpin>(stream: &mut S) -> std::io::Result<(String, Vec<u8>)> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    while !buf.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).await? == 0 {
            break;
        }
        buf.push(byte[0]);
    }
    Ok((String::from_utf8_lossy(&buf).to_string(), buf))
}

fn login_ok(head: &str, user: &str, password: &str) -> bool {
    let want = base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", user, password));
    head.lines().any(|l| {
        l.to_ascii_lowercase()
            .starts_with("proxy-authorization: basic ")
            && l.split_whitespace().last() == Some(want.as_str())
    })
}

async fn handle<S: AsyncRead + AsyncWrite + Unpin>(
    mut client: S,
    mode: Mode,
    log: ProxyLog,
) -> std::io::Result<()> {
    let (head, raw) = read_head(&mut client).await?;
    let first = head.lines().next().unwrap_or("").to_string();
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("").to_string();
    let note = |what: &str| {
        if let Ok(mut l) = log.0.lock() {
            l.push(what.to_string());
        }
    };
    if let Mode::Silent = &mode {
        note(&format!("{} {} silent", method, target));
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        return Ok(());
    }
    let allowed = match &mode {
        Mode::Login(u, p) => login_ok(&head, u, p),
        Mode::Open => true,
        Mode::OfferScheme(_) => false,
        Mode::Status(_) | Mode::Silent => true,
    };
    if !allowed {
        note(&format!(
            "{} {} 407",
            method,
            target.split('/').nth(2).unwrap_or(&target)
        ));
        let scheme = match &mode {
            Mode::OfferScheme(s) => *s,
            _ => "Basic realm=\"test\"",
        };
        client
            .write_all(
                format!(
                    "HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    scheme
                )
                .as_bytes(),
            )
            .await?;
        return Ok(());
    }
    if let Mode::Status(code) = &mode {
        note(&format!("{} {} {}", method, target, code));
        client
            .write_all(
                format!(
                    "HTTP/1.1 {} No\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    code
                )
                .as_bytes(),
            )
            .await?;
        return Ok(());
    }
    if method == "CONNECT" {
        let Ok(mut upstream) = TcpStream::connect(&target).await else {
            note(&format!("CONNECT {} fail", target));
            client
                .write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")
                .await?;
            return Ok(());
        };
        note(&format!("CONNECT {} ok", target));
        client
            .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
            .await?;
        let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
        return Ok(());
    }
    // Plain forwarding of `GET http://host:port/path HTTP/1.1`.
    let rest = target.trim_start_matches("http://");
    let (host, path) = rest
        .split_once('/')
        .map(|(h, p)| (h, format!("/{}", p)))
        .unwrap_or((rest, "/".to_string()));
    let Ok(mut upstream) = TcpStream::connect(host).await else {
        note(&format!("{} {} fail", method, host));
        client
            .write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")
            .await?;
        return Ok(());
    };
    note(&format!("{} {} ok", method, host));
    let text = String::from_utf8_lossy(&raw).to_string();
    let rewritten = text.replacen(&first, &format!("{} {} HTTP/1.1", method, path), 1);
    let rewritten: String = rewritten
        .lines()
        .filter(|l| !l.to_ascii_lowercase().starts_with("proxy-authorization"))
        .collect::<Vec<_>>()
        .join("\r\n");
    upstream
        .write_all(format!("{}\r\n\r\n", rewritten.trim_end()).as_bytes())
        .await?;
    let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
    Ok(())
}
