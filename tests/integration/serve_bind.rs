use std::io::Read;
use std::net::{SocketAddr, TcpStream, UdpSocket};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// The address of this computer on the network, if it has one besides loopback.
fn outside_ip() -> Option<std::net::IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?; // nothing is sent; this only picks a route
    let ip = socket.local_addr().ok()?.ip();
    if ip.is_loopback() || ip.is_unspecified() {
        None
    } else {
        Some(ip)
    }
}

fn wait_until_open(addr: SocketAddr) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

struct Serving(Child);
impl Drop for Serving {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn serve(dir: &std::path::Path, port: u16, public: bool) -> Serving {
    std::fs::write(
        dir.join("a.ag"),
        "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply \"ok\"\n",
    )
    .unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_metagente"));
    cmd.current_dir(dir)
        .args(["serve", "a.ag", "--a2a", &port.to_string()]);
    if public {
        cmd.arg("--public");
    }
    Serving(
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    )
}

#[test]
fn by_default_only_this_computer_can_connect() {
    let dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let server = serve(dir.path(), port, false);
    assert!(
        wait_until_open(SocketAddr::from(([127, 0, 0, 1], port))),
        "the server did not start"
    );
    if let Some(ip) = outside_ip() {
        let outside = SocketAddr::new(ip, port);
        assert!(
            TcpStream::connect_timeout(&outside, Duration::from_millis(500)).is_err(),
            "{} could connect without --public",
            outside
        );
    } else {
        eprintln!(
            "skipped the outside check: this computer has no network address besides loopback"
        );
    }
    drop(server);
}

#[test]
fn with_public_others_can_connect_and_a_warning_says_there_is_no_login() {
    let dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let mut server = serve(dir.path(), port, true);
    assert!(
        wait_until_open(SocketAddr::from(([127, 0, 0, 1], port))),
        "the server did not start"
    );
    if let Some(ip) = outside_ip() {
        assert!(
            TcpStream::connect_timeout(&SocketAddr::new(ip, port), Duration::from_millis(500))
                .is_ok(),
            "{} could not connect with --public",
            ip
        );
    } else {
        eprintln!(
            "skipped the outside check: this computer has no network address besides loopback"
        );
    }
    let mut stderr = server.0.stderr.take().unwrap();
    let _ = server.0.kill();
    let mut text = String::new();
    let _ = stderr.read_to_string(&mut text);
    assert!(text.contains("no login in this version"), "{}", text);
    assert!(text.contains("reached from other computers"), "{}", text);
}

#[test]
fn a_port_already_in_use_is_explained() {
    let dir = tempfile::tempdir().unwrap();
    let held = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = held.local_addr().unwrap().port();
    std::fs::write(
        dir.path().join("a.ag"),
        "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply \"ok\"\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_metagente"))
        .current_dir(dir.path())
        .args(["serve", "a.ag", "--a2a", &port.to_string()])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(err.contains("I could not listen for A2A"), "{}", err);
    assert!(err.contains("choose another port"), "{}", err);
}
