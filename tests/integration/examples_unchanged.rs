// 0.1.5: new file (FR-022, SC-010). Every example of `examples/`, every sample of `multi-agent-samples/` and every
// `.ag` and `metagente.toml` block that README.md and docs/ had at tag v0.1.4 behaves as it did on 0.1.4: `check` gives
// the same exit code and the same words. What 0.1.4 did was frozen BEFORE any change by `scripts/freeze-v0.1.4.sh`
// (tests/fixtures/v0.1.4-golden). The version number is compared as "any version" (F16). Nothing here edits an example.
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const HELLO: &str = "agent Hello\n  goal \"say hello\"\n  accepts hi\n  on hi\n    reply \"hi\"\n";

/// What a person generates when they run a sample; never part of the sample.
fn generated(name: &str) -> bool {
    name == "certs"
        || name == "valid-tokens.txt"
        || name == "target"
        || name.ends_with(".pdf")
        || name.ends_with(".pem")
}

fn copy_folder(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().to_string();
        if generated(&name) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            copy_folder(&path, &to.join(&name));
        } else {
            std::fs::copy(&path, to.join(&name)).unwrap();
        }
    }
}

fn normalize(text: &str, tmp: &Path) -> String {
    let mut out = text.to_string();
    let canonical = std::fs::canonicalize(tmp).unwrap_or_else(|_| tmp.to_path_buf());
    for p in [canonical, tmp.to_path_buf()] {
        out = out.replace(&p.display().to_string(), "<tmp>");
    }
    out.replace("0.1.4", "<version>")
        .replace(env!("CARGO_PKG_VERSION"), "<version>")
}

/// `metagente check <file>` from inside `dir`, in the format of the golden files.
fn check_here(dir: &Path, file: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_metagente"))
        .current_dir(dir)
        .args(["check", file])
        .output()
        .unwrap();
    format!(
        "exit: {}\n--- stdout\n{}--- stderr\n{}",
        out.status.code().unwrap_or(-1),
        normalize(&String::from_utf8_lossy(&out.stdout), dir),
        normalize(&String::from_utf8_lossy(&out.stderr), dir)
    )
}

/// `metagente run <args>` from inside `dir`, in the format of the golden files.
fn run_here(dir: &Path, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_metagente"))
        .current_dir(dir)
        .arg("run")
        .args(args)
        .output()
        .unwrap();
    format!(
        "exit: {}\n--- stdout\n{}--- stderr\n{}",
        out.status.code().unwrap_or(-1),
        normalize(&String::from_utf8_lossy(&out.stdout), dir),
        normalize(&String::from_utf8_lossy(&out.stderr), dir)
    )
}

fn golden(name: &str) -> String {
    let path = root().join("tests/fixtures/v0.1.4-golden").join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e))
}

fn goldens_with_prefix(prefix: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(root().join("tests/fixtures/v0.1.4-golden"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with(prefix))
        .collect();
    names.sort();
    names
}

#[test]
fn every_example_gives_the_result_it_gave_on_0_1_4() {
    let tmp = tempfile::tempdir().unwrap();
    copy_folder(&root().join("examples"), tmp.path());
    let names = goldens_with_prefix("examples-");
    assert!(names.len() >= 12, "the examples were frozen: {:?}", names);
    for name in names {
        let file = name
            .strip_prefix("examples-")
            .unwrap()
            .strip_suffix(".txt")
            .unwrap();
        assert!(
            tmp.path().join(file).is_file(),
            "examples/{} was removed",
            file
        );
        assert_eq!(
            check_here(tmp.path(), file),
            golden(&name),
            "examples/{}",
            file
        );
    }
}

#[test]
fn every_sample_gives_the_result_it_gave_on_0_1_4() {
    let samples = root().join("multi-agent-samples");
    let mut dirs: Vec<String> = std::fs::read_dir(&samples)
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    dirs.sort();
    let mut checked = 0;
    for name in goldens_with_prefix("samples-") {
        let dir = dirs
            .iter()
            .filter(|d| name.starts_with(&format!("samples-{}-", d)))
            .max_by_key(|d| d.len())
            .unwrap_or_else(|| panic!("no sample folder for {}", name));
        let file = name
            .strip_prefix(&format!("samples-{}-", dir))
            .unwrap()
            .strip_suffix(".txt")
            .unwrap();
        let tmp = tempfile::tempdir().unwrap();
        copy_folder(&samples.join(dir), tmp.path());
        assert_eq!(
            check_here(tmp.path(), file),
            golden(&name),
            "multi-agent-samples/{}/{}",
            dir,
            file
        );
        checked += 1;
    }
    assert!(
        checked >= 11,
        "the samples were frozen, checked {}",
        checked
    );
}

#[test]
fn every_block_of_the_0_1_4_docs_gives_the_result_it_gave_on_0_1_4() {
    let blocks = root().join("tests/fixtures/v0.1.4-blocks");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&blocks)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    assert!(files.len() >= 20, "the blocks were frozen: {}", files.len());
    for block in files {
        let name = block.file_name().unwrap().to_string_lossy().to_string();
        let tmp = tempfile::tempdir().unwrap();
        let target = if name.ends_with(".toml") {
            std::fs::copy(&block, tmp.path().join("metagente.toml")).unwrap();
            std::fs::write(tmp.path().join("hello.ag"), HELLO).unwrap();
            "hello.ag"
        } else {
            std::fs::copy(&block, tmp.path().join("block.ag")).unwrap();
            "block.ag"
        };
        assert_eq!(
            check_here(tmp.path(), target),
            golden(&format!("{}.txt", name)),
            "block {}",
            name
        );
    }
}

/// The samples of the release zip are the examples of the same names. The zip is not in the repository (dist/ is
/// ignored), so this only runs where a person has one.
#[test]
fn the_samples_of_the_release_zip_are_the_examples() {
    let zip = root().join("dist/metagente-linux-amd64.zip");
    if !zip.is_file() {
        eprintln!(
            "note: {} is not here; the zip samples were not compared",
            zip.display()
        );
        return;
    }
    for name in [
        "planner.ag",
        "weather.ag",
        "remote_weather.ag",
        "read_file.ag",
        "clock.ag",
    ] {
        let out = Command::new("unzip")
            .args([
                "-p",
                &zip.display().to_string(),
                &format!("metagente/samples/{}", name),
            ])
            .output();
        let Ok(out) = out else {
            eprintln!("note: unzip is not available; the zip samples were not compared");
            return;
        };
        if !out.status.success() {
            continue;
        }
        let example = std::fs::read(root().join("examples").join(name)).unwrap();
        assert_eq!(
            out.stdout, example,
            "samples/{} of the zip differs from examples/{}",
            name, name
        );
    }
}

/// The examples that run offline with a result that does not depend on the clock, the network or a tool server give the
/// same output as on 0.1.4 (what `run` prints). The others (clock.ag, weather.ag, planner.ag, remote_weather.ag,
/// parameters.ag) are only checked, and are listed in specs/007-multiple-llm-models/validation.md.
#[test]
fn the_examples_that_run_offline_print_what_they_printed_on_0_1_4() {
    let cases: [(&str, &[&str]); 7] = [
        ("broken_permission.ag", &[]),
        ("broken_syntax.ag", &[]),
        ("broken_undeclared_tool.ag", &[]),
        ("cycle_a.ag", &[]),
        ("cycle_b.ag", &[]),
        ("multiline.ag", &["screen", "resume=text"]),
        ("read_file.ag", &["summarize"]),
    ];
    for (file, extra) in cases {
        let tmp = tempfile::tempdir().unwrap();
        copy_folder(&root().join("examples"), tmp.path());
        std::fs::write(tmp.path().join("notes.txt"), "hello notes\n").unwrap();
        let mut args = vec![file];
        args.extend_from_slice(extra);
        assert_eq!(
            run_here(tmp.path(), &args),
            golden(&format!("run-examples-{}.txt", file)),
            "run of examples/{}",
            file
        );
    }
}
