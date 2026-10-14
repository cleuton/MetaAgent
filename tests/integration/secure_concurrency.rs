// 0.1.3: new file. Many requests at once over TLS, and a handshake that never finishes counts against the time limit.
use crate::common::self_signed::{ServeProc, free_port, pair_in};
use crate::common::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn fifty_simultaneous_requests_over_tls_are_served_at_the_same_time() {
    let dir = tempfile::tempdir().unwrap();
    // each request waits one second: one after another this would need fifty seconds
    let source = "agent Slowpoke\n  goal \"Wait, then answer\"\n  tool clock\n  accepts echo word\n  on echo\n    clock.wait seconds: 1\n    reply \"echo {word}\"\n";
    std::fs::write(dir.path().join("slow.ag"), source).unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        "[secure]\nagents = [\"Slowpoke\"]\n[serve]\ntls_cert = \"cert.pem\"\ntls_key = \"key.pem\"\n",
    )
    .unwrap();
    let files = pair_in(dir.path());
    let port = free_port();
    let _serve = ServeProc::start(
        dir.path(),
        &["slow.ag".to_string(), "--a2a".to_string(), port.to_string()],
        1,
    );
    let client = reqwest::Client::builder()
        .add_root_certificate(reqwest::Certificate::from_pem(files.ca_pem.as_bytes()).unwrap())
        .build()
        .unwrap();
    let url = format!("https://localhost:{}/a2a", port);
    let started = std::time::Instant::now();
    let mut handles = Vec::new();
    for i in 0..50 {
        let client = client.clone();
        let url = url.clone();
        handles.push(tokio::spawn(async move {
            let body = serde_json::json!({"jsonrpc": "2.0", "id": i, "method": "SendMessage",
                "params": crate::common::a2a::user_text(&format!("echo w{}", i))});
            let reply: serde_json::Value = client
                .post(&url)
                .header("A2A-Version", "1.0")
                .body(body.to_string())
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            (i, reply)
        }));
    }
    for h in handles {
        let (i, reply) = h.await.unwrap();
        let task = &reply["result"]["task"];
        assert_eq!(
            task["status"]["state"], "TASK_STATE_COMPLETED",
            "request {}: {}",
            i, reply
        );
        assert_eq!(
            task["artifacts"][0]["parts"][0]["text"],
            format!("echo w{}", i)
        );
    }
    let took = started.elapsed().as_secs_f64();
    assert!(
        took < 10.0,
        "50 requests took {:.1}s over TLS, so they were not served at the same time",
        took
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_handshake_that_never_finishes_counts_against_the_time_limit_and_is_reported_plainly() {
    // a server that accepts the connection and then says nothing at all
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((socket, _)) = listener.accept().await {
            held.push(socket);
        }
    });
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |c| c.timeout_seconds = 1);
    let source = project_file("examples/remote_weather.ag").replace(
        "http://127.0.0.1:8080",
        &format!("https://127.0.0.1:{}", port),
    );
    let started = std::time::Instant::now();
    let err = run_source(&rt, &source, "plan", &[("city", "Lisbon")])
        .await
        .unwrap_err()
        .render();
    assert!(
        started.elapsed().as_secs() < 8,
        "took {:?}",
        started.elapsed()
    );
    assert!(
        err.contains("`Bob.ask` did not finish within 1 seconds"),
        "{}",
        err
    );
    for word in ["rustls", "hyper", "reqwest", "panicked"] {
        assert!(!err.contains(word), "`{}` leaked: {}", word, err);
    }
}
