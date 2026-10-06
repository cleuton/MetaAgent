// 0.1.2: new file. An agent loaded with `link` reads the parameters of the agent that loaded it.
use metagente::runtime::Runtime;
use metagente::runtime::config::Config;
use metagente::runtime::run::{RunOptions, run_file};
use std::path::Path;
use std::sync::Arc;

const A: &str = "agent Boss\n  goal \"b\"\n  link Helper from \"../b/helper.ag\"\n  accepts go\n  on go\n    reply Helper.get\n";
const B: &str = "agent Helper\n  goal \"h\"\n  accepts get\n  on get\n    reply @parameters.x\n";

// 0.1.2: folder `a` holds the loader and folder `b` the linked agent; each may or may not have a metagente.toml
fn setup(a_toml: Option<&str>, b_toml: Option<&str>) -> (tempfile::TempDir, Arc<Runtime>) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("a")).unwrap();
    std::fs::create_dir(dir.path().join("b")).unwrap();
    std::fs::write(dir.path().join("a/boss.ag"), A).unwrap();
    std::fs::write(dir.path().join("b/helper.ag"), B).unwrap();
    if let Some(t) = a_toml {
        std::fs::write(dir.path().join("a/metagente.toml"), t).unwrap();
    }
    if let Some(t) = b_toml {
        std::fs::write(dir.path().join("b/metagente.toml"), t).unwrap();
    }
    let config = Config::load(&dir.path().join("a")).unwrap();
    (dir, Runtime::new(config, None))
}

async fn run_boss(dir: &Path, rt: Arc<Runtime>) -> Result<String, String> {
    let opts = RunOptions {
        file: dir.join("a/boss.ag"),
        message: Some("go".into()),
        params: vec![],
        agent: None,
    };
    run_file(&opts, rt)
        .await
        .map(|v| v.to_display())
        .map_err(|d| d.render())
}

// 0.1.2: scenario 7
#[tokio::test]
async fn the_linked_agent_uses_the_parameters_of_the_loader_even_if_its_folder_has_others() {
    let (dir, rt) = setup(
        Some("[parameters]\nx = \"1\"\n"),
        Some("[parameters]\nx = \"2\"\n"),
    );
    assert_eq!(run_boss(dir.path(), rt).await.unwrap(), "1");
}

// 0.1.2: scenario 8
#[tokio::test]
async fn the_linked_agent_uses_the_loader_parameters_when_its_folder_has_no_file() {
    let (dir, rt) = setup(Some("[parameters]\nx = \"1\"\n"), None);
    assert_eq!(run_boss(dir.path(), rt).await.unwrap(), "1");
}

// 0.1.2: scenario 9
#[tokio::test]
async fn the_linked_agent_has_no_parameters_when_the_loader_has_none() {
    let (dir, rt) = setup(None, Some("[parameters]\nx = \"2\"\n"));
    let err = run_boss(dir.path(), rt).await.unwrap_err();
    assert!(err.contains("there is no parameter called `x`"), "{}", err);
    assert!(err.contains("helper.ag"), "{}", err);
}

// 0.1.2: a chain A -> B -> C reads the parameters of A
#[tokio::test]
async fn a_chain_of_links_reads_the_parameters_of_the_first_agent() {
    let (dir, rt) = setup(
        Some("[parameters]\nx = \"1\"\n"),
        Some("[parameters]\nx = \"2\"\n"),
    );
    std::fs::create_dir(dir.path().join("c")).unwrap();
    std::fs::write(
        dir.path().join("c/metagente.toml"),
        "[parameters]\nx = \"3\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("c/last.ag"),
        "agent Last\n  goal \"l\"\n  accepts get\n  on get\n    reply @parameters.x\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("b/helper.ag"),
        "agent Helper\n  goal \"h\"\n  link Last from \"../c/last.ag\"\n  accepts get\n  on get\n    reply Last.get\n",
    )
    .unwrap();
    assert_eq!(run_boss(dir.path(), rt).await.unwrap(), "1");
}
