//! A tiny MCP server, built with the same SDK, served over HTTP inside the test process.

use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::json;
use tokio_util_shim::CancellationToken;

/// The SDK re-exports nothing for cancellation, so tests use a tiny channel based token.
mod tokio_util_shim {
    pub use tokio::sync::watch;
    #[derive(Clone)]
    pub struct CancellationToken(pub watch::Sender<bool>);
    impl CancellationToken {
        pub fn new() -> Self {
            CancellationToken(watch::channel(false).0)
        }
        pub fn cancel(&self) {
            let _ = self.0.send(true);
        }
        pub async fn cancelled(&self) {
            let mut rx = self.0.subscribe();
            while !*rx.borrow() {
                if rx.changed().await.is_err() {
                    break;
                }
            }
        }
    }
}

/// Every url the `fetch` tool was asked for, so tests can check what the model chose.
static FETCHED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

pub fn fetched() -> Vec<String> {
    FETCHED.lock().map(|f| f.clone()).unwrap_or_default()
}

#[derive(Clone, Default)]
pub struct FakeWeather;

fn schema(value: serde_json::Value) -> std::sync::Arc<JsonObject> {
    std::sync::Arc::new(value.as_object().cloned().unwrap_or_default())
}

impl ServerHandler for FakeWeather {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(vec![
            Tool::new(
                "forecast",
                "Forecast for a city",
                schema(
                    json!({"type":"object","properties":{"city":{"type":"string"}},"required":["city"]}),
                ),
            ),
            Tool::new(
                "sleepy",
                "Answers after a while",
                schema(
                    json!({"type":"object","properties":{"seconds":{"type":"number"}},"required":["seconds"]}),
                ),
            ),
            Tool::new(
                "boom",
                "Always fails",
                schema(json!({"type":"object","properties":{}})),
            ),
            Tool::new(
                "fetch",
                "Fetches a URL from the internet and extracts its contents (like the reference fetch server)",
                schema(json!({
                    "type": "object",
                    "properties": {
                        "url": {"type": "string"},
                        "max_length": {"type": "integer"}
                    },
                    "required": ["url"]
                })),
            ),
        ]))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let args = request.arguments.clone().unwrap_or_default();
        let result = match request.name.as_ref() {
            "forecast" => {
                let city = args
                    .get("city")
                    .and_then(|c| c.as_str())
                    .unwrap_or("nowhere");
                let body = json!({"city": city, "summary": "sunny, 24 degrees"}).to_string();
                CallToolResult::success(vec![ContentBlock::text(body)])
            }
            "sleepy" => {
                let seconds = args.get("seconds").and_then(|s| s.as_f64()).unwrap_or(0.0);
                tokio::time::sleep(std::time::Duration::from_secs_f64(seconds)).await;
                CallToolResult::success(vec![ContentBlock::text("awake")])
            }
            "fetch" => {
                let url = args
                    .get("url")
                    .and_then(|u| u.as_str())
                    .unwrap_or("")
                    .to_string();
                if let Ok(mut seen) = FETCHED.lock() {
                    seen.push(url.clone());
                }
                if url.contains("Xyzzy") {
                    CallToolResult::error(vec![ContentBlock::text(format!(
                        "Failed to fetch {} - status code 404",
                        url
                    ))])
                } else {
                    let city = url.rsplit('/').next().unwrap_or("the city");
                    CallToolResult::success(vec![ContentBlock::text(format!(
                        "Contents of {}:\n{} is a city with a river and old streets.",
                        url, city
                    ))])
                }
            }
            "boom" => {
                CallToolResult::error(vec![ContentBlock::text("the weather station is on fire")])
            }
            other => return Err(McpError::invalid_params(format!("no tool {}", other), None)),
        };
        Ok(result.into())
    }
}

/// A running fake server. Dropping it or calling `stop` shuts it down.
pub struct Running {
    pub url: String,
    stop: CancellationToken,
}

impl Running {
    pub fn stop(&self) {
        self.stop.cancel();
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}

pub async fn start() -> Running {
    let service: StreamableHttpService<FakeWeather, LocalSessionManager> =
        StreamableHttpService::new(
            || Ok(FakeWeather),
            std::sync::Arc::new(LocalSessionManager::default()),
            StreamableHttpServerConfig::default().with_sse_keep_alive(None),
        );
    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let stop = CancellationToken::new();
    let signal = stop.clone();
    tokio::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(async move { signal.cancelled().await })
            .await;
    });
    Running {
        url: format!("http://127.0.0.1:{}/mcp", port),
        stop,
    }
}

/// 0.1.3: the same server over TLS, reachable as https://localhost:PORT/mcp (the certificate must be valid for `localhost`).
pub async fn start_tls(cert: &super::tls_server::TestCert) -> Running {
    let service: StreamableHttpService<FakeWeather, LocalSessionManager> =
        StreamableHttpService::new(
            || Ok(FakeWeather),
            std::sync::Arc::new(LocalSessionManager::default()),
            StreamableHttpServerConfig::default().with_sse_keep_alive(None),
        );
    let router = axum::Router::new().nest_service("/mcp", service);
    let port = super::tls_server::serve_tls(router, cert).await;
    Running {
        url: format!("https://localhost:{}/mcp", port),
        stop: CancellationToken::new(),
    }
}
