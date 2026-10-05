//! Turns an incoming A2A message into "which message, with which values" for an agent.

use crate::lang::AgentDef;
use crate::runtime::value::Value;
use crate::tools::Args;
use serde_json::Value as Json;

fn text_of(message: &Json) -> String {
    message
        .get("parts")
        .and_then(|p| p.as_array())
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn data_of(message: &Json) -> Option<&Json> {
    message
        .get("parts")
        .and_then(|p| p.as_array())
        .and_then(|parts| parts.iter().find_map(|p| p.get("data")))
}

fn args_from_object(map: &serde_json::Map<String, Json>) -> Args {
    map.iter()
        .map(|(k, v)| (k.clone(), Value::from_json(v)))
        .collect()
}

/// Returns the agent's message and values, or a plain sentence saying why this is not supported.
pub fn interpret(
    def: &AgentDef,
    message: &Json,
    request_metadata: Option<&Json>,
) -> Result<(String, Args), String> {
    let handled = || {
        def.accepts
            .iter()
            .map(|a| a.message.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let not_supported = |what: &str| {
        format!(
            "agent {} does not handle `{}`; it handles: {}",
            def.name,
            what,
            handled()
        )
    };
    // 1. The skill is named in metadata, or in a data part.
    let named = message
        .pointer("/metadata/skill")
        .or_else(|| request_metadata.and_then(|m| m.get("skill")))
        .and_then(|s| s.as_str())
        .map(String::from);
    let data = data_of(message);
    let named = named.or_else(|| {
        data.and_then(|d| d.get("skill").or_else(|| d.get("message")))
            .and_then(|s| s.as_str())
            .map(String::from)
    });
    if let Some(name) = named {
        if def.accept(&name).is_none() {
            return Err(not_supported(&name));
        }
        let args = match data {
            Some(Json::Object(map)) => match map.get("arguments") {
                Some(Json::Object(inner)) => args_from_object(inner),
                _ => {
                    let mut plain = map.clone();
                    plain.remove("skill");
                    plain.remove("message");
                    args_from_object(&plain)
                }
            },
            _ => Args::new(),
        };
        return Ok((name, args));
    }
    // 2. Plain text: "ask city=Lisbon", "ask Lisbon", or just "Lisbon" for a one message agent.
    let text = text_of(message);
    let text = text.trim();
    let mut words = text.split_whitespace();
    let first = words.next().unwrap_or("");
    let (name, rest): (String, String) = if def.accept(first).is_some() {
        (first.to_string(), text[first.len()..].trim().to_string())
    } else if def.accepts.len() == 1 {
        (def.accepts[0].message.clone(), text.to_string())
    } else {
        return Err(not_supported(if first.is_empty() {
            "an empty message"
        } else {
            first
        }));
    };
    let accept = def.accept(&name).ok_or_else(|| not_supported(&name))?;
    let mut args = Args::new();
    let pairs: Vec<(&str, &str)> = rest
        .split_whitespace()
        .filter_map(|w| w.split_once('='))
        .collect();
    let all_pairs = !rest.is_empty() && rest.split_whitespace().all(|w| w.contains('='));
    if all_pairs {
        for (k, v) in pairs {
            args.insert(k.to_string(), Value::text(v));
        }
    } else if accept.params.len() == 1 && !rest.is_empty() {
        args.insert(accept.params[0].clone(), Value::text(rest));
    }
    Ok((name, args))
}
