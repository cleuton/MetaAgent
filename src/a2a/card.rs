//! The Agent Card: how other agents discover what a Metagente agent can do.

use super::PROTOCOL_VERSION;
use crate::lang::AgentDef;
use serde_json::{Value, json};

/// `url` is where this agent's JSON-RPC endpoint lives.
pub fn agent_card(def: &AgentDef, url: &str) -> Value {
    let skills: Vec<Value> = def
        .accepts
        .iter()
        .map(|a| {
            let usage = if a.params.is_empty() {
                a.message.clone()
            } else {
                format!("{} {}", a.message, a.params.iter().map(|p| format!("{}=...", p)).collect::<Vec<_>>().join(" "))
            };
            json!({
                "id": a.message,
                "name": a.message,
                "description": a.description.clone().unwrap_or_else(|| {
                    if a.params.is_empty() {
                        format!("Ask {} to {}", def.name, a.message)
                    } else {
                        format!("Ask {} to {} (needs: {})", def.name, a.message, a.params.join(", "))
                    }
                }),
                "tags": [def.name, "metagente"],
                "examples": [usage],
                "inputModes": ["text/plain", "application/json"],
                "outputModes": ["text/plain", "application/json"],
            })
        })
        .collect();
    json!({
        "name": def.name,
        "description": def.goal.as_ref().map(|g| g.0.clone()).unwrap_or_else(|| format!("Metagente agent {}", def.name)),
        "supportedInterfaces": [{
            "url": url,
            "protocolBinding": "JSONRPC",
            "protocolVersion": PROTOCOL_VERSION,
        }],
        "version": env!("CARGO_PKG_VERSION"),
        "capabilities": {
            "streaming": false,
            "pushNotifications": false,
            "extendedAgentCard": false,
        },
        "defaultInputModes": ["text/plain", "application/json"],
        "defaultOutputModes": ["text/plain", "application/json"],
        "skills": skills,
    })
}
