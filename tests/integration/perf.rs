//! The performance goals from the plan: start fast, parse fast.

use std::process::Command;
use std::time::{Duration, Instant};

fn best_of(n: usize, mut f: impl FnMut()) -> Duration {
    (0..n)
        .map(|_| {
            let start = Instant::now();
            f();
            start.elapsed()
        })
        .min()
        .unwrap()
}

#[test]
fn a_trivial_agent_runs_from_cold_start_in_under_100_ms() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("a.ag"),
        "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply \"ok\"\n",
    )
    .unwrap();
    let best = best_of(7, || {
        let out = Command::new(env!("CARGO_BIN_EXE_metagente"))
            .current_dir(dir.path())
            .args(["run", "a.ag", "go"])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "ok");
    });
    assert!(
        best < Duration::from_millis(100),
        "best start-to-answer time was {:?}",
        best
    );
}

#[test]
fn a_five_hundred_line_file_parses_in_under_50_ms() {
    let mut source = String::new();
    let mut lines = 0;
    let mut n = 0;
    while lines < 500 {
        source.push_str(&format!(
            "agent Agent{n}\n  goal \"Agent number {n}\"\n  tool file\n  tool state\n  accepts one name\n  accepts two\n  on one\n    text = file.read path: \"a{n}.txt\"\n    state.set key: \"k\" value: text\n    if name is \"x\" and not text is \"y\"\n      reply \"first {{name}}\"\n    otherwise\n      for item in [1, 2, 3]\n        state.set key: \"i\" value: item\n      reply \"second\"\n  on two\n    reply state.get key: \"k\"\n"
        ));
        lines += 17;
        n += 1;
    }
    assert!(source.lines().count() >= 500);
    let best = best_of(5, || {
        let agents = metagente::lang::parse_file("big.ag", None, &source).unwrap();
        assert!(!agents.is_empty());
    });
    assert!(
        best < Duration::from_millis(50),
        "parsing {} lines took {:?}",
        source.lines().count(),
        best
    );
}

// 0.1.3: an agent that makes no network call must not pay for certificates it does not use
#[test]
fn an_agent_without_network_calls_builds_no_client_and_still_starts_fast() {
    use metagente::runtime::Runtime;
    use metagente::runtime::config::Config;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("big-ca.pem"), "x".repeat(200_000)).unwrap();
    let mut config = Config::default();
    config.network.allow_self_signed = true;
    config.network.ca_file = Some(dir.path().join("big-ca.pem"));
    config.network.proxy.url = Some("http://proxy.example:3128".to_string());
    let rt = Runtime::new(config, None);
    assert_eq!(rt.net.clients_built(), 0);

    // and the whole program, with a [network] section in its metagente.toml, still starts in under 100 ms
    std::fs::write(
        dir.path().join("a.ag"),
        "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply \"ok\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        "[network]\nallow_self_signed = true\n[network.proxy]\nurl = \"http://proxy.example:3128\"\n",
    )
    .unwrap();
    let best = best_of(7, || {
        let out = Command::new(env!("CARGO_BIN_EXE_metagente"))
            .current_dir(dir.path())
            .args(["run", "a.ag", "go"])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "ok");
    });
    assert!(
        best < Duration::from_millis(100),
        "best start-to-answer time was {:?}",
        best
    );
}
