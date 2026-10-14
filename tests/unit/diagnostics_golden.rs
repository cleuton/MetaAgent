//! Every kind of problem must read as plain language: where, what, and how to fix it.

use metagente::diagnostics::Diagnostic;
use metagente::lang::check::check_all;
use metagente::lang::parse_file;
use metagente::runtime::Runtime;
use metagente::runtime::config::Config;
use metagente::runtime::interpreter::Agent;
use metagente::runtime::task::{CallContext, new_task_id};
use metagente::runtime::value::Value;
use std::sync::Arc;

const FORBIDDEN: &[&str] = &[
    "unwrap",
    "panicked",
    "RUST_BACKTRACE",
    "Result<",
    "Option<",
    "Some(",
    "Err(",
    "::",
    ".rs:",
    "src/",
    "thread '",
    "stack backtrace",
    "Box<",
    "Arc<",
    "std::",
];

fn assert_plain(rendered: &str) {
    for word in FORBIDDEN {
        assert!(
            !rendered.contains(word),
            "found `{}` in:\n{}",
            word,
            rendered
        );
    }
}

fn located(d: &Diagnostic) -> String {
    let text = d.render();
    assert!(
        text.starts_with("Problem on line "),
        "no line reference:\n{}",
        text
    );
    assert!(text.contains("\nFix: "), "no suggested fix:\n{}", text);
    assert_plain(&text);
    text
}

fn problems(source: &str) -> Vec<Diagnostic> {
    match parse_file("golden.ag", None, source) {
        Ok(agents) => check_all(&agents.into_iter().map(Arc::new).collect::<Vec<_>>()),
        Err(d) => vec![d],
    }
}

#[test]
fn syntax_error() {
    let text = located(&problems("agent A\n  gaol \"x\"\n")[0]);
    assert!(text.contains("did you mean `goal`?"), "{}", text);
}

#[test]
fn undeclared_name() {
    let text =
        located(&problems("agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply nme\n")[0]);
    assert!(
        text.contains("I do not know what `nme` is here"),
        "{}",
        text
    );
}

#[test]
fn undeclared_tool() {
    let text = located(
        &problems(
            "agent A\n  goal \"x\"\n  accepts go\n  on go\n    x = file.read path: \"a\"\n    reply x\n",
        )[0],
    );
    assert!(text.contains("never declared it"), "{}", text);
    assert!(text.contains("tool file"), "{}", text);
}

#[test]
fn bad_parameter_for_a_builtin_tool() {
    let text = located(
        &problems(
            "agent A\n  goal \"x\"\n  tool file\n  accepts go\n  on go\n    x = file.read pth: \"a\"\n    reply x\n",
        )[0],
    );
    assert!(text.contains("needs a value for `path`"), "{}", text);
}

#[test]
fn unknown_action_of_a_builtin_tool() {
    let text = located(
        &problems(
            "agent A\n  goal \"x\"\n  tool file\n  accepts go\n  on go\n    x = file.reed path: \"a\"\n    reply x\n",
        )[0],
    );
    assert!(text.contains("did you mean `file.read`?"), "{}", text);
}

#[test]
fn handler_without_accepts_and_accepts_without_handler() {
    let all = problems("agent A\n  goal \"x\"\n  accepts one\n  on two\n    reply \"x\"\n");
    assert_eq!(all.len(), 2);
    for d in &all {
        located(d);
    }
}

#[test]
fn missing_goal() {
    let text = located(&problems("agent A\n  accepts go\n  on go\n    reply \"x\"\n")[0]);
    assert!(text.contains("has no goal"), "{}", text);
}

#[tokio::test]
async fn unknown_message_and_missing_or_extra_values() {
    let dir = tempfile::tempdir().unwrap();
    let rt = Runtime::new(
        Config {
            root: dir.path().to_path_buf(),
            ..Config::default()
        },
        None,
    );
    let source = "agent A\n  goal \"x\"\n  accepts ask city\n  on ask\n    reply city\n";
    let def = Arc::new(parse_file("golden.ag", None, source).unwrap().remove(0));
    let agent = Agent::new(rt, def).unwrap();
    let ctx = CallContext::new(new_task_id());
    let no_args = std::collections::BTreeMap::new();
    let mut wrong = std::collections::BTreeMap::new();
    wrong.insert("cty".to_string(), Value::text("x"));
    for (message, args, expected) in [
        ("aks", no_args.clone(), "did you mean `ask`?"),
        ("ask", no_args, "needs a value for `city`"),
        ("ask", wrong, "needs a value for `city`"),
    ] {
        let text = agent
            .handle(message, args, &ctx)
            .await
            .unwrap_err()
            .render();
        assert!(text.contains(expected), "{}", text);
        assert!(text.contains("Fix: "), "{}", text);
        assert_plain(&text);
    }
}

#[tokio::test]
async fn missing_language_model() {
    let dir = tempfile::tempdir().unwrap();
    let rt = Runtime::new(
        Config {
            root: dir.path().to_path_buf(),
            ..Config::default()
        },
        None,
    );
    let source = "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply think \"hi\"\n";
    let def = Arc::new(parse_file("golden.ag", None, source).unwrap().remove(0));
    let agent = Agent::new(rt, def).unwrap();
    let d = agent
        .handle("go", Default::default(), &CallContext::new(new_task_id()))
        .await
        .unwrap_err();
    let text = located(&d);
    assert!(text.contains("needs a language model"), "{}", text);
}

#[tokio::test]
async fn runtime_errors_point_at_the_line() {
    let dir = tempfile::tempdir().unwrap();
    let rt = Runtime::new(
        Config {
            root: dir.path().to_path_buf(),
            ..Config::default()
        },
        None,
    );
    let source = "agent A\n  goal \"x\"\n  tool file\n  accepts go\n  on go\n    x = file.read path: \"nope.txt\"\n    reply x\n";
    let def = Arc::new(parse_file("golden.ag", None, source).unwrap().remove(0));
    let agent = Agent::new(rt, def).unwrap();
    let d = agent
        .handle("go", Default::default(), &CallContext::new(new_task_id()))
        .await
        .unwrap_err();
    let text = located(&d);
    assert!(text.contains("line 6"), "{}", text);
    assert!(text.contains("does not exist"), "{}", text);
}

// 0.1.2: the new problems read like every other one: the line, what went wrong, a fix, no Rust text

#[test]
fn text_that_is_never_closed() {
    let text = located(
        &problems(
            "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply \"\"\"never ends\n    reply \"b\"\n",
        )[0],
    );
    assert!(text.starts_with("Problem on line 5"), "{}", text);
    assert!(
        text.contains("the text that starts on line 5 was never closed"),
        "{}",
        text
    );
    assert!(
        text.contains("Fix: end it with three quotes, like this: \"\"\""),
        "{}",
        text
    );
}

#[test]
fn an_error_after_a_multi_line_text_names_the_right_line() {
    let source = "agent A\n  goal \"x\"\n  accepts go\n  on go\n    a = \"\"\"one\ntwo\nthree\"\"\"\n    reply nme\n";
    let text = located(&problems(source)[0]);
    assert!(text.starts_with("Problem on line 8"), "{}", text);
    assert!(
        text.contains("I do not know what `nme` is here"),
        "{}",
        text
    );
}

// 0.1.2: parse with the parameters given, as `metagente run` and `check` do
fn problems_with(source: &str, params: &metagente::lang::Params) -> Vec<Diagnostic> {
    match metagente::lang::parse_file_with("golden.ag", None, source, params) {
        Ok(agents) => check_all(&agents.into_iter().map(Arc::new).collect::<Vec<_>>()),
        Err(d) => vec![d],
    }
}

#[test]
fn a_parameter_that_does_not_exist() {
    let text = located(
        &problems_with(
            "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply @parameters.missing\n",
            &Default::default(),
        )[0],
    );
    assert!(text.starts_with("Problem on line 5"), "{}", text);
    assert!(
        text.contains("there is no parameter called `missing`"),
        "{}",
        text
    );
    assert!(
        text.contains(
            "Fix: add it under [parameters] in metagente.toml, for example: missing = \"some text\""
        ),
        "{}",
        text
    );
}

#[test]
fn a_parameter_that_is_not_text() {
    let mut params = metagente::lang::Params::default();
    params.not_text.insert("retries".to_string());
    let text = located(
        &problems_with(
            "agent A\n  goal @parameters.retries\n  accepts go\n  on go\n    reply \"x\"\n",
            &params,
        )[0],
    );
    assert!(
        text.contains("the parameter `retries` is not text"),
        "{}",
        text
    );
    assert!(
        text.contains("Fix: write its value in quotes in metagente.toml"),
        "{}",
        text
    );
}

#[test]
fn a_badly_written_parameter() {
    for bad in [
        "@parameters",
        "@other.name",
        "@parameters.",
        "@parameters.a.b",
    ] {
        let source = format!(
            "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply {}\n",
            bad
        );
        let text = located(&problems_with(&source, &Default::default())[0]);
        assert!(
            text.contains(&format!("I do not understand `{}`", bad)),
            "{}",
            text
        );
        assert!(
            text.contains("Fix: parameters are written like this: @parameters.name"),
            "{}",
            text
        );
    }
}

#[test]
fn a_parameter_with_braces_that_are_not_a_name() {
    let mut params = metagente::lang::Params::default();
    params
        .values
        .insert("p".to_string(), "bad {not a name}".to_string());
    let text = located(
        &problems_with(
            "agent A\n  goal \"x\"\n  accepts go\n  on go\n    reply @parameters.p\n",
            &params,
        )[0],
    );
    assert!(
        text.contains("the parameter `p` has `{not a name}` in it"),
        "{}",
        text
    );
}

// 0.1.3: the messages of the secure list read as plain language (name, address, line, fix; no Rust or TLS text)
mod secure_messages {
    use super::{assert_plain, located};
    use metagente::lang::parse_file;
    use metagente::runtime::config::SecureConfig;
    use metagente::runtime::secure::{Caller, Target, check_agents, decide};
    use std::sync::Arc;

    fn list(agents: &[&str], tools: &[&str]) -> SecureConfig {
        SecureConfig {
            agents: agents.iter().map(|s| s.to_string()).collect(),
            tools: tools.iter().map(|s| s.to_string()).collect(),
            ..SecureConfig::default()
        }
    }

    fn agents(source: &str) -> Vec<Arc<metagente::lang::AgentDef>> {
        parse_file("golden.ag", None, source)
            .unwrap()
            .into_iter()
            .map(Arc::new)
            .collect()
    }

    #[test]
    fn s1_a_secure_remote_with_an_http_address() {
        let a = agents(
            "agent A\n  goal \"x\"\n  remote Bob at \"http://host:8443\"\n  accepts go\n  on go\n    reply \"x\"\n",
        );
        let found = check_agents(&a, &list(&["Bob"], &[]));
        assert_eq!(found.len(), 1);
        let text = located(&found[0]);
        assert!(text.contains("Bob is declared secure in metagente.toml but this address uses http: http://host:8443"), "{}", text);
        assert!(text.contains("remove Bob from [secure] agents"), "{}", text);
    }

    #[test]
    fn s1_a_secure_tool_with_an_http_address() {
        let a = agents(
            "agent A\n  goal \"x\"\n  tool kb from mcp \"http://h:9/mcp\"\n  accepts go\n  on go\n    reply \"x\"\n",
        );
        let found = check_agents(&a, &list(&[], &["kb"]));
        assert_eq!(found.len(), 1);
        let text = located(&found[0]);
        assert!(text.contains("kb is declared secure"), "{}", text);
        assert!(text.contains("[secure] tools"), "{}", text);
    }

    #[test]
    fn s1_the_builtin_http_tool_with_a_literal_http_address() {
        let a = agents(
            "agent A\n  goal \"x\"\n  tool http\n  accepts go\n  on go\n    r = http.get url: \"http://example.com\"\n    reply r.status\n",
        );
        let found = check_agents(&a, &list(&[], &["http"]));
        assert_eq!(found.len(), 1);
        located(&found[0]);
    }

    #[test]
    fn s2_a_secure_agent_with_an_http_address() {
        let a = agents(
            "agent Researcher\n  goal \"x\"\n  remote Facts at \"http://h:1\"\n  accepts go\n  on go\n    reply \"x\"\n",
        );
        let found = check_agents(&a, &list(&["Researcher"], &[]));
        assert_eq!(found.len(), 1);
        let text = located(&found[0]);
        assert!(
            text.contains("Researcher is declared secure, so it may only call https addresses"),
            "{}",
            text
        );
    }

    #[test]
    fn s8_the_model_address_with_llm_on() {
        let l = SecureConfig {
            llm: true,
            ..SecureConfig::default()
        };
        let e = decide(
            &l,
            &Caller::new("A", false),
            Target::Model,
            "http://gateway",
        )
        .unwrap_err();
        let text = format!("{}\nFix: {}", e.message, e.fix.unwrap());
        assert!(text.contains(
            "The model address in [llm] uses http, and [secure] llm is on. Nothing was sent."
        ));
        assert_plain(&text);
    }

    #[test]
    fn a_literal_http_address_in_a_free_agent_is_not_reported() {
        let a = agents(
            "agent A\n  goal \"x\"\n  remote Bob at \"http://host:8443\"\n  accepts go\n  on go\n    reply \"x\"\n",
        );
        assert!(check_agents(&a, &SecureConfig::default()).is_empty());
    }

    // the serving certificate: S4 and S9 (and the expiry message S10) read as plain language
    mod serving_certificate {
        use super::assert_plain;
        use metagente::runtime::config::ServeTlsConfig;
        use metagente::runtime::tls::{Pair, load, resolve};
        use std::path::PathBuf;

        fn text(d: &metagente::diagnostics::Diagnostic) -> String {
            let t = d.render();
            assert_plain(&t);
            assert!(t.contains("Fix: "), "no suggested fix:\n{}", t);
            t
        }

        fn pair(dir: &std::path::Path) -> Pair {
            Pair {
                cert: dir.join("cert.pem"),
                key: dir.join("key.pem"),
                cert_at: Some(("metagente.toml".to_string(), 2)),
                key_at: Some(("metagente.toml".to_string(), 3)),
            }
        }

        #[test]
        fn s4_a_certificate_without_a_key_and_the_reverse() {
            let only_cert = ServeTlsConfig {
                cert: Some(PathBuf::from("cert.pem")),
                cert_line: 2,
                file: "metagente.toml".to_string(),
                ..ServeTlsConfig::default()
            };
            let e = resolve(None, None, &only_cert).err().unwrap();
            let t = text(&e);
            assert!(
                t.contains("I have a certificate but no key (tls_key)."),
                "{}",
                t
            );
            assert!(t.contains("--tls-key"), "{}", t);
            let only_key = ServeTlsConfig {
                key: Some(PathBuf::from("key.pem")),
                key_line: 3,
                file: "metagente.toml".to_string(),
                ..ServeTlsConfig::default()
            };
            let t = text(&resolve(None, None, &only_key).err().unwrap());
            assert!(
                t.contains("I have a key but no certificate (tls_cert)."),
                "{}",
                t
            );
        }

        #[test]
        fn s9_files_that_are_missing_or_not_pem() {
            let dir = tempfile::tempdir().unwrap();
            let t = text(&load(&pair(dir.path())).err().unwrap());
            assert!(t.contains("I could not read the certificate file"), "{}", t);
            assert!(t.contains("the file does not exist"), "{}", t);
            assert!(
                t.starts_with("Problem on line 2 of metagente.toml"),
                "{}",
                t
            );
            std::fs::write(dir.path().join("cert.pem"), "not pem").unwrap();
            std::fs::write(dir.path().join("key.pem"), "not pem").unwrap();
            let t = text(&load(&pair(dir.path())).err().unwrap());
            assert!(t.contains("is not a PEM certificate file"), "{}", t);
        }

        #[test]
        fn s9_and_s10_a_key_that_does_not_match_and_a_certificate_that_ran_out() {
            let dir = tempfile::tempdir().unwrap();
            let a = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
            let b = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
            std::fs::write(dir.path().join("cert.pem"), a.cert.pem()).unwrap();
            std::fs::write(dir.path().join("key.pem"), b.signing_key.serialize_pem()).unwrap();
            let t = text(&load(&pair(dir.path())).err().unwrap());
            assert!(t.contains("does not match the certificate in"), "{}", t);
            let mut params = rcgen::CertificateParams::new(vec!["localhost".to_string()]).unwrap();
            params.not_before = rcgen::date_time_ymd(1999, 1, 1);
            params.not_after = rcgen::date_time_ymd(2000, 1, 1);
            let key = rcgen::KeyPair::generate().unwrap();
            let cert = params.self_signed(&key).unwrap();
            std::fs::write(dir.path().join("cert.pem"), cert.pem()).unwrap();
            std::fs::write(dir.path().join("key.pem"), key.serialize_pem()).unwrap();
            let t = text(&load(&pair(dir.path())).err().unwrap());
            assert!(t.contains("expired on 2000-01-01"), "{}", t);
            assert!(t.contains("make a new certificate and key"), "{}", t);
        }
    }
}
