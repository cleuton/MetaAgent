// 0.1.5: new file. Fake Anthropic and OpenAI answering for their OFFICIAL host names, reached only through settings the
// program already has: a CONNECT proxy in [network] (Mode::Route pipes it to a local TLS server) and ca_file for a leaf
// certificate valid for api.openai.com and api.anthropic.com. Nothing is added to the program. Test code only.
use super::proxy::{self, Mode, ProxyLog};
use super::tls_server;
use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use metagente::runtime::config::Config;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// One request the fake server saw.
#[derive(Debug, Clone)]
pub struct Seen {
    pub path: String,
    pub host: String,
    pub authorization: Option<String>,
    pub x_api_key: Option<String>,
    pub anthropic_version: Option<String>,
    pub user_agent: String,
    pub body: Value,
}

/// What the fake answers.
#[derive(Debug, Clone)]
pub enum Reply {
    Json(u16, Value),
    Raw(u16, String),
    /// Never answers (for the timeout tests).
    Hang,
}

#[derive(Default)]
struct Inner {
    seen: Mutex<Vec<Seen>>,
    openai: Mutex<Vec<Reply>>,
    anthropic: Mutex<Vec<Reply>>,
}

pub struct Official {
    pub proxy_url: String,
    pub ca_file: PathBuf,
    pub proxy_log: ProxyLog,
    inner: Arc<Inner>,
    _dir: tempfile::TempDir,
}

fn text_reply(text: &str) -> Reply {
    Reply::Json(200, json!({"choices": [{"message": {"content": text}}]}))
}

pub fn openai_text(text: &str) -> Reply {
    text_reply(text)
}

pub fn openai_tool_call(id: &str, name: &str, args: Value) -> Reply {
    Reply::Json(
        200,
        json!({"choices": [{"message": {"content": null, "tool_calls": [{
            "id": id, "type": "function",
            "function": {"name": name, "arguments": args.to_string()}
        }]}}]}),
    )
}

pub fn anthropic_text(text: &str) -> Reply {
    Reply::Json(200, json!({"content": [{"type": "text", "text": text}]}))
}

pub fn anthropic_tool_call(id: &str, name: &str, args: Value) -> Reply {
    Reply::Json(
        200,
        json!({"content": [{"type": "tool_use", "id": id, "name": name, "input": args}]}),
    )
}

async fn handle(
    path: &'static str,
    inner: Arc<Inner>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let get = |k: &str| {
        headers
            .get(k)
            .and_then(|v| v.to_str().ok())
            .map(String::from)
    };
    let parsed: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    inner.seen.lock().unwrap().push(Seen {
        path: path.to_string(),
        host: get("host").unwrap_or_default(),
        authorization: get("authorization"),
        x_api_key: get("x-api-key"),
        anthropic_version: get("anthropic-version"),
        user_agent: get("user-agent").unwrap_or_default(),
        body: parsed,
    });
    let queue = if path == "/v1/messages" {
        &inner.anthropic
    } else {
        &inner.openai
    };
    let reply = {
        let mut q = queue.lock().unwrap();
        if q.len() > 1 {
            q.remove(0)
        } else {
            q.first().cloned().unwrap_or(Reply::Json(
                500,
                json!({"error": {"message": "no scripted reply"}}),
            ))
        }
    };
    match reply {
        Reply::Json(status, v) => (
            StatusCode::from_u16(status).unwrap(),
            [(header::CONTENT_TYPE, "application/json")],
            v.to_string(),
        )
            .into_response(),
        Reply::Raw(status, text) => (
            StatusCode::from_u16(status).unwrap(),
            [(header::CONTENT_TYPE, "application/json")],
            text,
        )
            .into_response(),
        Reply::Hang => {
            tokio::time::sleep(std::time::Duration::from_secs(120)).await;
            StatusCode::GATEWAY_TIMEOUT.into_response()
        }
    }
}

impl Official {
    pub async fn start() -> Official {
        let (ca_pem, cert) =
            tls_server::signed_by_new_authority(&["api.openai.com", "api.anthropic.com"]);
        let inner = Arc::new(Inner::default());
        let app = Router::new()
            .route(
                "/v1/chat/completions",
                post(
                    |State(i): State<Arc<Inner>>, h: HeaderMap, b: Bytes| async move {
                        handle("/v1/chat/completions", i, h, b).await
                    },
                ),
            )
            .route(
                "/v1/messages",
                post(
                    |State(i): State<Arc<Inner>>, h: HeaderMap, b: Bytes| async move {
                        handle("/v1/messages", i, h, b).await
                    },
                ),
            )
            .with_state(inner.clone());
        let port = tls_server::serve_tls(app, &cert).await;
        let proxy = proxy::start(Mode::Route(port)).await;
        let dir = tempfile::tempdir().expect("temp dir");
        let ca_file = dir.path().join("ca.pem");
        std::fs::write(&ca_file, ca_pem).expect("write ca");
        Official {
            proxy_url: proxy.url(),
            ca_file,
            proxy_log: proxy.log.clone(),
            inner,
            _dir: dir,
        }
    }

    /// The replies the fake OpenAI gives, in order; the last one repeats.
    pub fn script_openai(&self, replies: Vec<Reply>) {
        *self.inner.openai.lock().unwrap() = replies;
    }

    /// The replies the fake Anthropic gives, in order; the last one repeats.
    pub fn script_anthropic(&self, replies: Vec<Reply>) {
        *self.inner.anthropic.lock().unwrap() = replies;
    }

    /// Makes `config` reach the fakes: the proxy and the authority, through [network] only.
    pub fn apply(&self, config: &mut Config) {
        config.network.proxy.url = Some(self.proxy_url.clone());
        config.network.ca_file = Some(self.ca_file.clone());
    }

    /// The `[network]` lines for a metagente.toml that does the same.
    pub fn network_toml(&self) -> String {
        format!(
            "[network]\nca_file = \"{}\"\n\n[network.proxy]\nurl = \"{}\"\n",
            self.ca_file.display().to_string().replace('\\', "/"),
            self.proxy_url
        )
    }

    pub fn seen(&self) -> Vec<Seen> {
        self.inner.seen.lock().unwrap().clone()
    }

    pub fn seen_on(&self, path: &str) -> Vec<Seen> {
        self.seen().into_iter().filter(|s| s.path == path).collect()
    }
}
