// 0.1.3: new file. An agent loaded with `link` runs in the context of the agent that loaded it (D14), so a secure
// loader cannot escape the https rule by delegating.
use axum::{Router, routing::get};
use metagente::runtime::Runtime;
use metagente::runtime::config::Config;
use metagente::runtime::run::{RunOptions, run_file};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A plain http server that counts the requests it answers.
async fn plain_server() -> (String, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    let hits = count.clone();
    let app = Router::new().route(
        "/x",
        get(move || {
            let hits = hits.clone();
            async move {
                hits.fetch_add(1, Ordering::SeqCst);
                "ok"
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://127.0.0.1:{}/x", port), count)
}

fn boss(_url: &str) -> String {
    "agent Boss\n  goal \"b\"\n  tool http\n  link Helper from \"../b/helper.ag\"\n  accepts own url\n  accepts delegate\n  on own\n    x = http.get url: url\n    reply x.text\n  on delegate\n    reply Helper.get\n".to_string()
}

fn helper(url: &str) -> String {
    format!(
        "agent Helper\n  goal \"h\"\n  tool http\n  link Deep from \"../c/deep.ag\"\n  accepts get\n  accepts deeper\n  on get\n    x = http.get url: \"{url}\"\n    reply x.text\n  on deeper\n    reply Deep.get\n",
        url = url
    )
}

fn deep(url: &str) -> String {
    format!(
        "agent Deep\n  goal \"d\"\n  tool http\n  accepts get\n  on get\n    x = http.get url: \"{url}\"\n    reply x.text\n",
        url = url
    )
}

/// Folders a (the loader), b (the linked agent) and c (linked by the linked agent), each with its own optional toml.
fn setup(url: &str, a_toml: &str, b_toml: Option<&str>) -> (tempfile::TempDir, Arc<Runtime>) {
    let dir = tempfile::tempdir().unwrap();
    for d in ["a", "b", "c"] {
        std::fs::create_dir(dir.path().join(d)).unwrap();
    }
    std::fs::write(dir.path().join("a/boss.ag"), boss(url)).unwrap();
    std::fs::write(dir.path().join("b/helper.ag"), helper(url)).unwrap();
    std::fs::write(dir.path().join("c/deep.ag"), deep(url)).unwrap();
    std::fs::write(dir.path().join("a/metagente.toml"), a_toml).unwrap();
    if let Some(t) = b_toml {
        std::fs::write(dir.path().join("b/metagente.toml"), t).unwrap();
    }
    let config = Config::load(&dir.path().join("a")).unwrap();
    (dir, Runtime::new(config, None))
}

async fn run_boss(
    dir: &Path,
    rt: &Arc<Runtime>,
    message: &str,
    url: &str,
) -> Result<String, String> {
    let opts = RunOptions {
        file: dir.join("a/boss.ag"),
        message: Some(message.into()),
        params: if message == "own" {
            vec![format!("url={}", url)]
        } else {
            vec![]
        },
        agent: None,
    };
    run_file(&opts, rt.clone())
        .await
        .map(|v| v.to_display())
        .map_err(|d| d.render())
}

#[tokio::test]
async fn a_secure_loader_makes_the_agent_it_links_https_only() {
    let (url, hits) = plain_server().await;
    let (dir, rt) = setup(&url, "[secure]\nagents = [\"Boss\"]\n", None);
    let err = run_boss(dir.path(), &rt, "delegate", &url)
        .await
        .unwrap_err();
    assert!(
        err.contains("Helper is declared secure, so it may only call https addresses"),
        "{}",
        err
    );
    assert!(err.contains(&url), "{}", err);
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "the linked agent must not reach the http server"
    );
}

#[tokio::test]
async fn a_chain_of_links_uses_the_settings_of_the_first_agent_for_all_of_them() {
    let (url, hits) = plain_server().await;
    let (dir, rt) = setup(&url, "[secure]\nagents = [\"Boss\"]\n", None);
    // Boss links Helper, which links Deep: the delegation goes through the helper to the deepest agent
    std::fs::write(
        dir.path().join("a/boss.ag"),
        boss(&url).replace("Helper.get", "Helper.deeper"),
    )
    .unwrap();
    let err = run_boss(dir.path(), &rt, "delegate", &url)
        .await
        .unwrap_err();
    assert!(
        err.contains("is declared secure, so it may only call https addresses"),
        "{}",
        err
    );
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_loader_that_is_not_secure_leaves_the_agent_it_links_free() {
    let (url, hits) = plain_server().await;
    let (dir, rt) = setup(&url, "[secure]\nagents = [\"SomebodyElse\"]\n", None);
    assert_eq!(
        run_boss(dir.path(), &rt, "delegate", &url).await.unwrap(),
        "ok"
    );
    assert_eq!(run_boss(dir.path(), &rt, "own", &url).await.unwrap(), "ok");
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_linked_agent_whose_own_name_is_in_the_loaders_list_keeps_that_rule_while_the_loader_stays_free()
 {
    let (url, hits) = plain_server().await;
    let (dir, rt) = setup(&url, "[secure]\nagents = [\"Helper\"]\n", None);
    // the loader itself is free: its own http call works
    assert_eq!(run_boss(dir.path(), &rt, "own", &url).await.unwrap(), "ok");
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    // the linked agent is bound by the rule
    let err = run_boss(dir.path(), &rt, "delegate", &url)
        .await
        .unwrap_err();
    assert!(err.contains("Helper is declared secure"), "{}", err);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_secure_list_in_the_folder_of_the_linked_agent_is_ignored() {
    let (url, hits) = plain_server().await;
    let (dir, rt) = setup(
        &url,
        "[runtime]\ntimeout_seconds = 30\n",
        Some("[secure]\nagents = [\"Helper\"]\n"),
    );
    assert_eq!(
        run_boss(dir.path(), &rt, "delegate", &url).await.unwrap(),
        "ok"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}
