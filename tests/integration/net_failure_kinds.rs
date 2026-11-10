// 0.1.5: new file. A failed model request is told apart by KIND only (no address, no library text): timeout, no
// connection, certificate or proxy. Named models write their own sentence for each kind (FR-013).
use crate::common::proxy::{self, Mode};
use crate::common::tls_server;
use metagente::runtime::config::Config;
use metagente::runtime::net::{ClientSet, FailureKind};
use std::time::Duration;
use tokio::net::TcpListener;

async fn kind_of(net: &ClientSet, url: &str, timeout: Option<Duration>) -> FailureKind {
    let client = net.model().expect("client");
    let mut req = client.get(url);
    if let Some(t) = timeout {
        req = req.timeout(t);
    }
    let error = req.send().await.expect_err("the request must fail");
    net.failure_kind(&error, url)
}

#[tokio::test]
async fn a_closed_port_is_no_connection() {
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        l.local_addr().unwrap().port()
    };
    let net = ClientSet::for_config(&Config::default());
    assert_eq!(
        kind_of(&net, &format!("https://127.0.0.1:{}/", port), None).await,
        FailureKind::NoConnection
    );
}

#[tokio::test]
async fn a_server_that_never_answers_is_a_timeout() {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((s, _)) = l.accept().await {
            held.push(s);
        }
    });
    let net = ClientSet::for_config(&Config::default());
    assert_eq!(
        kind_of(
            &net,
            &format!("https://127.0.0.1:{}/", port),
            Some(Duration::from_millis(300))
        )
        .await,
        FailureKind::Timeout
    );
}

#[tokio::test]
async fn an_untrusted_certificate_is_a_certificate_problem() {
    let cert = tls_server::self_signed(&["127.0.0.1"]);
    let port = tls_server::serve_tls(axum::Router::new(), &cert).await;
    let net = ClientSet::for_config(&Config::default());
    assert_eq!(
        kind_of(&net, &format!("https://127.0.0.1:{}/", port), None).await,
        FailureKind::Certificate
    );
}

#[tokio::test]
async fn a_proxy_that_asks_for_a_login_or_refuses_is_a_proxy_problem() {
    for mode in [Mode::Login("u".into(), "p".into()), Mode::Status(403)] {
        let p = proxy::start(mode).await;
        let mut config = Config::default();
        config.network.proxy.url = Some(p.url());
        let net = ClientSet::for_config(&config);
        assert_eq!(
            kind_of(&net, "https://api.openai.com/v1/chat/completions", None).await,
            FailureKind::Proxy
        );
    }
}
