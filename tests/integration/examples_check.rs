//! Every file in examples/ must behave: valid ones pass `check`, `broken_*` ones fail with a plain message.

use std::path::PathBuf;
use std::process::Command;

fn examples() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().map(|e| e == "ag").unwrap_or(false))
        .collect();
    files.sort();
    files
}

fn metagente(dir: &std::path::Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_metagente"));
    c.current_dir(dir);
    c
}

#[test]
fn valid_examples_pass_check() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut count = 0;
    for file in examples() {
        let name = file.file_name().unwrap().to_string_lossy().to_string();
        if name.starts_with("broken_") {
            continue;
        }
        // 0.1.2: parameters.ag reads examples/metagente.toml, so it is checked from its own folder below
        if name == "parameters.ag" {
            continue;
        }
        let out = metagente(&root)
            .args(["check", &format!("examples/{}", name)])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "examples/{} should pass check:\n{}",
            name,
            String::from_utf8_lossy(&out.stderr)
        );
        count += 1;
    }
    assert!(
        count >= 6,
        "expected the beginner examples to be there, found {}",
        count
    );
}

#[test]
fn broken_examples_fail_with_a_plain_message() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut count = 0;
    for file in examples() {
        let name = file.file_name().unwrap().to_string_lossy().to_string();
        if !name.starts_with("broken_") {
            continue;
        }
        let path = format!("examples/{}", name);
        let check = metagente(&root).args(["check", &path]).output().unwrap();
        let failed = if !check.status.success() {
            check
        } else {
            // Some problems only show up when the agent runs (for example a file outside its folder).
            let scratch = tempfile::tempdir().unwrap();
            let run = Command::new(env!("CARGO_BIN_EXE_metagente"))
                .current_dir(scratch.path())
                .args(["run", root.join(&path).to_str().unwrap()])
                .output()
                .unwrap();
            assert!(!run.status.success(), "{} should fail when run", name);
            run
        };
        let err = String::from_utf8_lossy(&failed.stderr);
        assert!(err.contains("Problem"), "{}: {}", name, err);
        assert!(err.contains("Fix:"), "{}: {}", name, err);
        for word in ["panicked", "unwrap", "src/", "RUST_BACKTRACE"] {
            assert!(!err.contains(word), "{}: found `{}` in {}", name, word, err);
        }
        count += 1;
    }
    assert!(count >= 3, "expected the broken examples, found {}", count);
}

// 0.1.2: examples/parameters.ag reads the [parameters] of examples/metagente.toml, next to it
#[test]
fn the_parameters_example_passes_check_from_its_own_folder() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
    let out = metagente(&dir)
        .args(["check", "parameters.ag"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "examples/parameters.ag should pass check:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// 0.1.2: the multi-line example is an ordinary example and must pass check from the project folder
#[test]
fn the_multiline_example_passes_check() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out = metagente(&root)
        .args(["check", "examples/multiline.ag"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "examples/multiline.ag should pass check:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
