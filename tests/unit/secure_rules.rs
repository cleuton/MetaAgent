// 0.1.3: new file. The one rule that refuses `http://` for something declared secure (FR-005, FR-007, D3, D10).

use metagente::runtime::config::SecureConfig;
use metagente::runtime::secure::{Caller, Target, decide, is_plain_http};

fn list() -> SecureConfig {
    SecureConfig {
        agents: vec!["Bob".into(), "Researcher".into()],
        tools: vec!["kb".into(), "http".into()],
        llm: false,
        ..SecureConfig::default()
    }
}

fn free() -> Caller<'static> {
    Caller::new("Planner", false)
}

#[test]
fn a_secure_name_refuses_http_and_allows_https() {
    let l = list();
    let e = decide(&l, &free(), Target::Remote("Bob"), "http://host:8443").unwrap_err();
    assert!(
        e.message.contains("Bob is declared secure"),
        "{}",
        e.message
    );
    assert!(e.message.contains("http://host:8443"));
    assert!(e.fix.unwrap().contains("remove Bob from [secure] agents"));
    assert!(decide(&l, &free(), Target::Remote("Bob"), "https://host:8443").is_ok());
    let e = decide(&l, &free(), Target::Tool("kb"), "http://h/mcp").unwrap_err();
    assert!(e.fix.unwrap().contains("[secure] tools"));
}

#[test]
fn a_name_that_is_not_secure_may_use_both() {
    let l = list();
    assert!(decide(&l, &free(), Target::Remote("Alice"), "http://h").is_ok());
    assert!(decide(&l, &free(), Target::Remote("Alice"), "https://h").is_ok());
    assert!(decide(&l, &free(), Target::Tool("weather"), "http://h").is_ok());
}

#[test]
fn a_secure_agent_refuses_http_everywhere_it_calls() {
    let l = list();
    let me = Caller::new("Researcher", false);
    let e = decide(&l, &me, Target::Remote("Alice"), "http://h").unwrap_err();
    assert!(
        e.message
            .starts_with("Researcher is declared secure, so it may only call https addresses"),
        "{}",
        e.message
    );
    assert!(decide(&l, &me, Target::Tool("weather"), "http://h").is_err());
    assert!(decide(&l, &me, Target::Model, "http://gateway").is_err());
    assert!(decide(&l, &me, Target::Remote("Alice"), "https://h").is_ok());
}

#[test]
fn a_secure_loader_passes_the_rule_to_the_agent_it_links() {
    let l = list();
    let linked = Caller::new("Helper", true);
    assert!(decide(&l, &linked, Target::Remote("Alice"), "http://h").is_err());
    assert!(
        decide(
            &l,
            &Caller::new("Helper", false),
            Target::Remote("Alice"),
            "http://h"
        )
        .is_ok()
    );
}

#[test]
fn the_builtin_http_tool_can_be_secure() {
    let l = list();
    assert!(decide(&l, &free(), Target::Tool("http"), "http://example.com").is_err());
    assert!(decide(&l, &free(), Target::Tool("http"), "https://example.com").is_ok());
}

#[test]
fn the_model_is_secure_only_when_llm_is_on() {
    let mut l = list();
    assert!(decide(&l, &free(), Target::Model, "http://gateway").is_ok());
    l.llm = true;
    let e = decide(&l, &free(), Target::Model, "http://gateway").unwrap_err();
    assert!(e.message.contains("[secure] llm is on"));
    assert!(e.message.contains("Nothing was sent"));
    assert!(decide(&l, &free(), Target::Model, "https://api.anthropic.com").is_ok());
}

#[test]
fn addresses_that_are_not_network_addresses_are_never_checked() {
    let l = list();
    let me = Caller::new("Researcher", false);
    for address in [
        "npx -y weather-mcp",
        "./local-server",
        "files/notes.txt",
        "",
    ] {
        assert!(
            decide(&l, &me, Target::Tool("kb"), address).is_ok(),
            "{}",
            address
        );
    }
}

#[test]
fn http_is_recognised_in_any_letter_case_and_with_spaces() {
    assert!(is_plain_http("http://a"));
    assert!(is_plain_http("  HTTP://a"));
    assert!(!is_plain_http("https://a"));
    assert!(!is_plain_http("httpx://a"));
}

#[test]
fn the_address_in_a_message_never_shows_a_login() {
    let l = list();
    let e = decide(
        &l,
        &free(),
        Target::Remote("Bob"),
        "http://user:secret@host:8443/x",
    )
    .unwrap_err();
    assert!(!e.message.contains("secret"), "{}", e.message);
    assert!(!e.message.contains("user:"), "{}", e.message);
}

#[test]
fn a_name_in_both_lists_applies_each_list_to_its_own_kind() {
    let l = SecureConfig {
        agents: vec!["same".into()],
        tools: vec!["same".into()],
        ..SecureConfig::default()
    };
    assert!(decide(&l, &free(), Target::Remote("same"), "http://h").is_err());
    assert!(decide(&l, &free(), Target::Tool("same"), "http://h").is_err());
    let only_agent = SecureConfig {
        agents: vec!["same".into()],
        ..SecureConfig::default()
    };
    assert!(decide(&only_agent, &free(), Target::Tool("same"), "http://h").is_ok());
    let only_tool = SecureConfig {
        tools: vec!["same".into()],
        ..SecureConfig::default()
    };
    assert!(decide(&only_tool, &free(), Target::Remote("same"), "http://h").is_ok());
}

#[test]
fn matching_is_case_sensitive() {
    let l = list();
    assert!(decide(&l, &free(), Target::Remote("bob"), "http://h").is_ok());
}
