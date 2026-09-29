use metagente::lang::{Expr, Stmt, ToolKind, parse_file};

const WEATHER: &str = r#"agent Weather
  goal "Answer questions about the weather"
  tool weather from mcp "npx -y weather-mcp"
  accepts ask city
  on ask
    forecast = weather.forecast city: city
    reply "In {city} it will be {forecast.summary}"
"#;

fn parse(text: &str) -> Result<Vec<metagente::lang::AgentDef>, String> {
    parse_file("test.ag", None, text).map_err(|d| d.render())
}

fn err(text: &str) -> String {
    parse(text).expect_err("expected a problem")
}

#[test]
fn parses_the_reference_agent() {
    let agents = parse(WEATHER).unwrap();
    assert_eq!(agents.len(), 1);
    let a = &agents[0];
    assert_eq!(a.name, "Weather");
    assert_eq!(
        a.goal.as_ref().unwrap().0,
        "Answer questions about the weather"
    );
    assert_eq!(a.tools.len(), 1);
    assert!(
        matches!(&a.tools[0].kind, ToolKind::Mcp { command } if command == "npx -y weather-mcp")
    );
    assert_eq!(a.accepts[0].message, "ask");
    assert_eq!(a.accepts[0].params, vec!["city"]);
    let handler = a.handler("ask").unwrap();
    assert_eq!(handler.body.len(), 2);
    match &handler.body[0] {
        Stmt::Assign {
            name,
            value: Expr::Call(call),
            ..
        } => {
            assert_eq!(name, "forecast");
            assert_eq!(call.target, "weather");
            assert_eq!(call.action, "forecast");
            assert_eq!(call.args.len(), 1);
        }
        other => panic!("unexpected {:?}", other),
    }
}

#[test]
fn parses_several_agents_and_all_declarations() {
    let text = r#"agent A
  goal "one"
  tool file "data/"
  tool http
  tool state
  tool clock
  tool env "HOME" "USER"
  link B
  link C from "other/c.ag"
  remote Bob at "https://example.org"
  accepts go a b  # do things
  on start
    reply "hi"
  on go
    if a is b and not a is 3
      reply "same"
    otherwise
      for x in [1, 2, 3]
        file.write path: "x" text: x within 5 seconds
      reply think "what now"
agent B
  goal "two"
"#;
    let agents = parse(text).unwrap();
    assert_eq!(agents.len(), 2);
    let a = &agents[0];
    assert_eq!(a.tools.len(), 5);
    assert_eq!(a.links.len(), 2);
    assert_eq!(a.links[1].path.as_deref(), Some("other/c.ag"));
    assert_eq!(a.remotes[0].url, "https://example.org");
    assert_eq!(a.accepts[0].description.as_deref(), Some("do things"));
    assert!(a.start.is_some());
    assert!(matches!(&a.tools[4].kind, ToolKind::Env { names } if names == &["HOME", "USER"]));
    assert!(matches!(&a.tools[0].kind, ToolKind::File { scope: Some(s) } if s == "data/"));
}

#[test]
fn bare_tool_env_is_an_error_with_a_fix() {
    let e = err("agent A\n  goal \"x\"\n  tool env\n");
    assert!(e.contains("Problem on line 3 of test.ag"), "{}", e);
    assert!(e.contains("tool env \"HOME\""), "{}", e);
}

#[test]
fn unknown_word_suggests_the_closest_keyword() {
    let e = err("agent A\n  gaol \"x\"\n");
    assert!(e.contains("`gaol`"), "{}", e);
    assert!(e.contains("did you mean `goal`?"), "{}", e);
}

#[test]
fn unknown_builtin_tool_lists_the_real_ones() {
    let e = err("agent A\n  tool fille\n");
    assert!(e.contains("did you mean `tool file`?"), "{}", e);
}

#[test]
fn unterminated_text_is_reported_on_its_line() {
    let e = err("agent A\n  goal \"never ends\n");
    assert!(e.contains("line 2"), "{}", e);
    assert!(e.contains("never ends"), "{}", e);
    assert!(e.contains("closing quote"), "{}", e);
}

#[test]
fn odd_indentation_and_tabs_are_explained() {
    let e = err("agent A\n   goal \"x\"\n");
    assert!(e.contains("3 spaces"), "{}", e);
    let e = err("agent A\n\tgoal \"x\"\n");
    assert!(e.contains("tab"), "{}", e);
}

#[test]
fn file_must_start_with_agent() {
    let e = err("goal \"x\"\n");
    assert!(e.contains("agent"), "{}", e);
    let e = err("\n\n");
    assert!(e.contains("no agent"), "{}", e);
}

#[test]
fn empty_handler_and_stray_otherwise_are_errors() {
    let e = err("agent A\n  goal \"x\"\n  on go\n");
    assert!(e.contains("nothing under it"), "{}", e);
    let e = err("agent A\n  on go\n    otherwise\n      reply \"x\"\n");
    assert!(
        e.contains("`otherwise` must come right after an `if`"),
        "{}",
        e
    );
}

#[test]
fn extra_tokens_are_reported() {
    let e = err("agent A\n  goal \"x\" \"y\"\n");
    assert!(e.contains("did not expect"), "{}", e);
}

#[test]
fn errors_never_mention_rust() {
    for bad in [
        "agent\n",
        "agent A\n  on go\n    x = \n",
        "agent A\n  tool a from b\n",
        "agent A\n  on go\n    ???\n",
    ] {
        let e = err(bad);
        for word in ["panicked", "unwrap", "Result", "src/", "rust"] {
            assert!(
                !e.to_lowercase().contains(&word.to_lowercase()),
                "{} in {}",
                word,
                e
            );
        }
    }
}

#[test]
fn errors_show_the_offending_line_with_a_marker() {
    let e = err("agent A\n  gaol \"x\"\n");
    assert!(e.contains("2 |   gaol \"x\""), "{}", e);
    assert!(e.contains("^"), "{}", e);
    assert!(!e.contains('╭'), "{}", e);
}

#[test]
fn hyphenated_names_and_dotted_actions_can_call_real_mcp_tools() {
    let text = "agent A\n  goal \"x\"\n  tool gh from mcp \"gh-mcp\"\n  accepts go\n  on go\n    a = gh.create-issue title: \"t\"\n    b = gh.Weather.ask city: \"x\" within 5 seconds\n    reply b\n";
    let agents = parse(text).unwrap();
    let body = &agents[0].handler("go").unwrap().body;
    match (&body[0], &body[1]) {
        (
            Stmt::Assign {
                value: Expr::Call(a),
                ..
            },
            Stmt::Assign {
                value: Expr::Call(b),
                ..
            },
        ) => {
            assert_eq!(a.action, "create-issue");
            assert_eq!(b.target, "gh");
            assert_eq!(b.action, "Weather.ask");
            assert_eq!(b.within, Some(5.0));
        }
        other => panic!("{:?}", other),
    }
}
