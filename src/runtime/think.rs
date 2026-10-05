//! `think`: ask the language model, letting it use the agent's declared tools.

use super::interpreter::Agent;
use super::task::CallContext;
use super::value::Value;
use crate::diagnostics::MgResult;
use crate::lang::Span;
use crate::llm::{ChatMsg, LlmReply, LlmRequest, ToolCallReq, ToolSpec};
use crate::tools::Args;
use std::time::Duration;

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub async fn think(
    agent: &Agent,
    prompt: String,
    ctx: &CallContext,
    span: Span,
) -> MgResult<Value> {
    let Some(llm) = agent.rt.llm.clone() else {
        return Err(agent
            .diag(span, "this agent needs a language model to think, and none is set up")
            .fix("run `metagente new` to create a metagente.toml, or add an [llm] section to it (see the tutorial)"));
    };
    // What the model may use: exactly the tools this agent declared.
    let mut specs = Vec::new();
    let mut names: Vec<&String> = agent.tools.keys().collect();
    names.sort();
    for name in names {
        let tool = &agent.tools[name];
        let actions = tool.actions().await.map_err(|e| {
            let mut d = agent.diag(
                span,
                format!("I could not list what `{}` can do: {}", name, e.message),
            );
            d.suggestion = e.fix;
            d
        })?;
        for action in actions {
            specs.push(ToolSpec {
                name: format!("{}__{}", sanitize(name), sanitize(&action.name)),
                description: format!("{}.{}: {}", name, action.name, action.description),
                schema: action.json_schema(),
            });
        }
    }
    let goal = agent
        .def
        .goal
        .as_ref()
        .map(|g| g.0.as_str())
        .unwrap_or("help the user");
    let system = format!(
        "You are an agent called {}. Your goal: {}. Use the tools you have when they help, and give a short, direct answer.",
        agent.def.name, goal
    );
    let max_steps = agent.rt.config.think_max_steps.max(1);
    let mut messages = vec![ChatMsg::User(prompt)];
    for _ in 0..max_steps {
        let request = LlmRequest {
            system: system.clone(),
            messages: messages.clone(),
            tools: specs.clone(),
        };
        let reply = tokio::time::timeout(
            Duration::from_secs(agent.rt.config.timeout_seconds.max(1) * 4),
            llm.complete(&request),
        )
        .await
        .map_err(|_| {
            agent
                .diag(span, "the language model took too long to answer")
                .fix("try again in a moment")
        })?
        .map_err(|e| {
            agent
                .diag(span, format!("the language model could not answer: {}", e))
                .fix("check the [llm] settings in metagente.toml and that the key variable is set")
        })?;
        match reply {
            LlmReply::Text(text) => return Ok(Value::Text(text)),
            LlmReply::ToolCall(call) => {
                let result = run_tool_call(agent, &call, ctx).await;
                messages.push(ChatMsg::Assistant {
                    text: None,
                    tool_call: Some(call.clone()),
                });
                messages.push(ChatMsg::ToolResult {
                    id: call.id.clone(),
                    name: call.name.clone(),
                    content: result,
                });
            }
        }
    }
    Err(agent
        .diag(
            span,
            format!("the agent used {} steps and did not finish", max_steps),
        )
        .fix("make the goal more specific, or raise think_max_steps in metagente.toml"))
}

/// Runs one tool the model asked for. Problems go back to the model as text, not to the user.
async fn run_tool_call(agent: &Agent, call: &ToolCallReq, ctx: &CallContext) -> String {
    let (target, action) = match call.name.split_once("__") {
        Some((t, a)) => (t, a),
        None => {
            return format!(
                "error: `{}` is not a tool name of the form target__action",
                call.name
            );
        }
    };
    let real_target = agent.tools.keys().find(|k| sanitize(k) == target).cloned();
    let Some(real_target) = real_target else {
        return format!("error: this agent has no tool called `{}`", target);
    };
    let mut args = Args::new();
    if let serde_json::Value::Object(map) = &call.args {
        for (k, v) in map {
            args.insert(k.clone(), Value::from_json(v));
        }
    }
    let real_action = match agent.tools[&real_target].actions().await {
        Ok(list) => list
            .into_iter()
            .find(|a| sanitize(&a.name) == action)
            .map(|a| a.name),
        Err(_) => None,
    }
    .unwrap_or_else(|| action.to_string());
    let seconds = agent.rt.config.timeout_seconds.max(1);
    let outcome = tokio::time::timeout(
        Duration::from_secs(seconds),
        agent.tools[&real_target].call(&real_action, args, ctx),
    )
    .await;
    match outcome {
        Ok(Ok(Value::Text(t))) => t,
        Ok(Ok(other)) => other.to_json().to_string(),
        Ok(Err(e)) => format!("error: {}", e.message),
        Err(_) => format!(
            "error: {}.{} did not finish within {} seconds",
            real_target, real_action, seconds
        ),
    }
}
