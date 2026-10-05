//! `tool http`: web requests.

use super::{ActionInfo, Args, Tool, ToolError, need_text, unknown_action};
use crate::runtime::task::CallContext;
use crate::runtime::value::Value;
use async_trait::async_trait;

pub struct HttpTool {
    client: reqwest::Client,
}

impl HttpTool {
    pub fn new(client: reqwest::Client) -> HttpTool {
        HttpTool { client }
    }

    async fn finish(
        &self,
        url: &str,
        request: reqwest::RequestBuilder,
    ) -> Result<Value, ToolError> {
        let response = request.send().await.map_err(|e| {
            let why = if e.is_connect() {
                "the connection failed".to_string()
            } else if e.is_timeout() {
                "the server took too long".to_string()
            } else {
                "the request did not complete".to_string()
            };
            ToolError::new(format!("I could not reach {}: {}", url, why))
                .fix("check the address and your internet connection")
        })?;
        let status = response.status().as_u16();
        let text = response.text().await.map_err(|_| {
            ToolError::new(format!(
                "{} answered, but I could not read the answer as text",
                url
            ))
        })?;
        let json = serde_json::from_str::<serde_json::Value>(&text)
            .map(|j| Value::from_json(&j))
            .unwrap_or(Value::Nothing);
        Ok(Value::record([
            ("status".to_string(), Value::Number(status as f64)),
            ("text".to_string(), Value::Text(text)),
            ("json".to_string(), json),
        ]))
    }
}

fn check_url(url: &str) -> Result<(), ToolError> {
    if url.starts_with("http://") || url.starts_with("https://") {
        Ok(())
    } else {
        Err(ToolError::new(format!("`{}` is not a web address", url))
            .fix("start the address with http:// or https://"))
    }
}

#[async_trait]
impl Tool for HttpTool {
    fn name(&self) -> &str {
        "http"
    }

    async fn actions(&self) -> Result<Vec<ActionInfo>, ToolError> {
        Ok(describe())
    }

    async fn call(&self, action: &str, args: Args, _ctx: &CallContext) -> Result<Value, ToolError> {
        match action {
            "get" => {
                let url = need_text("http", "get", &args, "url")?;
                check_url(&url)?;
                self.finish(&url, self.client.get(&url)).await
            }
            "post" => {
                let url = need_text("http", "post", &args, "url")?;
                check_url(&url)?;
                let request = match args.get("body") {
                    Some(Value::Text(t)) => self.client.post(&url).body(t.clone()),
                    Some(Value::Nothing) | None => self.client.post(&url),
                    Some(other) => self.client.post(&url).json(&other.to_json()),
                };
                self.finish(&url, request).await
            }
            other => Err(unknown_action("http", other, &["get", "post"])),
        }
    }
}

/// What this tool can do, also used by `metagente check`.
pub fn describe() -> Vec<ActionInfo> {
    vec![
        ActionInfo::simple("get", "Fetch a web address", &[("url", true)]),
        ActionInfo::simple(
            "post",
            "Send data to a web address",
            &[("url", true), ("body", false)],
        ),
    ]
}
