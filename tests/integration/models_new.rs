// 0.1.5: new file (US6-3, FR-017). `metagente new` writes an active [version] with the version of the program that ran
// it and a commented example of a named model, which loads once uncommented.
use std::process::Command;

fn new_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_metagente"))
        .current_dir(dir.path())
        .args(["new", "hello"])
        .output()
        .unwrap();
    assert!(out.status.success());
    dir
}

fn toml_of(dir: &tempfile::TempDir) -> String {
    std::fs::read_to_string(dir.path().join("metagente.toml")).unwrap()
}

#[test]
fn the_new_metagente_toml_has_an_active_version_and_a_commented_named_model() {
    let dir = new_project();
    let toml = toml_of(&dir);
    assert!(
        toml.contains(&format!(
            "[version]\nminimum = \"{}\"",
            env!("CARGO_PKG_VERSION")
        )),
        "{}",
        toml
    );
    for line in [
        "# [llm.fast]",
        "# provider = \"anthropic\"",
        "# model_env = \"FAST_MODEL\"",
        "# api_key_env = \"FAST_KEY\"",
        "think \"...\" using fast",
    ] {
        assert!(toml.contains(line), "missing `{}` in:\n{}", line, toml);
    }
    // nothing of the example is active
    let parsed: toml::Table = toml.parse().unwrap();
    assert!(parsed.get("llm").is_none(), "{:?}", parsed.get("llm"));
}

#[test]
fn the_new_project_passes_check_and_prints_no_warning() {
    let dir = new_project();
    let out = Command::new(env!("CARGO_BIN_EXE_metagente"))
        .current_dir(dir.path())
        .args(["check", "hello.ag"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(!String::from_utf8_lossy(&out.stderr).contains("Warning"));
}

#[test]
fn uncommenting_the_example_loads_and_an_agent_can_use_it() {
    let dir = new_project();
    let toml = toml_of(&dir)
        .replace("# [llm.fast]", "[llm.fast]")
        .replace("# provider = \"anthropic\"               # or \"openai\"", "provider = \"anthropic\"")
        .replace("# model_env = \"FAST_MODEL\"             # the NAME of a variable that holds the model name (or: model = \"...\")", "model_env = \"FAST_MODEL\"")
        .replace("# api_key_env = \"FAST_KEY\"             # the NAME of the variable that holds the key", "api_key_env = \"FAST_KEY\"");
    assert!(toml.contains("\n[llm.fast]\nprovider = \"anthropic\"\nmodel_env = \"FAST_MODEL\"\napi_key_env = \"FAST_KEY\"\n"), "{}", toml);
    std::fs::write(dir.path().join("metagente.toml"), toml).unwrap();
    std::fs::write(
        dir.path().join("ask.ag"),
        "agent Ask\n  goal \"x\"\n  accepts go\n  on go\n    reply think \"hi\" using fast\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_metagente"))
        .current_dir(dir.path())
        .args(["check", "ask.ag"])
        .env_remove("FAST_KEY")
        .env_remove("FAST_MODEL")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // the project has [version] already, so only the unset variables are mentioned
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("the model fast needs the variable FAST_KEY"),
        "{}",
        err
    );
    assert!(!err.contains("[version]"), "{}", err);
}
