use crate::common::fake_mcp;
use crate::common::*;
use metagente::runtime::run::{RunOptions, run_file};
use std::path::Path;

fn options(file: &Path, message: &str, params: &[&str]) -> RunOptions {
    RunOptions {
        file: file.to_path_buf(),
        message: Some(message.to_string()),
        params: params.iter().map(|s| s.to_string()).collect(),
        agent: None,
    }
}

fn setup(dir: &Path, url: &str) {
    std::fs::write(
        dir.join("weather.ag"),
        project_file("examples/weather.ag").replace("npx -y weather-mcp", url),
    )
    .unwrap();
    std::fs::write(dir.join("planner.ag"), project_file("examples/planner.ag")).unwrap();
}

#[tokio::test]
async fn planner_calls_weather_by_name_and_picks_up_changes_without_being_edited() {
    let server = fake_mcp::start().await;
    let dir = tempfile::tempdir().unwrap();
    setup(dir.path(), &server.url);
    let rt = runtime_in(dir.path(), None, |_| {});
    let first = run_file(
        &options(&dir.path().join("planner.ag"), "plan", &["city=Lisbon"]),
        rt.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        first.to_display(),
        "Pack for this: In Lisbon it will be sunny, 24 degrees"
    );

    // Change only the linked agent, in the same running process.
    std::thread::sleep(std::time::Duration::from_millis(50));
    let changed = project_file("examples/weather.ag")
        .replace("npx -y weather-mcp", &server.url)
        .replace("In {city} it will be", "Forecast for {city}:");
    std::fs::write(dir.path().join("weather.ag"), changed).unwrap();
    let planner_before = std::fs::read_to_string(dir.path().join("planner.ag")).unwrap();
    let second = run_file(
        &options(&dir.path().join("planner.ag"), "plan", &["city=Lisbon"]),
        rt,
    )
    .await
    .unwrap();
    assert_eq!(
        second.to_display(),
        "Pack for this: Forecast for Lisbon: sunny, 24 degrees"
    );
    assert_eq!(
        planner_before,
        std::fs::read_to_string(dir.path().join("planner.ag")).unwrap()
    );
}

#[tokio::test]
async fn link_resolution_order_same_file_then_next_to_then_agents_folder_then_quoted_path() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime_in(dir.path(), None, |_| {});
    let helper = |word: &str| {
        format!(
            "agent Helper\n  goal \"h\"\n  accepts hi\n  on hi\n    reply \"{}\"\n",
            word
        )
    };
    let boss =
        "agent Boss\n  goal \"b\"\n  link Helper\n  accepts go\n  on go\n    reply Helper.hi\n";

    // 1. agents folder only
    std::fs::create_dir(dir.path().join("agents")).unwrap();
    std::fs::write(
        dir.path().join("agents/Helper.ag"),
        helper("from agents folder"),
    )
    .unwrap();
    std::fs::write(dir.path().join("boss.ag"), boss).unwrap();
    let r = run_file(&options(&dir.path().join("boss.ag"), "go", &[]), rt.clone())
        .await
        .unwrap();
    assert_eq!(r.to_display(), "from agents folder");

    // 2. next to the calling file wins over the agents folder (lower case file name works too)
    std::fs::write(dir.path().join("helper.ag"), helper("from next to")).unwrap();
    let r = run_file(&options(&dir.path().join("boss.ag"), "go", &[]), rt.clone())
        .await
        .unwrap();
    assert_eq!(r.to_display(), "from next to");

    // 3. the same file wins over everything
    std::fs::write(
        dir.path().join("boss.ag"),
        format!("{}{}", boss, helper("from same file")),
    )
    .unwrap();
    let r = run_file(&options(&dir.path().join("boss.ag"), "go", &[]), rt.clone())
        .await
        .unwrap();
    assert_eq!(r.to_display(), "from same file");

    // 4. a quoted path is used as written
    std::fs::create_dir(dir.path().join("elsewhere")).unwrap();
    std::fs::write(
        dir.path().join("elsewhere/x.ag"),
        helper("from quoted path"),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("boss2.ag"),
        boss.replace("link Helper", "link Helper from \"elsewhere/x.ag\""),
    )
    .unwrap();
    let r = run_file(&options(&dir.path().join("boss2.ag"), "go", &[]), rt)
        .await
        .unwrap();
    assert_eq!(r.to_display(), "from quoted path");
}
