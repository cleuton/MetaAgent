// 0.1.2: new file. Every file changed for 0.1.2 carries a `0.1.2:` comment, and the version says 0.1.2 everywhere.
// 0.1.3: new content. Every file changed for 0.1.3 carries a `0.1.3:` comment, files that did not change carry none,
// and the version says 0.1.3 everywhere. (The 0.1.2 version of this test did the same for 0.1.2.)
// 0.1.4: rewritten for 0.1.4 (FR-052): the same three checks with the `0.1.4:` marker and the 0.1.4 version, plus the
// check that no comment of an earlier release was removed.
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(relative: &str) -> String {
    std::fs::read_to_string(root().join(relative))
        .unwrap_or_else(|e| panic!("cannot read {}: {}", relative, e))
}

// 0.1.4: the files that this release added or changed (code, tests, examples, samples)
const CHANGED: &[&str] = &[
    "Cargo.toml",
    "src/a2a/card.rs",
    "src/a2a/client.rs",
    "src/a2a/server.rs",
    "src/a2a/store.rs",
    "src/link/tool.rs",
    "src/main.rs",
    "src/mcp/client.rs",
    "src/mcp/gate.rs",
    "src/mcp/mod.rs",
    "src/mcp/server.rs",
    "src/mcp/tool.rs",
    "src/runtime/auth.rs",
    "src/runtime/auth_check.rs",
    "src/runtime/config.rs",
    "src/runtime/mod.rs",
    "src/runtime/run.rs",
    "src/runtime/scaffold.rs",
    "src/runtime/serve.rs",
    "src/runtime/task.rs",
    "src/runtime/validator.rs",
    "src/runtime/version.rs",
    "tests/contract/a2a_wire.rs",
    "tests/contract/agent_card.rs",
    "tests/integration/auth_cache.rs",
    "tests/integration/auth_chain_sample.rs",
    "tests/integration/auth_concurrency.rs",
    "tests/integration/auth_fail_closed.rs",
    "tests/integration/auth_forward.rs",
    "tests/integration/auth_no_leak.rs",
    "tests/integration/auth_regression.rs",
    "tests/integration/auth_side_doors.rs",
    "tests/integration/auth_validator.rs",
    "tests/integration/common/auth.rs",
    "tests/integration/common/fake_a2a.rs",
    "tests/integration/common/mod.rs",
    "tests/integration/common/python.rs",
    "tests/integration/main.rs",
    "tests/integration/net_config.rs",
    "tests/integration/python_interop_auth.rs",
    "tests/integration/version_gate.rs",
    "tests/interop/a2a_sdk_client.py",
    "tests/interop/python_agent/agent.py",
    "tests/unit/auth_cache.rs",
    "tests/unit/auth_header.rs",
    "tests/unit/change_markers.rs",
    "tests/unit/main.rs",
    "tests/unit/redaction.rs",
    "tests/unit/version.rs",
    "multi-agent-samples/auth-chain/planner.ag",
    "multi-agent-samples/auth-chain/researcher.ag",
    "multi-agent-samples/auth-chain/facts.ag",
    "multi-agent-samples/auth-chain/metagente.toml",
    "multi-agent-samples/auth-chain/validator.py",
    "multi-agent-samples/auth-chain/.gitignore",
];

// 0.1.4: files that this release did not touch must not carry the marker. The language itself does not change
// (no new keyword, no new syntax), so the lexer, the parser and the checker are all in this list.
const UNCHANGED: &[&str] = &[
    "src/lang/ast.rs",
    "src/lang/check.rs",
    "src/lang/lexer.rs",
    "src/lang/parser.rs",
    "src/lang/params.rs",
    "src/lang/mod.rs",
    "src/link/resolve.rs",
    "src/link/check.rs",
    "src/link/loader.rs",
    "src/link/cycle.rs",
    "src/link/interface.rs",
    "src/tools/file.rs",
    "src/tools/http.rs",
    "src/llm/mod.rs",
    "src/diagnostics/mod.rs",
    "src/runtime/net.rs",
    "src/runtime/secure.rs",
    "src/runtime/tls.rs",
    "src/runtime/think.rs",
    "src/runtime/interpreter.rs",
    "examples/weather.ag",
    "examples/planner.ag",
    "tests/integration/permissions.rs",
];

#[test]
fn every_changed_file_has_the_marker() {
    for file in CHANGED {
        assert!(
            read(file).contains("0.1.4:"),
            "{} has no `0.1.4:` comment",
            file
        );
    }
}

#[test]
fn files_that_were_not_changed_have_no_marker() {
    for file in UNCHANGED {
        assert!(
            !read(file).contains("0.1.4:"),
            "{} should not carry the marker",
            file
        );
    }
}

#[test]
fn no_changed_file_lost_a_comment_of_an_earlier_release() {
    // 0.1.4: markers of 0.1.2 and 0.1.3 stay where they were (no existing comment may be removed)
    for file in [
        "src/runtime/config.rs",
        "src/runtime/serve.rs",
        "src/runtime/scaffold.rs",
        "src/runtime/run.rs",
        "src/main.rs",
    ] {
        assert!(
            read(file).contains("0.1.2:"),
            "{} lost its `0.1.2:` comments",
            file
        );
    }
    for file in [
        "src/a2a/client.rs",
        "src/a2a/server.rs",
        "src/link/tool.rs",
        "src/mcp/client.rs",
        "src/mcp/mod.rs",
        "src/mcp/tool.rs",
        "src/runtime/config.rs",
        "src/runtime/serve.rs",
        "src/runtime/task.rs",
        "src/runtime/scaffold.rs",
        "Cargo.toml",
    ] {
        assert!(
            read(file).contains("0.1.3:"),
            "{} lost its `0.1.3:` comments",
            file
        );
    }
}

#[test]
fn the_version_is_0_1_4_everywhere() {
    assert_eq!(env!("CARGO_PKG_VERSION"), "0.1.4");
    assert!(read("CHANGELOG.md").contains("## v0.1.4"));
    assert!(read("README.md").contains("**v0.1.4.**"));
    assert!(read("Cargo.lock").contains("name = \"metagente\"\nversion = \"0.1.4\""));
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_metagente"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&out.stdout)
            .trim_end()
            .ends_with("0.1.4")
    );
}

#[test]
fn the_samples_commit_no_secret() {
    // 0.1.4: no key and no token is ever committed: they are made by the person following each README
    let tokens = root().join("multi-agent-samples/auth-chain/.gitignore");
    assert!(read("multi-agent-samples/auth-chain/.gitignore").contains("valid-tokens.txt"));
    assert!(tokens.is_file());
    let gitignore = root().join("multi-agent-samples/https-chain/.gitignore");
    if gitignore.exists() {
        assert!(read("multi-agent-samples/https-chain/.gitignore").contains("certs/"));
    }
    assert!(Path::new(&root().join("Cargo.toml")).is_file());
}

#[test]
fn no_token_or_key_library_was_added() {
    // 0.1.4: Metagente never opens, signs or verifies a token (FR-036); the only new dependency is a hash
    let manifest = read("Cargo.toml");
    for banned in [
        "jsonwebtoken",
        "jwt",
        "openssl",
        "oauth",
        "biscuit",
        "paseto",
        "josekit",
    ] {
        assert!(
            !manifest.to_lowercase().contains(banned),
            "Cargo.toml mentions {}",
            banned
        );
    }
}
