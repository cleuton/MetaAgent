//! The Agent Card must have every field A2A 1.0 requires, with the right types.

use crate::a2a;
use serde_json::Value;

async fn card(url: &str) -> Value {
    reqwest::get(url)
        .await
        .expect("get")
        .json()
        .await
        .expect("json")
}

fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or_else(|| panic!("`{}` must be a non-empty string in {}", key, v))
}

#[tokio::test]
async fn the_card_has_all_required_fields() {
    let dir = tempfile::tempdir().unwrap();
    let server = a2a::start(dir.path(), a2a::WEATHER, None).await;
    let card = card(&format!("{}/.well-known/agent-card.json", server.base)).await;
    assert_eq!(text(&card, "name"), "Weather");
    assert_eq!(
        text(&card, "description"),
        "Answer questions about the weather"
    );
    assert!(!text(&card, "version").is_empty());
    let interfaces = card["supportedInterfaces"]
        .as_array()
        .expect("supportedInterfaces");
    assert!(!interfaces.is_empty());
    for i in interfaces {
        assert!(text(i, "url").starts_with("http"));
        assert_eq!(text(i, "protocolBinding"), "JSONRPC");
        assert_eq!(text(i, "protocolVersion"), "1.0");
    }
    assert!(card["capabilities"].is_object());
    assert_eq!(card["capabilities"]["streaming"], false);
    for key in ["defaultInputModes", "defaultOutputModes"] {
        let modes = card[key].as_array().expect(key);
        assert!(!modes.is_empty() && modes.iter().all(|m| m.is_string()));
    }
    // camelCase only, as the spec requires.
    let raw = card.to_string();
    for snake in [
        "supported_interfaces",
        "protocol_binding",
        "default_input_modes",
    ] {
        assert!(!raw.contains(snake), "{} in {}", snake, raw);
    }
}

#[tokio::test]
async fn there_is_one_skill_per_accepts_entry() {
    let dir = tempfile::tempdir().unwrap();
    let server = a2a::start(dir.path(), a2a::WEATHER, None).await;
    let card = card(&format!("{}/.well-known/agent-card.json", server.base)).await;
    let skills = card["skills"].as_array().expect("skills");
    let ids: Vec<&str> = skills.iter().map(|s| text(s, "id")).collect();
    assert_eq!(ids, vec!["ask", "broken"]);
    for skill in skills {
        assert!(!text(skill, "name").is_empty());
        assert!(!text(skill, "description").is_empty());
        assert!(
            skill["tags"]
                .as_array()
                .map(|t| !t.is_empty())
                .unwrap_or(false)
        );
    }
    assert_eq!(skills[0]["description"], "weather for a city");
}

#[tokio::test]
async fn every_served_agent_has_its_own_card_and_the_interface_points_back_at_the_server() {
    let dir = tempfile::tempdir().unwrap();
    let source = format!(
        "{}agent Other\n  goal \"Something else\"\n  accepts hi\n  on hi\n    reply \"hi\"\n",
        a2a::WEATHER
    );
    let server = a2a::start(dir.path(), &source, None).await;
    let default = card(&format!("{}/.well-known/agent-card.json", server.base)).await;
    assert_eq!(text(&default, "name"), "Weather");
    let other = card(&format!(
        "{}/agents/Other/.well-known/agent-card.json",
        server.base
    ))
    .await;
    assert_eq!(text(&other, "name"), "Other");
    let url = text(&other["supportedInterfaces"][0], "url").to_string();
    assert_eq!(url, format!("{}/agents/Other/a2a", server.base));
    let missing = reqwest::get(format!(
        "{}/agents/Nobody/.well-known/agent-card.json",
        server.base
    ))
    .await
    .unwrap();
    assert_eq!(missing.status().as_u16(), 404);
}
