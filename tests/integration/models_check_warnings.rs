// 0.1.5: new file (US5-6, US5-13, FR-010, FR-016). `check` warns about an unset variable only for a named model that
// some `using` points to; a missing [version] is a warning on check, run and serve; a `using` with an unknown nickname
// stops with the models that exist and the closest name.
use crate::common::self_signed::{ServeProc, free_port};
use std::process::Command;

fn version_line() -> String {
    format!("[version]\nminimum = \"{}\"\n\n", env!("CARGO_PKG_VERSION"))
}

fn agent_using(nickname: &str) -> String {
    format!(
        "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply think \"hi\" using {}\n",
        nickname
    )
}

struct Out {
    ok: bool,
    stderr: String,
}

fn run(toml: &str, agent: &str, args: &[&str], env: &[(&str, &str)]) -> Out {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("metagente.toml"), toml).unwrap();
    std::fs::write(dir.path().join("a.ag"), agent).unwrap();
    let mut c = Command::new(env!("CARGO_BIN_EXE_metagente"));
    c.current_dir(dir.path()).args(args);
    for name in [
        "FAST_KEY",
        "FAST_MODEL",
        "STRONG_KEY",
        "STRONG_MODEL",
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
    ] {
        c.env_remove(name);
    }
    for (k, v) in env {
        c.env(k, v);
    }
    let out = c.output().unwrap();
    Out {
        ok: out.status.success(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    }
}

fn two_models() -> String {
    format!(
        "{}[llm.fast]\nprovider = \"anthropic\"\nmodel_env = \"FAST_MODEL\"\napi_key_env = \"FAST_KEY\"\n\n[llm.strong]\nprovider = \"openai\"\nmodel_env = \"STRONG_MODEL\"\napi_key_env = \"STRONG_KEY\"\n",
        version_line()
    )
}

#[test]
fn check_warns_about_an_unset_variable_of_a_used_model_and_still_succeeds() {
    let out = run(&two_models(), &agent_using("fast"), &["check", "a.ag"], &[]);
    assert!(out.ok, "{}", out.stderr);
    assert!(
        out.stderr.contains(
            "Warning: the model fast needs the variable FAST_KEY, and it is not set here."
        ),
        "{}",
        out.stderr
    );
    assert!(
        out.stderr.contains("Warning: the model fast needs the variable FAST_MODEL, and it is empty or not set here."),
        "{}",
        out.stderr
    );
    // only the model that is used
    assert!(!out.stderr.contains("STRONG"), "{}", out.stderr);
}

#[test]
fn the_warning_is_gone_when_the_variables_are_set_and_an_empty_one_counts_as_unset() {
    let set = run(
        &two_models(),
        &agent_using("fast"),
        &["check", "a.ag"],
        &[("FAST_KEY", "k"), ("FAST_MODEL", "m")],
    );
    assert!(set.ok);
    assert!(!set.stderr.contains("Warning"), "{}", set.stderr);
    let empty = run(
        &two_models(),
        &agent_using("fast"),
        &["check", "a.ag"],
        &[("FAST_KEY", "  "), ("FAST_MODEL", "m")],
    );
    assert!(empty.ok);
    assert!(
        empty
            .stderr
            .contains("the model fast needs the variable FAST_KEY"),
        "{}",
        empty.stderr
    );
}

#[test]
fn a_model_that_nothing_uses_and_a_plain_think_get_no_warning() {
    let toml = format!(
        "{}[llm]\nprovider = \"anthropic\"\n\n[llm.unused]\nprovider = \"openai\"\nmodel_env = \"UNUSED_MODEL\"\napi_key_env = \"UNUSED_KEY\"\n",
        version_line()
    );
    let plain = "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply think \"hi\"\n";
    let out = run(&toml, plain, &["check", "a.ag"], &[]);
    assert!(out.ok, "{}", out.stderr);
    assert!(!out.stderr.contains("Warning"), "{}", out.stderr);
    assert!(!out.stderr.contains("UNUSED"), "{}", out.stderr);
}

#[test]
fn a_long_variable_name_is_not_shown_in_the_warning() {
    let toml = format!(
        "{}[llm.fast]\nprovider = \"anthropic\"\nmodel = \"m\"\napi_key_env = \"A_VERY_LONG_VARIABLE_NAME_OF_MORE_THAN_24\"\n",
        version_line()
    );
    let out = run(&toml, &agent_using("fast"), &["check", "a.ag"], &[]);
    assert!(out.ok);
    assert!(
        out.stderr
            .contains("the model fast needs the variable named on line 7, and it is not set here."),
        "{}",
        out.stderr
    );
    assert!(!out.stderr.contains("A_VERY_LONG"), "{}", out.stderr);
}

const NO_VERSION: &str =
    "[llm.fast]\nprovider = \"anthropic\"\nmodel = \"m\"\napi_key_env = \"FAST_KEY\"\n";
const NO_VERSION_WARNING: &str = "Warning: [llm.fast] is set but [version] is not. Older programs do not know it. Add a [version] section with minimum = \"0.1.5\".";

#[test]
fn a_sub_table_without_version_is_a_warning_on_check_and_run_never_an_error() {
    let check = run(
        NO_VERSION,
        &agent_using("fast"),
        &["check", "a.ag"],
        &[("FAST_KEY", "k")],
    );
    assert!(check.ok, "{}", check.stderr);
    assert!(
        check.stderr.contains(NO_VERSION_WARNING),
        "{}",
        check.stderr
    );
    let plain = "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply \"hi\"\n";
    let ran = run(NO_VERSION, plain, &["run", "a.ag", "go"], &[]);
    assert!(ran.ok, "{}", ran.stderr);
    assert!(ran.stderr.contains(NO_VERSION_WARNING), "{}", ran.stderr);
    // with [version] there is no warning
    let with = run(
        &format!("{}{}", version_line(), NO_VERSION),
        plain,
        &["run", "a.ag", "go"],
        &[],
    );
    assert!(!with.stderr.contains("[version]"), "{}", with.stderr);
}

#[test]
fn serve_prints_the_missing_version_warning_at_start() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("metagente.toml"), NO_VERSION).unwrap();
    std::fs::write(
        dir.path().join("a.ag"),
        "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply \"hi\"\n",
    )
    .unwrap();
    let port = free_port();
    let serve = ServeProc::start(
        dir.path(),
        &["a.ag".to_string(), "--a2a".to_string(), port.to_string()],
        1,
    );
    assert!(
        serve.said().contains(NO_VERSION_WARNING),
        "{}",
        serve.said()
    );
}

#[test]
fn an_unknown_nickname_lists_the_models_and_suggests_the_closest() {
    for args in [vec!["check", "a.ag"], vec!["run", "a.ag", "go"]] {
        let out = run(&two_models(), &agent_using("fats"), &args, &[]);
        assert!(!out.ok, "{:?}", args);
        assert!(
            out.stderr.contains("Problem on line 5 of a.ag"),
            "{}",
            out.stderr
        );
        assert!(
            out.stderr
                .contains("There is no model called fats. The models are: fast, strong."),
            "{}",
            out.stderr
        );
        assert!(out.stderr.contains("did you mean fast?"), "{}", out.stderr);
    }
}

#[test]
fn an_unknown_nickname_with_no_models_says_how_to_add_one() {
    for args in [vec!["check", "a.ag"], vec!["run", "a.ag", "go"]] {
        let out = run(&version_line(), &agent_using("fast"), &args, &[]);
        assert!(!out.ok, "{:?}", args);
        assert!(
            out.stderr
                .contains("There is no model called fast. No model is defined under [llm]."),
            "{}",
            out.stderr
        );
        assert!(
            out.stderr
                .contains("add [llm.fast] with provider = \"anthropic\" or \"openai\""),
            "{}",
            out.stderr
        );
    }
}

#[test]
fn a_nickname_that_is_not_a_safe_name_is_not_shown() {
    let long = "a234567890123456789012345";
    let out = run(&two_models(), &agent_using(long), &["check", "a.ag"], &[]);
    assert!(!out.ok);
    assert!(
        out.stderr.contains(
            "There is no model with the name written after using. The models are: fast, strong."
        ),
        "{}",
        out.stderr
    );
    // the source line of the problem is the person's own `.ag` line, as for every error of the language
    assert!(out.stderr.contains("Problem on line 5"), "{}", out.stderr);
}
