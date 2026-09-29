//! Talks to an external MCP server: connect, discover tools, check arguments, call.

use crate::runtime::value::Value;
use crate::tools::{ActionInfo, Args, ParamInfo, ToolError, closest_name};
use rmcp::model::{CallToolRequestParams, Tool as McpToolInfo};
use rmcp::service::RunningService;
use rmcp::transport::{ConfigureCommandExt, StreamableHttpClientTransport, TokioChildProcess};
use rmcp::{RoleClient, ServiceExt};
use tokio::sync::Mutex;

struct Live {
    service: RunningService<RoleClient, ()>,
    tools: Vec<McpToolInfo>,
}

pub struct McpConnection {
    command: String,
    live: Mutex<Option<Live>>,
}

/// Splits a command line into words, honoring simple quotes.
pub fn split_command(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut has_word = false;
    for c in line.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => current.push(c),
            (None, '"') | (None, '\'') => {
                quote = Some(c);
                has_word = true;
            }
            (None, c) if c.is_whitespace() => {
                if has_word || !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                    has_word = false;
                }
            }
            (None, c) => current.push(c),
        }
    }
    if has_word || !current.is_empty() {
        words.push(current);
    }
    words
}

impl McpConnection {
    pub fn new(command: String) -> McpConnection {
        McpConnection {
            command,
            live: Mutex::new(None),
        }
    }

    async fn connect(&self) -> Result<Live, ToolError> {
        let start_failed = |why: String| {
            ToolError::new(format!(
                "I could not start the tool server `{}`: {}",
                self.command, why
            ))
            .fix("check that the program is installed and the command is spelled right")
        };
        let service = if self.command.starts_with("http://") || self.command.starts_with("https://")
        {
            let transport = StreamableHttpClientTransport::from_uri(self.command.clone());
            ().serve(transport).await.map_err(|e| {
                ToolError::new(format!(
                    "I could not connect to the tool server at {}: {}",
                    self.command,
                    short(&e.to_string())
                ))
                .fix("check the address and that the server is running")
            })?
        } else {
            let words = split_command(&self.command);
            let Some((program, rest)) = words.split_first() else {
                return Err(ToolError::new("the tool server command is empty")
                    .fix("write the command in quotes, for example: tool weather from mcp \"npx -y weather-mcp\""));
            };
            let transport =
                TokioChildProcess::new(tokio::process::Command::new(program).configure(|cmd| {
                    cmd.args(rest);
                    cmd.stderr(std::process::Stdio::null());
                }))
                .map_err(|e| start_failed(e.to_string()))?;
            ().serve(transport)
                .await
                .map_err(|e| start_failed(short(&e.to_string())))?
        };
        let tools = service.list_all_tools().await.map_err(|e| {
            ToolError::new(format!(
                "the tool server `{}` did not tell me its tools: {}",
                self.command,
                short(&e.to_string())
            ))
            .fix("check that it is an MCP server")
        })?;
        Ok(Live { service, tools })
    }

    async fn with_live<T>(
        &self,
        f: impl for<'a> FnOnce(
            &'a Live,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<T, ToolError>> + Send + 'a>,
        >,
    ) -> Result<T, ToolError> {
        let mut guard = self.live.lock().await;
        if guard.is_none() {
            *guard = Some(self.connect().await?);
        }
        let live = guard
            .as_ref()
            .ok_or_else(|| ToolError::new("the tool server is not connected"))?;
        let outcome = f(live).await;
        if let Err(e) = &outcome {
            if e.related.iter().any(|r| r == "connection-lost") {
                *guard = None;
            }
        }
        outcome
    }

    pub async fn actions(&self) -> Result<Vec<ActionInfo>, ToolError> {
        self.with_live(|live| {
            Box::pin(async move { Ok(live.tools.iter().map(to_action).collect()) })
        })
        .await
    }

    pub async fn call(
        &self,
        tool_name: &str,
        action: &str,
        args: Args,
    ) -> Result<Value, ToolError> {
        let tool_name = tool_name.to_string();
        let action = action.to_string();
        self.with_live(move |live| {
            Box::pin(async move {
                let Some(info) = live.tools.iter().find(|t| t.name == action.as_str()) else {
                    let names: Vec<&str> = live.tools.iter().map(|t| t.name.as_ref()).collect();
                    return Err(crate::tools::unknown_action(&tool_name, &action, &names));
                };
                let arguments = check_arguments(&action, &info.input_schema, &args)?;
                let params = CallToolRequestParams::new(action.clone()).with_arguments(arguments);
                match live.service.call_tool(params).await {
                    Ok(result) => result_to_value(&action, result),
                    Err(rmcp::ServiceError::McpError(e)) => {
                        Err(ToolError::new(format!("the tool `{}` refused the call: {}", action, e.message))
                            .fix("check the values you sent against what the tool expects"))
                    }
                    Err(e) => Err(ToolError::new(format!(
                        "the tool server stopped answering while running `{}` ({})",
                        action,
                        short(&e.to_string())
                    ))
                    .fix("make sure the server is running, then try again; I will reconnect on the next call")
                    .related(["connection-lost".to_string()])),
                }
            })
        })
        .await
        .map_err(|mut e| {
            e.related.retain(|r| r != "connection-lost");
            e
        })
    }
}

fn short(text: &str) -> String {
    let first = text.lines().next().unwrap_or("").trim();
    if first.chars().count() > 160 {
        format!("{}...", first.chars().take(160).collect::<String>())
    } else {
        first.to_string()
    }
}

fn to_action(tool: &McpToolInfo) -> ActionInfo {
    let schema = serde_json::Value::Object((*tool.input_schema).clone());
    let required: Vec<String> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let params = schema
        .get("properties")
        .and_then(|p| p.as_object())
        .map(|props| {
            props
                .keys()
                .map(|k| ParamInfo {
                    name: k.clone(),
                    required: required.contains(k),
                })
                .collect()
        })
        .unwrap_or_default();
    ActionInfo {
        name: tool.name.to_string(),
        description: tool
            .description
            .as_ref()
            .map(|d| d.to_string())
            .unwrap_or_default(),
        params,
        schema: Some(schema),
    }
}

/// Checks the values against the tool's own schema before anything is sent.
pub fn check_arguments(
    action: &str,
    schema: &serde_json::Map<String, serde_json::Value>,
    args: &Args,
) -> Result<serde_json::Map<String, serde_json::Value>, ToolError> {
    let props = schema.get("properties").and_then(|p| p.as_object());
    let required: Vec<&str> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    let expected: Vec<String> = props
        .map(|p| {
            p.iter()
                .map(|(k, v)| {
                    let t = v.get("type").and_then(|t| t.as_str()).unwrap_or("value");
                    let star = if required.contains(&k.as_str()) {
                        ""
                    } else {
                        " (optional)"
                    };
                    format!("{}: {}{}", k, t, star)
                })
                .collect()
        })
        .unwrap_or_default();
    let list = if expected.is_empty() {
        "no values".to_string()
    } else {
        expected.join(", ")
    };
    for name in &required {
        if !args.contains_key(*name) {
            return Err(
                ToolError::new(format!("`{}` needs a value for `{}`", action, name))
                    .fix(format!("`{}` expects: {}", action, list)),
            );
        }
    }
    let mut out = serde_json::Map::new();
    for (key, value) in args {
        let Some(prop) = props.and_then(|p| p.get(key)) else {
            if props.is_some() {
                let names: Vec<&str> = props
                    .map(|p| p.keys().map(String::as_str).collect())
                    .unwrap_or_default();
                let mut e = ToolError::new(format!("`{}` does not take `{}`", action, key));
                e = match closest_name(key, names.iter().copied()) {
                    Some(s) => e.fix(format!(
                        "did you mean `{}`? `{}` expects: {}",
                        s, action, list
                    )),
                    None => e.fix(format!("`{}` expects: {}", action, list)),
                };
                return Err(e);
            }
            out.insert(key.clone(), value.to_json());
            continue;
        };
        let wanted = prop.get("type").and_then(|t| t.as_str());
        let json = match (wanted, value) {
            (Some("number") | Some("integer"), Value::Text(t)) => match t.trim().parse::<f64>() {
                Ok(n) => Value::Number(n).to_json(),
                Err(_) => {
                    return Err(ToolError::new(format!(
                        "`{}` for `{}` must be a number, but I got \"{}\"",
                        key, action, t
                    ))
                    .fix(format!("`{}` expects: {}", action, list)));
                }
            },
            (Some("boolean"), Value::Text(t)) => match t.as_str() {
                "yes" | "true" => serde_json::Value::Bool(true),
                "no" | "false" => serde_json::Value::Bool(false),
                _ => {
                    return Err(ToolError::new(format!(
                        "`{}` for `{}` must be yes or no, but I got \"{}\"",
                        key, action, t
                    ))
                    .fix(format!("`{}` expects: {}", action, list)));
                }
            },
            (Some("string"), other) if !matches!(other, Value::Text(_)) => {
                serde_json::Value::String(other.to_display())
            }
            _ => value.to_json(),
        };
        out.insert(key.clone(), json);
    }
    Ok(out)
}

fn result_to_value(action: &str, result: rmcp::model::CallToolResult) -> Result<Value, ToolError> {
    let mut texts = Vec::new();
    for block in &result.content {
        if let Ok(json) = serde_json::to_value(block) {
            if json.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(t) = json.get("text").and_then(|t| t.as_str()) {
                    texts.push(t.to_string());
                }
            }
        }
    }
    if result.is_error == Some(true) {
        let why = if texts.is_empty() {
            "it gave no details".to_string()
        } else {
            texts.join(" ")
        };
        return Err(
            ToolError::new(format!("the tool `{}` failed: {}", action, why))
                .fix("check the values you sent, or the tool's own documentation"),
        );
    }
    if let Some(structured) = &result.structured_content {
        return Ok(Value::from_json(structured));
    }
    match texts.len() {
        0 => Ok(Value::Nothing),
        1 => {
            let text = &texts[0];
            match serde_json::from_str::<serde_json::Value>(text) {
                Ok(json @ (serde_json::Value::Object(_) | serde_json::Value::Array(_))) => {
                    Ok(Value::from_json(&json))
                }
                _ => Ok(Value::Text(text.clone())),
            }
        }
        _ => Ok(Value::Text(texts.join("\n"))),
    }
}
