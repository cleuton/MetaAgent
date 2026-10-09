// 0.1.3: new file. The [network] section: what is accepted, what is refused, and the warnings.
use std::path::Path;
use std::process::{Command, Output};

fn metagente(dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_metagente"));
    c.current_dir(dir);
    // the real environment of whoever runs the tests must not leak in
    for v in [
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
        "NO_PROXY",
        "no_proxy",
    ] {
        c.env_remove(v);
    }
    c
}

const AGENT: &str = "agent A\n  goal \"a\"\n  accepts go\n  on go\n    reply \"ok\"\n";

fn project(toml: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.ag"), AGENT).unwrap();
    std::fs::write(dir.path().join("metagente.toml"), toml).unwrap();
    dir
}

fn run(dir: &Path, command: &str) -> Output {
    metagente(dir).args([command, "a.ag"]).output().unwrap()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

#[test]
fn the_version_is_0_1_3() {
    let dir = tempfile::tempdir().unwrap();
    let out = metagente(dir.path()).arg("--version").output().unwrap();
    assert!(
        String::from_utf8_lossy(&out.stdout)
            .trim_end()
            .ends_with("0.1.3")
    );
}

#[test]
fn a_file_without_the_section_behaves_as_before() {
    let dir = project("[runtime]\ntimeout_seconds = 5\n");
    for command in ["check", "run"] {
        let o = run(dir.path(), command);
        assert!(o.status.success(), "{}: {}", command, err(&o));
        assert!(!err(&o).contains("Warning"), "{}", err(&o));
    }
}

#[test]
fn literal_logins_are_refused_by_every_command_with_the_line() {
    let cases = [
        (
            "[network.proxy]\nurl = \"http://proxy.example:3128\"\npassword = \"hunter2\"\n",
            3,
            "`password`",
        ),
        ("[network.proxy]\nusername = \"ann\"\n", 2, "`username`"),
        (
            "[network.proxy]\nurl = \"http://ann:hunter2@proxy.example:3128\"\n",
            2,
            "user name or password",
        ),
    ];
    for (toml, line, what) in cases {
        let dir = project(toml);
        for command in ["check", "run"] {
            let o = run(dir.path(), command);
            assert!(!o.status.success(), "{}", command);
            let text = err(&o);
            assert!(
                text.contains(&format!("line {}", line)),
                "{}: {}",
                command,
                text
            );
            assert!(text.contains(what), "{}: {}", command, text);
            assert!(text.contains("username_env"), "{}", text);
            assert!(
                !text.contains("hunter2"),
                "the password must not be repeated: {}",
                text
            );
        }
    }
}

#[test]
fn half_a_login_is_an_error_naming_the_missing_key() {
    let dir = project("[network.proxy]\nurl = \"http://p.example:1\"\nusername_env = \"U\"\n");
    let text = err(&run(dir.path(), "check"));
    assert!(
        text.contains("username_env but no password_env"),
        "{}",
        text
    );
    let dir = project("[network.proxy]\nurl = \"http://p.example:1\"\npassword_env = \"P\"\n");
    let text = err(&run(dir.path(), "check"));
    assert!(
        text.contains("password_env but no username_env"),
        "{}",
        text
    );
}

#[test]
fn a_login_variable_that_is_not_set_is_named_by_check() {
    let dir = project(
        "[network.proxy]\nurl = \"http://p.example:1\"\nusername_env = \"MG_NO_SUCH_USER\"\npassword_env = \"MG_NO_SUCH_PASS\"\n",
    );
    let o = run(dir.path(), "check");
    assert!(!o.status.success());
    assert!(err(&o).contains("`MG_NO_SUCH_USER`"), "{}", err(&o));
}

#[test]
fn wrong_types_and_bad_addresses_say_what_to_write() {
    let cases = [
        (
            "[network]\nallow_self_signed = \"yes\"\n",
            "must be true or false",
        ),
        (
            "[network]\nself_signed_hosts = \"localhost\"\n",
            "must be a list of texts",
        ),
        (
            "[network.proxy]\nurl = \"ftp://p.example\"\n",
            "not a proxy address I can use",
        ),
        ("[network.proxy]\nno_proxy = 3\n", "must be a list of texts"),
    ];
    for (toml, words) in cases {
        let dir = project(toml);
        let text = err(&run(dir.path(), "check"));
        assert!(text.contains(words), "{}: {}", toml, text);
    }
}

#[test]
fn ca_file_must_be_readable_and_is_found_next_to_metagente_toml() {
    let dir = project("[network]\nca_file = \"certs/missing.pem\"\n");
    let text = err(&run(dir.path(), "check"));
    assert!(
        text.contains("certs/missing.pem") || text.contains("certs\\missing.pem"),
        "{}",
        text
    );
    assert!(text.contains("line 2"), "{}", text);
    std::fs::create_dir(dir.path().join("certs")).unwrap();
    std::fs::write(dir.path().join("certs/missing.pem"), "not really").unwrap();
    assert!(run(dir.path(), "check").status.success());
}

#[test]
fn unknown_keys_are_noted_not_fatal() {
    let dir = project("[network]\nallow_everything = true\n[network.proxy]\nspeed = 1\n");
    let o = run(dir.path(), "run");
    assert!(o.status.success(), "{}", err(&o));
    assert!(err(&o).contains("allow_everything"), "{}", err(&o));
    assert!(
        err(&o).contains("`speed` in [network.proxy]"),
        "{}",
        err(&o)
    );
}

#[test]
fn certificate_warnings_appear_on_run_and_check_every_time() {
    let dir = project("[network]\nallow_self_signed = true\nllm_allow_self_signed = true\n");
    for command in ["run", "check"] {
        let text = err(&run(dir.path(), command));
        assert!(
            text.contains("certificate checks are OFF for agents and tools"),
            "{}: {}",
            command,
            text
        );
        assert!(text.contains("development only"), "{}", text);
        assert!(text.contains("language model connection"), "{}", text);
        assert!(
            text.contains("key will be sent to a server whose identity was not verified"),
            "{}",
            text
        );
        assert_eq!(text.matches("Warning:").count(), 2, "{}", text);
    }
}

#[test]
fn check_reports_the_proxy_source_and_the_pass_to_tools_warning() {
    let dir =
        project("[network.proxy]\nurl = \"http://proxy.example:3128\"\npass_to_tools = true\n");
    let mut c = metagente(dir.path());
    c.env("HTTPS_PROXY", "http://other.example:1")
        .args(["check", "a.ag"]);
    let o = c.output().unwrap();
    assert!(o.status.success(), "{}", err(&o));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        out.contains("proxy: from metagente.toml (http://proxy.example:3128)"),
        "{}",
        out
    );
    assert!(out.contains("ignored"), "{}", out);
    assert!(
        err(&o).contains("proxy login is passed to the tool programs"),
        "{}",
        err(&o)
    );

    let dir = project("");
    let mut c = metagente(dir.path());
    c.env("HTTPS_PROXY", "http://ann:hunter2@envproxy.example:3128")
        .args(["check", "a.ag"]);
    let o = c.output().unwrap();
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        out.contains("from the environment variable HTTPS_PROXY"),
        "{}",
        out
    );
    assert!(!out.contains("hunter2") && !out.contains("ann:"), "{}", out);
}

#[test]
fn nothing_in_the_agent_file_can_change_the_network_settings() {
    // there is no syntax for it: the words are not part of the language
    let dir = project("");
    let source = "agent A\n  goal \"a\"\n  allow_self_signed true\n  accepts go\n  on go\n    reply \"ok\"\n";
    std::fs::write(dir.path().join("a.ag"), source).unwrap();
    let o = run(dir.path(), "check");
    assert!(!o.status.success());
    // and the login variables are as unreadable as the model key
    let toml = "[network.proxy]\nurl = \"http://p.example:1\"\nusername_env = \"MG_PU\"\npassword_env = \"MG_PP\"\n";
    let dir = project(toml);
    let source = "agent A\n  goal \"a\"\n  tool env \"MG_PP\"\n  accepts go\n  on go\n    v = env.get name: \"MG_PP\"\n    reply v\n";
    std::fs::write(dir.path().join("a.ag"), source).unwrap();
    let o = metagente(dir.path())
        .env("MG_PU", "ann")
        .env("MG_PP", "hunter2")
        .args(["run", "a.ag"])
        .output()
        .unwrap();
    assert!(!o.status.success());
    let text = err(&o);
    assert!(text.contains("agents can never read it"), "{}", text);
    assert!(!text.contains("hunter2"), "{}", text);
}

#[tokio::test]
async fn a_changed_network_section_applies_to_the_next_request_without_a_restart() {
    use crate::common::tls_server;
    use axum::{Router, routing::get};
    use metagente::runtime::Runtime;
    use metagente::runtime::config::Config;
    use metagente::runtime::serve::Served;
    use std::sync::Arc;
    let cert = tls_server::self_signed(&["localhost"]);
    let port =
        tls_server::serve_tls(Router::new().route("/hi", get(|| async { "hello" })), &cert).await;
    let source = format!(
        "agent Web\n  goal \"w\"\n  tool http\n  accepts go\n  on go\n    x = http.get url: \"https://localhost:{}/hi\"\n    reply x.text\n",
        port
    );
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.ag"), &source).unwrap();
    std::fs::write(
        dir.path().join("metagente.toml"),
        "[network]\nallow_self_signed = false\n",
    )
    .unwrap();
    let rt: Arc<Runtime> = Runtime::new(Config::load(dir.path()).unwrap(), None);
    let agents = metagente::runtime::run::load_agents(&dir.path().join("a.ag")).unwrap();
    let served = Served::new(rt.clone(), dir.path().join("a.ag"), None, agents);
    let ask = |rt: &Arc<Runtime>| {
        let rt = rt.clone();
        let source = source.clone();
        async move { crate::common::run_source(&rt, &source, "go", &[]).await }
    };
    assert!(ask(&rt).await.is_err());
    std::fs::write(
        dir.path().join("metagente.toml"),
        "[network]\nallow_self_signed = true\n",
    )
    .unwrap();
    let _ = served.agents(); // what serving does for every request
    assert_eq!(ask(&rt).await.unwrap().to_display(), "hello");
    std::fs::write(
        dir.path().join("metagente.toml"),
        "[network]\nallow_self_signed = false\n",
    )
    .unwrap();
    let _ = served.agents();
    assert!(ask(&rt).await.is_err());
}

#[test]
fn metagente_new_writes_a_commented_network_example_that_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let out = metagente(dir.path())
        .args(["new", "hello"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", err(&out));
    let toml = std::fs::read_to_string(dir.path().join("metagente.toml")).unwrap();
    for needle in [
        "# [network]",
        "# allow_self_signed = false",
        "# llm_allow_self_signed = false",
        "# self_signed_hosts",
        "# ca_file",
        "# [network.proxy]",
        "# username_env",
        "# password_env",
        "# no_proxy",
        "# pass_to_tools = false",
    ] {
        assert!(toml.contains(needle), "metagente.toml lacks `{}`", needle);
    }
    // the example is only comments, so the starter project checks clean and prints no warning
    let o = metagente(dir.path())
        .args(["check", "hello.ag"])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", err(&o));
    assert!(!err(&o).contains("Warning"), "{}", err(&o));
}
