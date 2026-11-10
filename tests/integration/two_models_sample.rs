// 0.1.5: new file (US6, FR-018, SC-006). The sample multi-agent-samples/two-models/: one agent of fewer than 10 lines of
// code, a metagente.toml with [version] and two named models (one Anthropic, one OpenAI), and a README whose
// "Break it on purpose" messages are exactly what the program prints.
use std::path::{Path, PathBuf};
use std::process::Command;

fn sample() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("multi-agent-samples/two-models")
}

fn copy_to(tmp: &Path) {
    for name in ["metagente.toml", "assistant.ag"] {
        std::fs::copy(sample().join(name), tmp.join(name)).unwrap();
    }
}

fn run(dir: &Path, args: &[&str], with_variables: bool) -> String {
    let mut c = Command::new(env!("CARGO_BIN_EXE_metagente"));
    c.current_dir(dir).args(args);
    for n in ["FAST_KEY", "FAST_MODEL", "STRONG_KEY", "STRONG_MODEL"] {
        if with_variables {
            c.env(n, "x");
        } else {
            c.env_remove(n);
        }
    }
    let out = c.output().unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn the_agent_has_fewer_than_ten_lines_of_code() {
    let text = std::fs::read_to_string(sample().join("assistant.ag")).unwrap();
    let code = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .count();
    assert!(code < 10, "{} lines of code", code);
    assert!(text.contains("using fast") && text.contains("using strong"));
}

#[test]
fn the_toml_has_version_and_two_named_models_one_of_each_company() {
    let text = std::fs::read_to_string(sample().join("metagente.toml")).unwrap();
    let t: toml::Table = text.parse().unwrap();
    assert_eq!(t["version"]["minimum"].as_str(), Some("0.1.5"));
    let llm = t["llm"].as_table().unwrap();
    assert_eq!(llm["fast"]["provider"].as_str(), Some("anthropic"));
    assert_eq!(llm["strong"]["provider"].as_str(), Some("openai"));
    for (name, model_env, key_env) in [
        ("fast", "FAST_MODEL", "FAST_KEY"),
        ("strong", "STRONG_MODEL", "STRONG_KEY"),
    ] {
        assert_eq!(llm[name]["model_env"].as_str(), Some(model_env));
        assert_eq!(llm[name]["api_key_env"].as_str(), Some(key_env));
    }
    // no key and no model name is written in the file
    assert!(!text.contains("sk-"));
}

#[test]
fn the_sample_passes_check_with_the_variables_set_and_warns_without_them() {
    let dir = tempfile::tempdir().unwrap();
    copy_to(dir.path());
    let ok = run(dir.path(), &["check", "assistant.ag"], true);
    assert!(
        ok.contains("No problems found in assistant.ag (1 agent)."),
        "{}",
        ok
    );
    assert!(!ok.contains("Warning"), "{}", ok);
    let warned = run(dir.path(), &["check", "assistant.ag"], false);
    assert!(
        warned.contains(
            "Warning: the model fast needs the variable FAST_KEY, and it is not set here."
        ),
        "{}",
        warned
    );
    assert!(
        warned.contains("No problems found in assistant.ag (1 agent)."),
        "{}",
        warned
    );
}

/// The ```text blocks of the "Break it on purpose" section of the README, in order.
fn readme_blocks() -> Vec<String> {
    let text = std::fs::read_to_string(sample().join("README.md")).unwrap();
    let section = text
        .split("## Break it on purpose")
        .nth(1)
        .unwrap()
        .split("## Common problems")
        .next()
        .unwrap();
    let mut blocks = Vec::new();
    let mut current: Option<Vec<&str>> = None;
    for line in section.lines() {
        let t = line.trim();
        if t == "```text" {
            current = Some(Vec::new());
        } else if t == "```" {
            if let Some(b) = current.take() {
                blocks.push(b.join("\n"));
            }
        } else if let Some(b) = current.as_mut() {
            b.push(t);
        }
    }
    blocks
}

/// "Problem on line N of FILE: message" with the folder removed from FILE, and the Fix line.
fn problem_and_fix(output: &str) -> String {
    let mut lines = output.lines();
    let problem = lines
        .by_ref()
        .find(|l| l.starts_with("Problem on line"))
        .unwrap_or_else(|| panic!("no problem in:\n{}", output));
    let (head, rest) = problem.split_once(" of ").unwrap();
    let (path, message) = rest.split_once(": ").unwrap();
    let base = path.rsplit('/').next().unwrap();
    let fix = output.lines().find(|l| l.starts_with("Fix:")).unwrap_or("");
    format!("{} of {}: {}\n{}", head, base, message, fix)
}

#[test]
fn every_break_it_on_purpose_message_is_what_the_program_prints() {
    let blocks = readme_blocks();
    assert_eq!(blocks.len(), 4, "{:?}", blocks);
    // 1. a misspelled model name
    let dir = tempfile::tempdir().unwrap();
    copy_to(dir.path());
    let agent = std::fs::read_to_string(dir.path().join("assistant.ag")).unwrap();
    std::fs::write(
        dir.path().join("assistant.ag"),
        agent.replace("using fast", "using fats"),
    )
    .unwrap();
    assert_eq!(
        problem_and_fix(&run(dir.path(), &["check", "assistant.ag"], true)),
        blocks[0]
    );
    // 2. a misspelled setting
    let dir = tempfile::tempdir().unwrap();
    copy_to(dir.path());
    let toml = std::fs::read_to_string(dir.path().join("metagente.toml")).unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        toml.replacen("api_key_env", "api_key_evn", 1),
    )
    .unwrap();
    assert_eq!(
        problem_and_fix(&run(dir.path(), &["check", "assistant.ag"], true)),
        blocks[1]
    );
    // 3. a variable that is not set
    let dir = tempfile::tempdir().unwrap();
    copy_to(dir.path());
    assert_eq!(
        problem_and_fix(&run(
            dir.path(),
            &["run", "assistant.ag", "ask", "question=why"],
            false
        )),
        blocks[2]
    );
    // 4. another address
    let dir = tempfile::tempdir().unwrap();
    copy_to(dir.path());
    let toml = std::fs::read_to_string(dir.path().join("metagente.toml")).unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        toml.replacen(
            "model_env = \"FAST_MODEL\"\n",
            "model_env = \"FAST_MODEL\"\nbase_url = \"https://example.com/v1\"\n",
            1,
        ),
    )
    .unwrap();
    assert_eq!(
        problem_and_fix(&run(dir.path(), &["check", "assistant.ag"], true)),
        blocks[3]
    );
}

#[test]
fn the_readme_has_the_commands_for_every_system_and_a_table_of_common_problems() {
    let text = std::fs::read_to_string(sample().join("README.md")).unwrap();
    for needle in [
        "**Linux and macOS**",
        "export FAST_KEY",
        "$env:FAST_KEY",
        "set FAST_KEY",
        "## Common problems",
        "| What you see |",
    ] {
        assert!(text.contains(needle), "{}", needle);
    }
}
