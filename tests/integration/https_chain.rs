// 0.1.3: new file. The sample multi-agent-samples/https-chain, end to end: three processes, a certificate made on the
// spot with the command of its README, and every message that the README promises (User Story 7).
use crate::common::self_signed::{
    ServeProc, free_port, is_http_answer, plain_request, sample_certificate,
};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn sample() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("multi-agent-samples/https-chain")
}

fn readme() -> String {
    std::fs::read_to_string(sample().join("README.md")).unwrap()
}

/// A copy of the sample (without any certificate) with its two addresses moved to free ports.
fn copy_with_ports(a2a: u16, mcp: u16) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_into(dir.path(), a2a, mcp);
    dir
}

fn copy_into(dir: &Path, a2a: u16, mcp: u16) {
    for name in ["planner.ag", "researcher.ag", "facts.ag", "metagente.toml"] {
        let text = std::fs::read_to_string(sample().join(name)).unwrap();
        let text = text
            .replace("8443", &a2a.to_string())
            .replace("9443", &mcp.to_string());
        std::fs::write(dir.join(name), text).unwrap();
    }
}

fn metagente(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_metagente"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}

fn serve_args(file: &str, flag: &str, port: u16) -> Vec<String> {
    vec![file.to_string(), flag.to_string(), port.to_string()]
}

/// The text of a process with the test's ports and folder written as the README writes them.
fn as_in_readme(text: &str, dir: &Path, a2a: u16, mcp: u16) -> String {
    text.replace(&a2a.to_string(), "8443")
        .replace(&mcp.to_string(), "9443")
        .replace(&format!("{}/", dir.display()), "")
}

fn guide() -> String {
    std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/secure-connections-guide.md"),
    )
    .unwrap()
}

/// A message quoted in the guide must be word for word the one the program prints (the test compares the README text
/// with the real output, and these lines are the same ones).
fn assert_in_guide(guide: &str, lines: &[&str]) {
    for line in lines {
        assert!(guide.contains(line), "the guide does not show:\n{}", line);
    }
}

fn said(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

/// Each line must be printed by the program AND written in the README, word for word.
fn assert_message(readme: &str, output: &str, lines: &[&str]) {
    for line in lines {
        assert!(
            output.contains(line),
            "the program did not print:\n{}\n--- it printed:\n{}",
            line,
            output
        );
        assert!(readme.contains(line), "the README does not show:\n{}", line);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_chain_works_over_https_and_only_https_and_every_message_of_the_readme_is_real() {
    let readme = readme();
    let (a2a, mcp) = (free_port(), free_port());
    let dir = copy_with_ports(a2a, mcp);
    let files = sample_certificate(dir.path(), &readme);

    // the three terminals of the README
    let facts = ServeProc::start(dir.path(), &serve_args("facts.ag", "--mcp", mcp), 1);
    let researcher = ServeProc::start(dir.path(), &serve_args("researcher.ag", "--a2a", a2a), 1);
    let planner = metagente(dir.path(), &["run", "planner.ag", "plan", "topic=rust"]);
    assert!(planner.status.success(), "{}", said(&planner));
    assert_eq!(
        String::from_utf8_lossy(&planner.stdout).trim(),
        "Planner got: Rust is a systems language"
    );
    assert!(readme.contains("Planner got: Rust is a systems language"));
    for command in [
        "metagente serve facts.ag --mcp 9443",
        "metagente serve researcher.ag --a2a 8443",
        "metagente run planner.ag plan topic=rust",
    ] {
        assert!(readme.contains(command), "the README lacks `{}`", command);
    }
    for system in ["Linux", "macOS", "Windows"] {
        assert!(
            readme.contains(system),
            "the README says nothing for {}",
            system
        );
    }

    // no plain http listener exists on either port
    for port in [a2a, mcp] {
        let got = plain_request(port).await;
        assert!(!is_http_answer(&got), "port {} answered plain http", port);
    }

    // no private key, and no part of one, in anything the three programs printed
    let key = std::fs::read_to_string(&files.key).unwrap();
    let everything = format!(
        "{}\n{}\n{}",
        said(&planner),
        facts.said(),
        researcher.said()
    );
    assert!(!everything.contains("PRIVATE KEY"), "{}", everything);
    for line in key
        .lines()
        .filter(|l| l.len() > 20 && !l.starts_with("-----"))
    {
        assert!(
            !everything.contains(line),
            "a line of the private key was printed"
        );
    }

    // break it on purpose, 1: http instead of https in the Planner (nothing is sent: the run stops first)
    let broken = copy_with_ports(a2a, mcp);
    std::fs::create_dir_all(broken.path().join("certs")).unwrap();
    for f in ["dev-cert.pem", "dev-key.pem"] {
        std::fs::copy(
            dir.path().join("certs").join(f),
            broken.path().join("certs").join(f),
        )
        .unwrap();
    }
    let planner_text = std::fs::read_to_string(broken.path().join("planner.ag")).unwrap();
    std::fs::write(
        broken.path().join("planner.ag"),
        planner_text.replace(
            &format!("https://127.0.0.1:{}", a2a),
            &format!("http://127.0.0.1:{}", a2a),
        ),
    )
    .unwrap();
    let o = metagente(broken.path(), &["run", "planner.ag", "plan", "topic=rust"]);
    assert!(!o.status.success());
    assert_message(
        &readme,
        &as_in_readme(&said(&o), broken.path(), a2a, mcp),
        &[
            "Problem on line 4 of planner.ag: Researcher is declared secure in metagente.toml but this address uses http: http://127.0.0.1:8443",
            "Fix: write https:// instead of http://, or remove Researcher from [secure] agents",
        ],
    );
    assert_in_guide(
        &guide(),
        &[
            "Problem on line 4 of planner.ag: Researcher is declared secure in metagente.toml but this address uses http: http://127.0.0.1:8443",
            "Fix: write https:// instead of http://, or remove Researcher from [secure] agents",
        ],
    );
    // ... and in the tool address of the Researcher, which is served and so reads its file again
    let researcher_text = std::fs::read_to_string(dir.path().join("researcher.ag")).unwrap();
    std::fs::write(
        dir.path().join("researcher.ag"),
        researcher_text.replace(
            &format!("https://127.0.0.1:{}", mcp),
            &format!("http://127.0.0.1:{}", mcp),
        ),
    )
    .unwrap();
    let o = metagente(dir.path(), &["run", "planner.ag", "plan", "topic=rust"]);
    assert!(!o.status.success());
    assert_message(
        &readme,
        &as_in_readme(&said(&o), dir.path(), a2a, mcp),
        &[
            "kb is declared secure in metagente.toml but this address uses http: http://127.0.0.1:9443/mcp",
            "Fix: write https:// instead of http://, or remove kb from [secure] tools",
        ],
    );
    std::fs::write(dir.path().join("researcher.ag"), researcher_text).unwrap();

    // 2: the certificate is taken away: the secure agent does not start, and never falls back to http
    let toml = std::fs::read_to_string(dir.path().join("metagente.toml")).unwrap();
    let without_both: String = toml
        .lines()
        .filter(|l| !l.starts_with("tls_cert") && !l.starts_with("tls_key"))
        .map(|l| format!("{}\n", l))
        .collect();
    std::fs::write(broken.path().join("metagente.toml"), &without_both).unwrap();
    let o = crate::common::self_signed::serve_refused(
        broken.path(),
        &serve_args("researcher.ag", "--a2a", free_port()),
    );
    assert!(!o.status.success());
    assert_message(
        &readme,
        &as_in_readme(&said(&o), broken.path(), a2a, mcp),
        &[
            "Researcher is declared secure, so I will only serve it over https, but no certificate is set.",
            "Fix: set tls_cert and tls_key in [serve] of metagente.toml, or use --tls-cert and --tls-key",
        ],
    );
    assert_in_guide(
        &guide(),
        &[
            "Researcher is declared secure, so I will only serve it over https, but no certificate is set.",
            "Fix: set tls_cert and tls_key in [serve] of metagente.toml, or use --tls-cert and --tls-key",
        ],
    );
    let without_cert: String = toml
        .lines()
        .filter(|l| !l.starts_with("tls_cert"))
        .map(|l| format!("{}\n", l))
        .collect();
    std::fs::write(broken.path().join("metagente.toml"), without_cert).unwrap();
    let o = crate::common::self_signed::serve_refused(
        broken.path(),
        &serve_args("researcher.ag", "--a2a", free_port()),
    );
    assert_message(
        &readme,
        &as_in_readme(&said(&o), broken.path(), a2a, mcp),
        &[
            "I have a key but no certificate (tls_cert).",
            "Fix: add tls_cert to [serve] in metagente.toml, or use --tls-cert with the PEM file of the certificate",
        ],
    );

    // 3: the certificate is not trusted any more
    let no_trust = toml.replace("ca_file = ", "# ca_file = ");
    std::fs::write(broken.path().join("metagente.toml"), &no_trust).unwrap();
    std::fs::write(broken.path().join("planner.ag"), planner_text).unwrap();
    let o = metagente(broken.path(), &["run", "planner.ag", "plan", "topic=rust"]);
    assert!(!o.status.success());
    assert_message(
        &readme,
        &as_in_readme(&said(&o), broken.path(), a2a, mcp),
        &[
            "I could not trust the certificate of https://127.0.0.1:8443/.well-known/agent-card.json (remote agent Researcher): it was signed by an authority I do not know, for example a self-signed certificate",
            "Fix: add that authority with ca_file in [network]; for development only you can set allow_self_signed = true",
        ],
    );
    assert_in_guide(
        &guide(),
        &[
            "I could not trust the certificate of https://127.0.0.1:8443/.well-known/agent-card.json (remote agent Researcher): it was signed by an authority I do not know, for example a self-signed certificate",
            "Fix: add that authority with ca_file in [network]; for development only you can set allow_self_signed = true",
        ],
    );

    // 4: the other way to trust it, with its warning
    let other_way = toml.replace(
        "ca_file = \"certs/dev-cert.pem\"        # preferred: trust this one certificate and keep checking it",
        "allow_self_signed = true\nself_signed_hosts = [\"127.0.0.1\"]",
    );
    assert_ne!(other_way, toml, "the toml of the sample changed shape");
    std::fs::write(broken.path().join("metagente.toml"), other_way).unwrap();
    let o = metagente(broken.path(), &["run", "planner.ag", "plan", "topic=rust"]);
    assert!(o.status.success(), "{}", said(&o));
    assert_eq!(
        String::from_utf8_lossy(&o.stdout).trim(),
        "Planner got: Rust is a systems language"
    );
    assert_message(
        &readme,
        &said(&o),
        &[
            "Warning: certificate checks are OFF for agents and tools for 127.0.0.1 (allow_self_signed = true in metagente.toml). Use this for development only.",
        ],
    );
    assert_in_guide(
        &guide(),
        &[
            "Warning: certificate checks are OFF for agents and tools for 127.0.0.1 (allow_self_signed = true in metagente.toml). Use this for development only.",
        ],
    );
}

#[test]
fn the_sample_does_not_commit_a_key_and_says_so() {
    let gitignore = std::fs::read_to_string(sample().join(".gitignore")).unwrap();
    assert!(
        gitignore.lines().any(|l| l.trim() == "certs/"),
        "{}",
        gitignore
    );
    assert!(
        !sample().join("certs").exists(),
        "certs/ must not exist in the repository"
    );
    let mut stack = vec![sample()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(text) = std::fs::read_to_string(&path) {
                assert!(
                    !text.contains("BEGIN PRIVATE KEY")
                        || path.extension().is_some_and(|e| e == "md"),
                    "{} holds a private key",
                    path.display()
                );
                assert!(
                    !text.contains("MIIE"),
                    "{} looks like it holds key material",
                    path.display()
                );
            }
        }
    }
}

// 0.1.3: the beginner guide is linked from the README, the tutorial, the programming guide and the sample, and the guide
// links back to the sample and teaches the same certificate command as the sample
#[test]
fn the_beginner_guide_is_linked_from_everywhere_and_links_back_to_the_sample() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let read = |p: &str| std::fs::read_to_string(root.join(p)).unwrap();
    assert!(root.join("docs/secure-connections-guide.md").is_file());
    for (file, link) in [
        ("README.md", "docs/secure-connections-guide.md"),
        ("docs/tutorial.md", "secure-connections-guide.md"),
        ("docs/guide.md", "secure-connections-guide.md"),
        (
            "multi-agent-samples/https-chain/README.md",
            "../../docs/secure-connections-guide.md",
        ),
    ] {
        assert!(
            read(file).contains(link),
            "{} does not link the guide",
            file
        );
    }
    let guide = read("docs/secure-connections-guide.md");
    assert!(guide.contains("../multi-agent-samples/https-chain/README.md"));
    // every relative link of the guide points at something that exists
    for part in guide.split("](").skip(1) {
        let target = part.split(')').next().unwrap();
        if target.starts_with("http") || target.starts_with('#') {
            continue;
        }
        let path = target.split('#').next().unwrap();
        assert!(
            root.join("docs").join(path).exists(),
            "the guide links {} which does not exist",
            target
        );
    }
    // the commands of the guide are the ones of the sample README
    let readme = read("multi-agent-samples/https-chain/README.md");
    for command in [
        "metagente serve facts.ag --mcp 9443",
        "metagente serve researcher.ag --a2a 8443",
        "metagente run planner.ag plan topic=rust",
    ] {
        assert!(
            guide.contains(command) && readme.contains(command),
            "{}",
            command
        );
    }
    let make = readme
        .lines()
        .find(|l| l.starts_with("openssl req -x509"))
        .unwrap();
    assert!(
        guide.contains(make),
        "the guide teaches another certificate command than the sample"
    );
}
