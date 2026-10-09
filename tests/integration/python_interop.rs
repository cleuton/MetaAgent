// 0.1.3: new file. A Python agent and a Metagente agent call each other over A2A, in both directions:
// plain http, https with a self-signed certificate, and through an authenticated proxy.
// The Python side is the official a2a-sdk. Without Python the tests say they were skipped (in CI they fail).
use crate::common::a2a::{self, WEATHER};
use crate::common::proxy::{self, Mode};
use crate::common::python::*;
use crate::common::*;

const RELAY: &str = "agent Relay\n  goal \"Ask the Python agent and add my own words\"\n  remote Pyra at \"URL\"\n  accepts relay text\n  on relay\n    a = Pyra.echo text: text\n    b = Pyra.shout text: text\n    reply \"metagente got {text}; python said: {a} and {b}\"\n";

const CALLER: &str = "agent Caller\n  goal \"Call the Python agent\"\n  remote Pyra at \"URL\"\n  accepts hello text\n  accepts nothing\n  on hello\n    a = Pyra.echo text: text\n    b = Pyra.shout text: text\n    reply \"{a} | {b}\"\n  on nothing\n    reply Pyra.dance text: \"x\"\n";

fn caller(url: &str) -> String {
    CALLER.replace("URL", url)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn metagente_calls_a_python_agent_over_plain_http() {
    let Some(py) = python_or_skip() else { return };
    let (_agent, url) = start_agent(&py, None);
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let reply = run_source(&rt, &caller(&url), "hello", &[("text", "hello")])
        .await
        .unwrap();
    assert_eq!(reply.to_display(), "echo: hello | HELLO!");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_skill_the_python_agent_does_not_offer_lists_the_ones_it_does() {
    let Some(py) = python_or_skip() else { return };
    let (_agent, url) = start_agent(&py, None);
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let err = run_source(&rt, &caller(&url), "nothing", &[])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("does not handle `dance`"), "{}", err);
    assert!(err.contains("echo") && err.contains("shout"), "{}", err);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_python_client_calls_a_metagente_agent_over_plain_http() {
    let Some(py) = python_or_skip() else { return };
    let dir = tempfile::tempdir().unwrap();
    let server = a2a::start(dir.path(), WEATHER, None).await;
    let seen = client(&py, &server.base, &[]).unwrap();
    assert_eq!(seen["card_name"], "Weather");
    assert_eq!(seen["state"], "TASK_STATE_COMPLETED");
    assert_eq!(seen["text"], "sunny in Lisbon");
    // a message the agent does not accept is the protocol's "not supported" error, not a crash
    let unsupported = seen["unsupported_error"].as_str().unwrap_or("");
    assert!(unsupported.contains("dance"), "{}", unsupported);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn python_to_metagente_to_python_round_trip() {
    let Some(py) = python_or_skip() else { return };
    let (_agent, url) = start_agent(&py, None);
    let dir = tempfile::tempdir().unwrap();
    let server = a2a::start(dir.path(), &RELAY.replace("URL", &url), None).await;
    let seen = client(&py, &server.base, &["--message", "relay text=hello"]).unwrap();
    assert_eq!(seen["state"], "TASK_STATE_COMPLETED");
    let text = seen["text"].as_str().unwrap();
    assert_eq!(
        text,
        "metagente got hello; python said: echo: hello and HELLO!"
    );
}

// ---------- https with a self-signed certificate ----------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn metagente_calls_a_self_signed_python_agent_only_when_allowed() {
    let Some(py) = python_or_skip() else { return };
    let certs = tempfile::tempdir().unwrap();
    let (cert, key) = make_cert(&py, certs.path());
    let (_agent, url) = start_agent(&py, Some((&cert, &key)));
    let dir = tempfile::tempdir().unwrap();

    let strict = runtime_in(dir.path(), None, |_| {});
    let err = run_source(&strict, &caller(&url), "hello", &[("text", "hi")])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("could not trust the certificate"), "{}", err);
    assert!(
        err.contains("allow_self_signed") && err.contains("ca_file"),
        "{}",
        err
    );

    let lax = runtime_in(dir.path(), None, |c| c.network.allow_self_signed = true);
    let reply = run_source(&lax, &caller(&url), "hello", &[("text", "hi")])
        .await
        .unwrap();
    assert_eq!(reply.to_display(), "echo: hi | HI!");

    // trusting its certificate with ca_file works with checks still on
    let trusting = runtime_in(dir.path(), None, |c| c.network.ca_file = Some(cert.clone()));
    let reply = run_source(&trusting, &caller(&url), "hello", &[("text", "hi")])
        .await
        .unwrap();
    assert_eq!(reply.to_display(), "echo: hi | HI!");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_python_client_calls_a_metagente_agent_over_https_with_a_self_signed_certificate() {
    let Some(py) = python_or_skip() else { return };
    let certs = tempfile::tempdir().unwrap();
    let (cert, key) = make_cert(&py, certs.path());
    let dir = tempfile::tempdir().unwrap();
    let server = a2a::start(dir.path(), WEATHER, None).await;
    let target: u16 = server.base.rsplit(':').next().unwrap().parse().unwrap();
    let (_front, https) = start_front(&py, target, &cert, &key);

    // by default the Python client refuses the certificate too
    assert!(client(&py, &https, &[]).is_err());
    let seen = client(&py, &https, &["--insecure"]).unwrap();
    assert_eq!(seen["text"], "sunny in Lisbon");
    assert!(
        seen["endpoint"].as_str().unwrap().starts_with("https://"),
        "{}",
        seen["endpoint"]
    );
    let trusted = client(&py, &https, &["--ca", cert.to_str().unwrap()]).unwrap();
    assert_eq!(trusted["text"], "sunny in Lisbon");
}

// ---------- through an authenticated proxy ----------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn metagente_calls_python_through_an_authenticated_proxy_with_one_tunnel() {
    let Some(py) = python_or_skip() else { return };
    let certs = tempfile::tempdir().unwrap();
    let (cert, key) = make_cert(&py, certs.path());
    let (_agent, url) = start_agent(&py, Some((&cert, &key)));
    let p = proxy::start(Mode::Login("ann".into(), "p@ss:/%word".into())).await;
    let dir = tempfile::tempdir().unwrap();
    let env: std::collections::HashMap<String, String> =
        [("PROXY_USER", "ann"), ("PROXY_PASSWORD", "p@ss:/%word")]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
    let mut config = metagente::runtime::config::Config {
        root: dir.path().to_path_buf(),
        ..Default::default()
    };
    config.network.proxy.url = Some(p.url());
    config.network.proxy.username_env = Some("PROXY_USER".into());
    config.network.proxy.password_env = Some("PROXY_PASSWORD".into());
    config.network.ca_file = Some(cert.clone());
    let net = std::sync::Arc::new(metagente::runtime::net::ClientSet::with_env(
        config.network.clone(),
        std::time::Duration::from_secs(5),
        move |n| env.get(n).cloned(),
    ));
    let rt = metagente::runtime::Runtime::with_net(config, None, net);
    let reply = run_source(&rt, &caller(&url), "hello", &[("text", "hi")])
        .await
        .unwrap();
    assert_eq!(reply.to_display(), "echo: hi | HI!");
    let host = url.trim_start_matches("https://");
    assert_eq!(p.log.lines(), vec![format!("CONNECT {} ok", host)]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_python_client_reaches_a_metagente_agent_through_a_proxy() {
    let Some(py) = python_or_skip() else { return };
    let dir = tempfile::tempdir().unwrap();
    let server = a2a::start(dir.path(), WEATHER, None).await;
    let p = proxy::start(Mode::Login("ann".into(), "secret".into())).await;
    let with_login = p.url().replace("http://", "http://ann:secret@");
    let seen = client(&py, &server.base, &["--proxy", &with_login]).unwrap();
    assert_eq!(seen["text"], "sunny in Lisbon");
    assert!(!p.log.lines().is_empty());
    assert!(
        p.log.lines().iter().all(|l| l.ends_with(" ok")),
        "{:?}",
        p.log.lines()
    );
    let wrong = p.url().replace("http://", "http://ann:nope@");
    assert!(client(&py, &server.base, &["--proxy", &wrong]).is_err());
}

// ---------- the beginner example ----------

#[test]
fn the_beginner_example_has_its_files_and_both_agents_check_clean() {
    let folder = root().join("examples/python_interop");
    for file in [
        "caller.ag",
        "greeter.ag",
        "python_agent.py",
        "python_client.py",
        "requirements.txt",
        "README.md",
    ] {
        assert!(
            folder.join(file).is_file(),
            "examples/python_interop/{} is missing",
            file
        );
    }
    for agent in ["caller.ag", "greeter.ag"] {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_metagente"))
            .current_dir(&folder)
            .args(["check", agent])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}: {}",
            agent,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let readme = std::fs::read_to_string(folder.join("README.md")).unwrap();
    for needle in [
        "python3 -m venv .venv",
        "pip install -r",
        "metagente run caller.ag",
        "metagente serve greeter.ag",
        "python_client.py",
    ] {
        assert!(
            readme.contains(needle),
            "the example README lacks `{}`",
            needle
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_beginner_example_works_in_both_directions() {
    let Some(py) = python_or_skip() else { return };
    let folder = root().join("examples/python_interop");
    // Metagente -> Python, with the example's own launcher and caller.ag (only the port differs)
    let port = free_port();
    let mut agent = std::process::Command::new(&py)
        .arg(folder.join("python_agent.py"))
        .args(["--port", &port.to_string()])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    {
        use std::io::BufRead;
        let mut lines = std::io::BufReader::new(agent.stdout.take().unwrap()).lines();
        assert!(lines.next().unwrap().unwrap().starts_with("READY"));
        std::mem::forget(lines);
    }
    let _guard = Guard(agent);
    let dir = tempfile::tempdir().unwrap();
    let source = std::fs::read_to_string(folder.join("caller.ag"))
        .unwrap()
        .replace("9100", &port.to_string());
    std::fs::write(dir.path().join("caller.ag"), source).unwrap();
    let out = tokio::process::Command::new(env!("CARGO_BIN_EXE_metagente"))
        .current_dir(dir.path())
        .args(["run", "caller.ag", "shout_it", "text=hello"])
        .output()
        .await
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "Pyra says: HELLO!"
    );

    // Python -> Metagente, with the example's own greeter.ag and client launcher
    let served = tempfile::tempdir().unwrap();
    let greeter = std::fs::read_to_string(folder.join("greeter.ag")).unwrap();
    let server = a2a::start(served.path(), &greeter, None).await;
    let out = tokio::process::Command::new(&py)
        .arg(folder.join("python_client.py"))
        .arg(&server.base)
        .args(["--message", "greet name=Ana"])
        .output()
        .await
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let seen: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        seen["text"],
        "Hello, Ana! This greeting came from Metagente."
    );
}
