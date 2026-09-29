use crate::common::*;

#[tokio::test]
async fn an_undeclared_tool_is_refused_and_the_file_is_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let source = "agent Sneaky\n  goal \"x\"\n  accepts go\n  on go\n    file.write path: \"x.txt\" text: \"hi\"\n    reply \"done\"\n";
    let err = run_source(&rt, source, "go", &[])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("Problem on line 5"), "{}", err);
    assert!(
        err.contains("agent Sneaky uses `file` but never declared it"),
        "{}",
        err
    );
    assert!(
        err.contains("add the line `tool file` under `agent Sneaky`"),
        "{}",
        err
    );
    assert!(!dir.path().join("x.txt").exists());
}

#[tokio::test]
async fn a_scoped_file_tool_stays_inside_its_folder() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("data")).unwrap();
    std::fs::write(dir.path().join("data/ok.txt"), "fine").unwrap();
    std::fs::write(dir.path().join("secret.txt"), "top secret").unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let source = r#"agent Scoped
  goal "Read inside data only"
  tool file "data/"
  accepts read path
  accepts write path
  on read
    reply file.read path: path
  on write
    file.write path: path text: "x"
    reply "written"
"#;
    let agent = agent_from(&rt, source, None);
    assert_eq!(
        ask(&agent, "read", &[("path", "ok.txt")])
            .await
            .unwrap()
            .to_display(),
        "fine"
    );
    for bad in ["../secret.txt", "../../etc/passwd", "sub/../../secret.txt"] {
        let err = ask(&agent, "read", &[("path", bad)])
            .await
            .unwrap_err()
            .render();
        assert!(
            err.contains("outside the folder this agent may use (data/)"),
            "{} -> {}",
            bad,
            err
        );
    }
    let absolute = dir.path().join("secret.txt").display().to_string();
    let err = ask(&agent, "read", &[("path", &absolute)])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("outside the folder"), "{}", err);
    let err = ask(&agent, "write", &[("path", "../evil.txt")])
        .await
        .unwrap_err()
        .render();
    assert!(err.contains("outside the folder"), "{}", err);
    assert!(!dir.path().join("evil.txt").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn a_symlink_cannot_lead_out_of_the_folder() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("data")).unwrap();
    std::fs::write(dir.path().join("secret.txt"), "top secret").unwrap();
    std::os::unix::fs::symlink(
        dir.path().join("secret.txt"),
        dir.path().join("data/link.txt"),
    )
    .unwrap();
    std::os::unix::fs::symlink(dir.path(), dir.path().join("data/up")).unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let source = "agent S\n  goal \"x\"\n  tool file \"data/\"\n  accepts read path\n  on read\n    reply file.read path: path\n";
    let agent = agent_from(&rt, source, None);
    for bad in ["link.txt", "up/secret.txt"] {
        let err = ask(&agent, "read", &[("path", bad)])
            .await
            .unwrap_err()
            .render();
        assert!(err.contains("outside the folder"), "{} -> {}", bad, err);
    }
}

#[tokio::test]
async fn the_default_file_scope_is_the_project_folder() {
    let outer = tempfile::tempdir().unwrap();
    let project = outer.path().join("project");
    std::fs::create_dir(&project).unwrap();
    std::fs::write(outer.path().join("outside.txt"), "no").unwrap();
    std::fs::write(project.join("inside.txt"), "yes").unwrap();
    let rt = runtime_in(&project, None, |_| {});
    let source = "agent S\n  goal \"x\"\n  tool file\n  accepts read path\n  on read\n    reply file.read path: path\n";
    let agent = agent_from(&rt, source, None);
    assert_eq!(
        ask(&agent, "read", &[("path", "inside.txt")])
            .await
            .unwrap()
            .to_display(),
        "yes"
    );
    let err = ask(&agent, "read", &[("path", "../outside.txt")])
        .await
        .unwrap_err()
        .render();
    assert!(
        err.contains("outside the folder this agent may use (the project folder)"),
        "{}",
        err
    );
}

#[tokio::test]
async fn links_remotes_and_mcp_servers_must_be_declared_too() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    for (target, call) in [
        ("Other", "Other.ask city: \"x\""),
        ("weather", "weather.forecast city: \"x\""),
    ] {
        let source = format!(
            "agent A\n  goal \"x\"\n  accepts go\n  on go\n    r = {}\n    reply r\n",
            call
        );
        let err = run_source(&rt, &source, "go", &[])
            .await
            .unwrap_err()
            .render();
        assert!(
            err.contains(&format!("does not know anything called `{}`", target)),
            "{}",
            err
        );
        assert!(
            err.contains("`link") && err.contains("`remote") && err.contains("from mcp"),
            "{}",
            err
        );
    }
}
