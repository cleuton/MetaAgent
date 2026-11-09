//! The Agent Card: how other agents discover what a Metagente agent can do.

use super::PROTOCOL_VERSION;
use crate::lang::AgentDef;
use serde_json::{Value, json};

/// `url` is where this agent's JSON-RPC endpoint lives.
pub fn agent_card(def: &AgentDef, url: &str) -> Value {
    agent_card_for(def, url, None)
}

/// 0.1.4: like `agent_card`; when some message of the agent is protected, the card declares the Bearer requirement the
/// way A2A 1.0 provides: one scheme, and a requirement on each protected skill. An agent with no protected message gets
/// exactly the card of 0.1.3.
pub fn agent_card_for(
    def: &AgentDef,
    url: &str,
    policy: Option<&crate::runtime::auth::AuthPolicy>,
) -> Value {
    let protected = |message: &str| policy.is_some_and(|p| p.is_protected(&def.name, message));
    let skills: Vec<Value> = def
        .accepts
        .iter()
        .map(|a| {
            let usage = if a.params.is_empty() {
                a.message.clone()
            } else {
                format!("{} {}", a.message, a.params.iter().map(|p| format!("{}=...", p)).collect::<Vec<_>>().join(" "))
            };
            let mut skill = json!({
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
            });
            if protected(&a.message) {
                skill["securityRequirements"] = json!([{"schemes": {"bearer": {"list": []}}}]); // 0.1.4
            }
            skill
        })
        .collect();
    let any_protected = def.accepts.iter().any(|a| protected(&a.message));
    let mut card = json!({
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
    });
    if any_protected {
        // 0.1.4: no `bearerFormat`: Metagente never opens the token, so it does not say what kind it is
        card["securitySchemes"] = json!({"bearer": {"httpAuthSecurityScheme": {
            "scheme": "Bearer",
            "description": "Send the header Authorization: Bearer <token>"
        }}});
    }
    card
}
