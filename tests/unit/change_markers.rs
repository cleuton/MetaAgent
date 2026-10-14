// 0.1.2: new file. Every file changed for 0.1.2 carries a `0.1.2:` comment, and the version says 0.1.2 everywhere.
// 0.1.3: new content. Every file changed for 0.1.3 carries a `0.1.3:` comment, files that did not change carry none,
// and the version says 0.1.3 everywhere. (The 0.1.2 version of this test did the same for 0.1.2.)
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(relative: &str) -> String {
    std::fs::read_to_string(root().join(relative))
        .unwrap_or_else(|e| panic!("cannot read {}: {}", relative, e))
}

// 0.1.2: the files that this release added or changed in the code, the tests and the examples
// 0.1.3: the files that this release added or changed (code, tests, examples, samples, the CI workflow and the packaging script)
const CHANGED: &[&str] = &[
    "Cargo.toml",
    ".github/workflows/build.yml",
    "scripts/packaging-check.sh",
    "src/a2a/client.rs",
    "src/a2a/server.rs",
    "src/link/tool.rs",
    "src/llm/providers.rs",
    "src/main.rs",
    "src/mcp/client.rs",
    "src/mcp/mod.rs",
    "src/mcp/tool.rs",
    "src/runtime/config.rs",
    "src/runtime/mod.rs",
    "src/runtime/net.rs",
    "src/runtime/run.rs",
    "src/runtime/scaffold.rs",
    "src/runtime/secure.rs",
    "src/runtime/serve.rs",
    "src/runtime/task.rs",
    "src/runtime/think.rs",
    "src/runtime/tls.rs",
    "src/tools/http.rs",
    "src/tools/mod.rs",
    "tests/integration/common/a2a_tls.rs",
    "tests/integration/common/fake_mcp.rs",
    "tests/integration/common/mod.rs",
    "tests/integration/common/proxy.rs",
    "tests/integration/common/python.rs",
    "tests/integration/common/self_signed.rs",
    "tests/integration/common/tls_server.rs",
    "tests/integration/examples_check.rs",
    "tests/integration/https_chain.rs",
    "tests/integration/main.rs",
    "tests/integration/matrix_regression.rs",
    "tests/integration/mcp_https.rs",
    "tests/integration/net_config.rs",
    "tests/integration/net_model.rs",
    "tests/integration/net_pass_to_tools.rs",
    "tests/integration/net_proxy.rs",
    "tests/integration/net_regression.rs",
    "tests/integration/net_selfsigned.rs",
    "tests/integration/net_tls.rs",
    "tests/integration/perf.rs",
    "tests/integration/python_interop.rs",
    "tests/integration/secure_agent.rs",
    "tests/integration/secure_check.rs",
    "tests/integration/secure_concurrency.rs",
    "tests/integration/secure_link.rs",
    "tests/integration/secure_llm.rs",
    "tests/integration/secure_remote.rs",
    "tests/integration/secure_selfsigned.rs",
    "tests/integration/secure_serve.rs",
    "tests/integration/secure_serve_refusals.rs",
    "tests/integration/secure_tools.rs",
    "tests/interop/a2a_sdk_client.py",
    "tests/interop/helpers/make_cert.py",
    "tests/interop/python_agent/agent.py",
    "tests/interop/requirements.txt",
    "tests/unit/change_markers.rs",
    "tests/unit/diagnostics_golden.rs",
    "tests/unit/example_size.rs",
    "tests/unit/main.rs",
    "tests/unit/net_proxy_rules.rs",
    "tests/unit/secure_rules.rs",
    "examples/python_interop/caller.ag",
    "examples/python_interop/greeter.ag",
    "examples/python_interop/python_agent.py",
    "examples/python_interop/python_client.py",
    "examples/python_interop/requirements.txt",
    "multi-agent-samples/https-chain/planner.ag",
    "multi-agent-samples/https-chain/researcher.ag",
    "multi-agent-samples/https-chain/facts.ag",
    "multi-agent-samples/https-chain/metagente.toml",
    "multi-agent-samples/https-chain/.gitignore",
];

// 0.1.2: files of 0.1.1 that this release did not touch must not carry the marker
// 0.1.3: files that this release did not touch must not carry the marker. The language itself does not change
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
    "src/tools/file.rs",
    "src/llm/mod.rs",
    "src/diagnostics/mod.rs",
    "examples/weather.ag",
    "examples/planner.ag",
    "tests/integration/permissions.rs",
];

#[test]
fn every_changed_file_has_the_marker() {
    for file in CHANGED {
        assert!(
            read(file).contains("0.1.3:"),
            "{} has no `0.1.3:` comment",
            file
        );
    }
}

#[test]
fn files_that_were_not_changed_have_no_marker() {
    for file in UNCHANGED {
        assert!(
            !read(file).contains("0.1.3:"),
            "{} should not carry the marker",
            file
        );
    }
}

#[test]
fn no_changed_file_lost_a_comment_of_an_earlier_release() {
    // 0.1.3: markers of 0.1.2 stay where they were (no existing comment may be removed)
    for file in [
        "src/runtime/config.rs",
        "src/runtime/serve.rs",
        "src/runtime/scaffold.rs",
        "src/runtime/run.rs",
        "src/main.rs",
        "src/link/loader.rs",
    ] {
        assert!(
            read(file).contains("0.1.2:"),
            "{} lost its `0.1.2:` comments",
            file
        );
    }
}

#[test]
fn the_version_is_0_1_3_everywhere() {
    assert_eq!(env!("CARGO_PKG_VERSION"), "0.1.3");
    assert!(read("CHANGELOG.md").contains("## v0.1.3"));
    assert!(read("README.md").contains("**v0.1.3.**"));
    assert!(read("Cargo.lock").contains("name = \"metagente\"\nversion = \"0.1.3\""));
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_metagente"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&out.stdout)
            .trim_end()
            .ends_with("0.1.3")
    );
}

#[test]
fn the_sample_for_secure_connections_exists_once_it_is_built() {
    // 0.1.3: filled in by the sample tasks; here only the folder rule (no private key is ever committed)
    let gitignore = root().join("multi-agent-samples/https-chain/.gitignore");
    if gitignore.exists() {
        assert!(read("multi-agent-samples/https-chain/.gitignore").contains("certs/"));
    }
    assert!(Path::new(&root().join("Cargo.toml")).is_file());
}
