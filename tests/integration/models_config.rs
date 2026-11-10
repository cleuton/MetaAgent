// 0.1.5: new file (US5, FR-004). Every mistake of an `[llm.NAME]` stops `check`, `run` and `serve` with a plain message,
// the line of metagente.toml and the fix (contracts/metagente-toml-models.md). Nothing from the file is printed except a
// safe name (D10).
use crate::common::self_signed::serve_refused;
use std::process::Command;

const HI: &str = "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply \"hi\"\n";

struct Out {
    ok: bool,
    text: String,
}

fn run(toml: &str, args: &[&str]) -> Out {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("metagente.toml"), toml).unwrap();
    std::fs::write(dir.path().join("a.ag"), HI).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_metagente"))
        .current_dir(dir.path())
        .args(args)
        .output()
        .unwrap();
    Out {
        ok: out.status.success(),
        text: format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    }
}

/// `check a.ag` and `run a.ag go` must both stop with this message, at this line, with this fix.
fn refused(toml: &str, line: usize, message: &str, fix: &str) {
    for args in [vec!["check", "a.ag"], vec!["run", "a.ag", "go"]] {
        let out = run(toml, &args);
        assert!(!out.ok, "{:?} must stop for:\n{}\n{}", args, toml, out.text);
        assert!(
            out.text.contains(&format!("Problem on line {} of ", line)),
            "{:?} line {}:\n{}",
            args,
            line,
            out.text
        );
        assert!(
            out.text.contains(message),
            "{:?} says:\n{}\nwanted: {}",
            args,
            out.text,
            message
        );
        assert!(
            out.text.contains(&format!("Fix: {}", fix)) || out.text.contains(fix),
            "{:?}:\n{}\nwanted fix: {}",
            args,
            out.text,
            fix
        );
    }
}

#[test]
fn a_provider_that_is_not_anthropic_or_openai_is_refused_with_the_closest_name() {
    refused(
        "[llm.fast]\nprovider = \"anthropik\"\n",
        2,
        "The provider anthropik in [llm.fast] is not one a named model accepts. Use anthropic or openai.",
        "did you mean anthropic?",
    );
    refused(
        "[llm.fast]\nprovider = \"openai-compatible\"\nmodel = \"m\"\n",
        2,
        "The provider openai-compatible in [llm.fast] is not one a named model accepts. Use anthropic or openai.",
        "another address use the [llm] section",
    );
    refused(
        "[llm.fast]\nprovider = \"gemini\"\n",
        2,
        "The provider gemini in [llm.fast] is not one a named model accepts.",
        "use provider = \"anthropic\" or \"openai\"",
    );
}

#[test]
fn a_missing_provider_is_refused() {
    refused(
        "[llm.fast]\nmodel = \"x\"\n",
        1,
        "The provider is missing in [llm.fast].",
        "add provider = \"anthropic\" or \"openai\"",
    );
}

#[test]
fn a_misspelled_setting_is_refused_with_the_closest_name() {
    refused(
        "[llm.fast]\nprovider = \"anthropic\"\napi_key_evn = \"X\"\n",
        3,
        "[llm.fast] has the setting api_key_evn, which I do not know.",
        "did you mean api_key_env?",
    );
    refused(
        "[llm.fast]\nprovider = \"anthropic\"\ntemperature = \"hot\"\n",
        3,
        "[llm.fast] has the setting temperature, which I do not know.",
        "the settings of a named model are provider, model, model_env and api_key_env",
    );
}

#[test]
fn a_table_inside_a_sub_table_is_an_unknown_setting() {
    refused(
        "[llm.fast]\nprovider = \"anthropic\"\n\n[llm.fast.extra]\nx = 1\n",
        4,
        "[llm.fast] has the setting extra, which I do not know.",
        "the settings of a named model",
    );
}

#[test]
fn model_and_model_env_together_are_refused() {
    refused(
        "[llm.fast]\nprovider = \"anthropic\"\nmodel = \"a\"\nmodel_env = \"B\"\n",
        4,
        "[llm.fast] has both model and model_env.",
        "give only one",
    );
}

#[test]
fn a_key_written_in_the_file_is_refused_for_each_of_the_six_names() {
    for name in ["key", "api_key", "apikey", "token", "secret", "password"] {
        refused(
            &format!("[llm.fast]\nprovider = \"anthropic\"\n{} = \"abc\"\n", name),
            3,
            "[llm.fast] has a key written in the file. Keys are never kept here.",
            "put the key in an environment variable and name it in api_key_env",
        );
    }
}

#[test]
fn an_address_in_a_named_model_is_refused() {
    for name in ["base_url", "base_url_env"] {
        refused(
            &format!(
                "[llm.fast]\nprovider = \"openai\"\nmodel = \"m\"\n{} = \"https://host/v1\"\n",
                name
            ),
            4,
            &format!(
                "[llm.fast] has {}. A named model always uses the official address.",
                name
            ),
            "for another address, use the [llm] section",
        );
    }
}

#[test]
fn openai_needs_a_model_name_and_anthropic_does_not() {
    refused(
        "[llm.gpt]\nprovider = \"openai\"\n",
        1,
        "The provider openai needs a model name in [llm.gpt].",
        "add model = \"...\"",
    );
    let ok = run(
        "[llm.claude]\nprovider = \"anthropic\"\n",
        &["check", "a.ag"],
    );
    assert!(ok.ok, "{}", ok.text);
    let ok = run(
        "[llm.gpt]\nprovider = \"openai\"\nmodel_env = \"GPT_MODEL\"\n",
        &["check", "a.ag"],
    );
    assert!(ok.ok, "{}", ok.text);
}

#[test]
fn a_variable_name_with_a_wrong_shape_is_refused_without_repeating_it() {
    refused(
        "[llm.fast]\nprovider = \"anthropic\"\napi_key_env = \"FAST-KEY\"\n",
        3,
        "api_key_env in [llm.fast] must be the name of a variable, such as FAST_KEY.",
        "write only the name of the variable; the key itself stays in the environment",
    );
    refused(
        "[llm.fast]\nprovider = \"anthropic\"\nmodel_env = \"1MODEL\"\n",
        3,
        "model_env in [llm.fast] must be the name of a variable, such as FAST_MODEL.",
        "write only the name of the variable",
    );
    let out = run(
        "[llm.fast]\nprovider = \"anthropic\"\napi_key_env = \"FAST-KEY\"\n",
        &["check", "a.ag"],
    );
    assert!(!out.text.contains("FAST-KEY"), "{}", out.text);
}

#[test]
fn a_value_that_is_not_text_or_is_empty_is_refused() {
    refused(
        "[llm.fast]\nprovider = 5\n",
        2,
        "[llm.fast] setting provider must be text that is not empty.",
        "write it between quotes",
    );
    refused(
        "[llm.fast]\nprovider = \"anthropic\"\nmodel = \"\"\n",
        3,
        "[llm.fast] setting model must be text that is not empty.",
        "write it between quotes",
    );
    refused(
        "[llm.fast]\nprovider = \"anthropic\"\napi_key_env = \"  \"\n",
        3,
        "[llm.fast] setting api_key_env must be text that is not empty.",
        "write it between quotes",
    );
}

#[test]
fn a_setting_of_llm_or_a_word_of_the_language_cannot_be_a_nickname() {
    refused(
        "[llm.model]\nprovider = \"anthropic\"\n",
        1,
        "[llm.model] cannot be a model name, because model is a setting of [llm].",
        "choose another name, such as fast",
    );
    refused(
        "[llm.reply]\nprovider = \"anthropic\"\n",
        1,
        "[llm.reply] cannot be a model name, because reply is a word of the language.",
        "choose another name, such as fast",
    );
    refused(
        "[llm.using]\nprovider = \"anthropic\"\n",
        1,
        "[llm.using] cannot be a model name, because using is a word of the language.",
        "choose another name, such as fast",
    );
}

#[test]
fn a_nickname_with_a_wrong_shape_is_refused_and_an_unsafe_one_is_not_shown() {
    refused(
        "[llm.1fast]\nprovider = \"anthropic\"\n",
        1,
        "[llm.1fast] is not a name I can use for a model",
        "use a short name made of letters, digits, _ and -, such as fast",
    );
    refused(
        "[llm.a234567890123456789012345]\nprovider = \"anthropic\"\n",
        1,
        "[llm] has a name that is too long or has unusual characters (line 1).",
        "use a short name made of letters, digits, _ and -, such as fast",
    );
    let quoted = run(
        "[llm.\"fast model\"]\nprovider = \"anthropic\"\n",
        &["check", "a.ag"],
    );
    assert!(!quoted.ok);
    assert!(
        quoted
            .text
            .contains("[llm] has a name that is too long or has unusual characters (line 1)."),
        "{}",
        quoted.text
    );
    assert!(!quoted.text.contains("fast model"), "{}", quoted.text);
}

#[test]
fn a_nickname_defined_twice_is_a_mistake_of_the_toml_itself() {
    let out = run(
        "[llm.fast]\nprovider = \"anthropic\"\n\n[llm.fast]\nprovider = \"openai\"\nmodel = \"m\"\n",
        &["run", "a.ag", "go"],
    );
    assert!(!out.ok);
    assert!(out.text.contains("is not valid"), "{}", out.text);
}

#[test]
fn check_ignores_a_metagente_toml_that_is_not_toml_at_all_as_before() {
    let out = run("[llm.fast\nprovider = \n", &["check", "a.ag"]);
    assert!(out.ok, "{}", out.text);
    let out = run("[llm.fast\nprovider = \n", &["run", "a.ag", "go"]);
    assert!(!out.ok);
    assert!(out.text.contains("is not valid"), "{}", out.text);
}

#[test]
fn serve_refuses_to_start_for_the_same_mistakes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        "[llm.fast]\nprovider = \"anthropik\"\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("a.ag"), HI).unwrap();
    let out = serve_refused(dir.path(), &["a.ag".to_string()]);
    let text = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(!out.status.success());
    assert!(
        text.contains("The provider anthropik in [llm.fast] is not one a named model accepts."),
        "{}",
        text
    );
    assert!(text.contains("did you mean anthropic?"), "{}", text);
}

#[test]
fn a_long_variable_name_is_accepted_and_used_without_being_shown() {
    let name = "A_VERY_LONG_VARIABLE_NAME_OF_MORE_THAN_24";
    let toml = format!(
        "[llm.fast]\nprovider = \"anthropic\"\napi_key_env = \"{}\"\n",
        name
    );
    let out = run(&toml, &["check", "a.ag"]);
    assert!(out.ok, "{}", out.text);
}
