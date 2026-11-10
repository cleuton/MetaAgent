// 0.1.4: the sample multi-agent-samples/auth-chain end to end (FR-040): the answer, the refusals, and that the two
// downstream servers judged the token the caller sent to the Planner.
use crate::common::auth::*;
use std::path::Path;

fn python_ok() -> bool {
    std::process::Command::new("python3")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn copy_sample(to: &Path, researcher_port: u16, facts_port: u16) {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("multi-agent-samples/auth-chain");
    for name in [
        "planner.ag",
        "researcher.ag",
        "facts.ag",
        "metagente.toml",
        "validator.py",
        "call.json",
    ] {
        let text = std::fs::read_to_string(from.join(name)).unwrap();
        let text = text
            .replace("127.0.0.1:8081", &format!("127.0.0.1:{}", researcher_port))
            .replace("127.0.0.1:9000", &format!("127.0.0.1:{}", facts_port));
        write(to, name, &text);
    }
    // the validator of the sample, plus a line that records the token it was given (a test-only addition)
    let validator = std::fs::read_to_string(to.join("validator.py")).unwrap();
    write(
        to,
        "validator.py",
        &validator.replace(
            "token = sys.stdin.readline().strip()\n",
            "token = sys.stdin.readline().strip()\nopen('seen.txt', 'a').write(token + '\\n')\n",
        ),
    );
    write(to, "valid-tokens.txt", "demo-token-1\n");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_sample_works_end_to_end_and_every_stage_judges_the_same_token() {
    if !python_ok() {
        assert!(
            std::env::var_os("CI").is_none(),
            "python3 is needed for this test in CI"
        );
        eprintln!("skipped: python3 is not installed");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (pp, rp, fp) = (free_port(), free_port(), free_port());
    copy_sample(dir.path(), rp, fp);
    let d = dir.path();
    let _facts = serve(d, "facts", &["facts.ag", "--mcp", &fp.to_string()], fp);
    let _researcher = serve(
        d,
        "researcher",
        &["researcher.ag", "--a2a", &rp.to_string()],
        rp,
    );
    let planner = serve(d, "planner", &["planner.ag", "--a2a", &pp.to_string()], pp);

    // the README's request, as saved in call.json
    let body: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(d.join("call.json")).unwrap()).unwrap();
    let good = post(pp, "/a2a", Some("demo-token-1"), body.clone()).await;
    assert_eq!(good.status, 200, "{}", good.text);
    assert_eq!(answer(&good), "Planner got: Rust is a systems language");
    let seen = std::fs::read_to_string(d.join("seen.txt")).unwrap();
    assert_eq!(
        seen.lines().filter(|l| *l == "demo-token-1").count(),
        3,
        "Planner, Researcher and Facts each judged it: {}",
        seen
    );

    let none = post(pp, "/a2a", None, body.clone()).await;
    assert_eq!(none.status, 401);
    assert_eq!(none.www_authenticate.as_deref(), Some("Bearer"));
    assert!(answer(&none).contains("Planner.plan needs a token, and this request had none."));
    let bad = post(pp, "/a2a", Some("nope"), body).await;
    assert_eq!(bad.status, 401);
    assert!(answer(&bad).contains("The token was not accepted for Planner.plan."));
    assert!(planner.log().contains("Refused Planner.plan: no token."));
    assert!(
        planner.log().contains(
            "Refused Planner.plan: token not accepted (the validator exited with code 1)."
        )
    );
    // and the tool alone, with no token
    let direct = mcp_call(fp, None, "Facts.lookup", "rust").await;
    assert_eq!(direct.status, 401);
}

#[test]
fn the_sample_is_small_and_commits_no_token() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("multi-agent-samples/auth-chain");
    for name in ["planner.ag", "researcher.ag", "facts.ag"] {
        let text = std::fs::read_to_string(root.join(name)).unwrap();
        let code = text
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
            .count();
        assert!(code < 10, "{} has {} lines of code", name, code);
        assert!(!text.contains("think"), "{} must not need a model", name);
    }
    let validator = std::fs::read_to_string(root.join("validator.py")).unwrap();
    assert!(validator.lines().count() < 20);
    // the reader may have made valid-tokens.txt (git-ignored), but it must never be tracked
    if let Ok(out) = std::process::Command::new("git")
        .args(["ls-files", "--", "valid-tokens.txt"])
        .current_dir(&root)
        .output()
    {
        assert!(
            out.stdout.is_empty(),
            "valid-tokens.txt is made by the reader and must not be tracked"
        );
    }
    assert!(
        std::fs::read_to_string(root.join(".gitignore"))
            .unwrap()
            .contains("valid-tokens.txt")
    );
    let toml = std::fs::read_to_string(root.join("metagente.toml")).unwrap();
    assert!(toml.contains("[version]") && toml.contains("[auth]"));
    let readme = std::fs::read_to_string(root.join("README.md")).unwrap();
    for needed in [
        "Not tested on Microsoft Windows",
        "Break it on purpose",
        "Common problems",
        "https-chain",
    ] {
        assert!(
            readme.to_lowercase().contains(&needed.to_lowercase()),
            "README lacks {}",
            needed
        );
    }
}
