use crate::common::a2a::{self, rpc, user_text};

#[tokio::test]
async fn fifty_simultaneous_requests_all_complete_correctly_without_the_author_doing_anything() {
    let dir = tempfile::tempdir().unwrap();
    // Each request waits one second. Served one after another this would need fifty seconds.
    let source = "agent Slowpoke\n  goal \"Wait, then answer\"\n  tool clock\n  accepts echo word\n  on echo\n    clock.wait seconds: 1\n    reply \"echo {word}\"\n";
    let server = a2a::start(dir.path(), source, None).await;
    let url = format!("{}/a2a", server.base);
    let started = std::time::Instant::now();
    let mut handles = Vec::new();
    for i in 0..50 {
        let url = url.clone();
        handles.push(tokio::spawn(async move {
            let reply = rpc(&url, "SendMessage", user_text(&format!("echo w{}", i))).await;
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
        "50 requests took {:.1}s, so they were not served at the same time",
        took
    );
}
