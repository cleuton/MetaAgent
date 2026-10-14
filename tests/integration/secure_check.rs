// 0.1.3: new file. `metagente check` and the secure list: what it reports before anything runs (FR-003, FR-004, D16).
use std::path::Path;
use std::process::{Command, Output};

fn metagente(dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_metagente"));
    c.current_dir(dir);
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

fn caller(remote: &str, url: &str) -> String {
    format!(
        "agent Caller\n  goal \"c\"\n  remote {} at \"{}\"\n  accepts go\n  on go\n    x = {}.ask city: \"Lisbon\"\n    reply x\n",
        remote, url, remote
    )
}

fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (name, text) in files {
        std::fs::write(dir.path().join(name), text).unwrap();
    }
    dir
}

fn check(dir: &Path, file: &str) -> Output {
    metagente(dir).args(["check", file]).output().unwrap()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

#[test]
fn a_literal_http_address_of_a_secure_remote_is_reported_with_its_line() {
    let dir = project(&[
        ("a.ag", &caller("Bob", "http://host:8443")),
        ("metagente.toml", "[secure]\nagents = [\"Bob\"]\n"),
    ]);
    let o = check(dir.path(), "a.ag");
    assert!(!o.status.success());
    let e = err(&o);
    assert!(e.contains("Problem on line 3 of a.ag"), "{}", e);
    assert!(
        e.contains(
            "Bob is declared secure in metagente.toml but this address uses http: http://host:8443"
        ),
        "{}",
        e
    );
    assert!(e.contains("remove Bob from [secure] agents"), "{}", e);
}

#[test]
fn an_https_address_of_a_secure_remote_checks_clean() {
    let dir = project(&[
        ("a.ag", &caller("Bob", "https://host:8443")),
        ("metagente.toml", "[secure]\nagents = [\"Bob\"]\n"),
    ]);
    let o = check(dir.path(), "a.ag");
    assert!(o.status.success(), "{}", err(&o));
}

#[test]
fn a_listed_name_that_exists_nowhere_warns_and_names_the_closest_one() {
    let dir = project(&[
        ("a.ag", &caller("Bob", "https://host:8443")),
        ("metagente.toml", "[secure]\nagents = [\"Bobb\"]\n"),
    ]);
    let o = check(dir.path(), "a.ag");
    assert!(
        o.status.success(),
        "a typo is a warning, not an error: {}",
        err(&o)
    );
    let e = err(&o);
    assert!(
        e.contains("Warning: [secure] lists the agent Bobb"),
        "{}",
        e
    );
    assert!(e.contains("Did you mean Bob?"), "{}", e);
}

#[test]
fn a_name_declared_in_a_sibling_file_does_not_warn() {
    let dir = project(&[
        ("a.ag", &caller("Bob", "https://host:8443")),
        (
            "facts.ag",
            "agent Facts\n  goal \"f\"\n  accepts lookup topic\n  on lookup\n    reply \"x\"\n",
        ),
        (
            "metagente.toml",
            "[secure]\nagents = [\"Facts\", \"Bob\"]\n",
        ),
    ]);
    let o = check(dir.path(), "a.ag");
    assert!(o.status.success(), "{}", err(&o));
    assert!(!err(&o).contains("Warning"), "{}", err(&o));
}

#[test]
fn the_secure_list_comes_only_from_metagente_toml() {
    // FR-004: a comment in an .ag file, a parameter and a message value change nothing
    let dir = project(&[
        (
            "a.ag",
            &format!(
                "# [secure]\n# agents = [\"Bob\"]\n{}",
                caller("Bob", "http://host:8443")
            ),
        ),
        (
            "metagente.toml",
            "[parameters]\nsecure_agents = \"Bob\"\nagents = \"Bob\"\n",
        ),
    ]);
    let o = check(dir.path(), "a.ag");
    assert!(o.status.success(), "nothing is secure here: {}", err(&o));
    let o = metagente(dir.path())
        .args(["run", "a.ag", "go"])
        .output()
        .unwrap();
    assert!(
        !err(&o).contains("declared secure"),
        "no secure name may appear: {}",
        err(&o)
    );
}

#[test]
fn an_unknown_key_and_wrong_types_in_secure_are_reported() {
    let dir = project(&[
        ("a.ag", &caller("Bob", "https://host:8443")),
        ("metagente.toml", "[secure]\nagent = [\"Bob\"]\n"),
    ]);
    let o = check(dir.path(), "a.ag");
    assert!(!o.status.success());
    assert!(
        err(&o).contains("unknown setting `agent` in [secure]"),
        "{}",
        err(&o)
    );

    for (toml, needle) in [
        (
            "[secure]\nagents = \"Bob\"\n",
            "`agents` in [secure] must be a list of texts",
        ),
        (
            "[secure]\nllm = \"yes\"\n",
            "`llm` in [secure] must be true or false",
        ),
        (
            "[secure]\ntools = [1, 2]\n",
            "`tools` in [secure] must be a list of texts",
        ),
    ] {
        std::fs::write(dir.path().join("metagente.toml"), toml).unwrap();
        let o = check(dir.path(), "a.ag");
        let e = err(&o);
        assert!(!o.status.success(), "{}", toml);
        assert!(e.contains(needle), "{}: {}", toml, e);
        assert!(
            e.contains("line 2") || e.contains("metagente.toml"),
            "{}",
            e
        );
    }
}

#[test]
fn a_missing_certificate_file_is_only_a_warning_because_check_never_needs_one() {
    let dir = project(&[
        ("a.ag", &caller("Bob", "https://host:8443")),
        (
            "metagente.toml",
            "[secure]\nagents = [\"Caller\"]\n[serve]\ntls_cert = \"certs/dev-cert.pem\"\ntls_key = \"certs/dev-key.pem\"\n",
        ),
    ]);
    let o = check(dir.path(), "a.ag");
    assert!(o.status.success(), "{}", err(&o));
    let e = err(&o);
    assert!(
        e.contains("Warning: tls_cert in [serve] points to"),
        "{}",
        e
    );
    assert!(e.contains("`metagente serve` will need it"), "{}", e);
    assert!(e.contains("tls_key"), "{}", e);
}

#[test]
fn a_secure_agent_that_only_uses_local_tools_gets_a_one_line_note() {
    let dir = project(&[
        (
            "a.ag",
            "agent Caller\n  goal \"c\"\n  tool clock\n  accepts go\n  on go\n    reply clock.now\n",
        ),
        ("metagente.toml", "[secure]\nagents = [\"Caller\"]\n"),
    ]);
    let o = check(dir.path(), "a.ag");
    assert!(o.status.success(), "{}", err(&o));
    assert!(
        out(&o).contains("Note: Caller is declared secure. Its local tools and links (clock) are not network traffic"),
        "{}",
        out(&o)
    );
}

#[test]
fn a_secure_agent_with_a_literal_http_address_is_reported_before_it_runs() {
    let dir = project(&[
        (
            "a.ag",
            "agent Researcher\n  goal \"r\"\n  tool http\n  accepts go\n  on go\n    x = http.get url: \"http://example.com\"\n    reply x.status\n",
        ),
        ("metagente.toml", "[secure]\nagents = [\"Researcher\"]\n"),
    ]);
    let o = check(dir.path(), "a.ag");
    assert!(!o.status.success());
    let e = err(&o);
    assert!(
        e.contains("Researcher is declared secure, so it may only call https addresses"),
        "{}",
        e
    );
    assert!(e.contains("Problem on line 6"), "{}", e);
}

#[test]
fn a_file_without_a_secure_section_behaves_as_in_0_1_2() {
    let dir = project(&[
        ("a.ag", &caller("Bob", "http://host:8443")),
        ("metagente.toml", "[runtime]\ntimeout_seconds = 30\n"),
    ]);
    let o = check(dir.path(), "a.ag");
    assert!(o.status.success(), "{}", err(&o));
    assert!(!err(&o).contains("Warning"), "{}", err(&o));
}
