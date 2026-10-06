// 0.1.2: new file. Every file changed for 0.1.2 carries a `0.1.2:` comment, and the version says 0.1.2 everywhere.
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(relative: &str) -> String {
    std::fs::read_to_string(root().join(relative))
        .unwrap_or_else(|e| panic!("cannot read {}: {}", relative, e))
}

// 0.1.2: the files that this release added or changed in the code, the tests and the examples
const CHANGED: &[&str] = &[
    "Cargo.toml",
    "src/lang/lexer.rs",
    "src/lang/parser.rs",
    "src/lang/params.rs",
    "src/lang/mod.rs",
    "src/runtime/config.rs",
    "src/runtime/mod.rs",
    "src/runtime/run.rs",
    "src/runtime/serve.rs",
    "src/runtime/scaffold.rs",
    "src/link/loader.rs",
    "src/main.rs",
    "tests/unit/main.rs",
    "tests/unit/lexer_multiline.rs",
    "tests/unit/parser_tests.rs",
    "tests/unit/diagnostics_golden.rs",
    "tests/unit/change_markers.rs",
    "tests/integration/main.rs",
    "tests/integration/examples_check.rs",
    "tests/integration/multiline_run.rs",
    "tests/integration/parameters.rs",
    "tests/integration/link_parameters.rs",
    "examples/multiline.ag",
    "examples/parameters.ag",
    "examples/metagente.toml",
];

// 0.1.2: files of 0.1.1 that this release did not touch must not carry the marker
const UNCHANGED: &[&str] = &[
    "src/lang/ast.rs",
    "src/lang/check.rs",
    "src/link/resolve.rs",
    "src/link/check.rs",
    "src/link/tool.rs",
    "src/a2a/server.rs",
    "src/mcp/client.rs",
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
            read(file).contains("0.1.2:"),
            "{} has no `0.1.2:` comment",
            file
        );
    }
}

#[test]
fn files_that_were_not_changed_have_no_marker() {
    for file in UNCHANGED {
        assert!(
            !read(file).contains("0.1.2:"),
            "{} should not carry the marker",
            file
        );
    }
}

#[test]
fn the_version_is_0_1_2_everywhere() {
    assert_eq!(env!("CARGO_PKG_VERSION"), "0.1.2");
    assert!(read("CHANGELOG.md").contains("## v0.1.2"));
    assert!(read("README.md").contains("**v0.1.2.**"));
    assert!(read("Cargo.lock").contains("name = \"metagente\"\nversion = \"0.1.2\""));
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_metagente"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&out.stdout)
            .trim_end()
            .ends_with("0.1.2")
    );
}

#[test]
fn syntax_md_marks_what_is_new() {
    let doc = read("docs/syntax.md");
    for needle in [
        "(new in 0.1.2)",
        "## Text",
        "## Parameters",
        "## Changes in 0.1.2",
        "\"@parameters.\" NAME",
        "### `\"\"\"`",
        "### `@parameters`",
    ] {
        assert!(doc.contains(needle), "docs/syntax.md lacks `{}`", needle);
    }
    assert!(Path::new(&root().join("examples/metagente.toml")).is_file());
}
