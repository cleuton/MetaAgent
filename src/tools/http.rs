//! `tool http`: web requests.

use super::{ActionInfo, Args, Tool, ToolError, need_text, unknown_action};
use crate::runtime::task::CallContext;
use crate::runtime::value::Value;
use async_trait::async_trait;

pub struct HttpTool {
    net: std::sync::Arc<crate::runtime::net::ClientSet>, // 0.1.3: certificates and proxies come from [network]
    caller: String, // 0.1.3: the agent that makes the calls, for the secure rule
}

impl HttpTool {
    pub fn new(net: std::sync::Arc<crate::runtime::net::ClientSet>, caller: String) -> HttpTool {
        HttpTool { net, caller }
    }

    /// 0.1.3: an `http://` address is refused when `http` is listed in `[secure] tools` or the agent is secure.
    fn guard(&self, inherited: bool, url: &str) -> Result<(), ToolError> {
        crate::runtime::secure::decide(
            &self.net.secure(),
            &crate::runtime::secure::Caller::new(&self.caller, inherited),
            crate::runtime::secure::Target::Tool("http"),
            url,
        )
    }

    async fn finish(
        &self,
        url: &str,
        request: reqwest::RequestBuilder,
    ) -> Result<Value, ToolError> {
        let response = match request.send().await {
            Ok(r) => r,
            Err(e) => {
                return Err(self
                    .net
                    .explain(
                        &e,
                        url,
                        "",
                        "check the address and your internet connection",
                        crate::runtime::net::Purpose::Agents,
                    )
                    .await);
            }
        };
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

    async fn call(&self, action: &str, args: Args, ctx: &CallContext) -> Result<Value, ToolError> {
        match action {
            "get" => {
                let url = need_text("http", "get", &args, "url")?;
                check_url(&url)?;
                self.guard(ctx.secure, &url)?; // 0.1.3
                let client = self.net.agents_for(&url)?;
                self.finish(&url, client.get(&url)).await
            }
            "post" => {
                let url = need_text("http", "post", &args, "url")?;
                check_url(&url)?;
                self.guard(ctx.secure, &url)?; // 0.1.3
                let client = self.net.agents_for(&url)?;
                let request = match args.get("body") {
                    Some(Value::Text(t)) => client.post(&url).body(t.clone()),
                    Some(Value::Nothing) | None => client.post(&url),
                    Some(other) => client.post(&url).json(&other.to_json()),
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
