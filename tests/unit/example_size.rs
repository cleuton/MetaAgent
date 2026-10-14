#[test]
fn the_reference_weather_agent_has_fewer_than_ten_lines() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/weather.ag"))
        .unwrap();
    let lines = text.lines().count();
    assert!(lines < 10, "examples/weather.ag has {} lines", lines);
}

// 0.1.3: the three agents of multi-agent-samples/https-chain stay under 10 lines of code each. Blank lines and comment
// lines (including the `# 0.1.3:` marker) do not count.
#[test]
fn the_https_chain_agents_have_fewer_than_ten_lines_of_code() {
    for name in ["planner.ag", "researcher.ag", "facts.ag"] {
        let path = format!(
            "{}/multi-agent-samples/https-chain/{}",
            env!("CARGO_MANIFEST_DIR"),
            name
        );
        let text = std::fs::read_to_string(&path).unwrap();
        let code = text
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
            .count();
        assert!(code < 10, "{} has {} lines of code", name, code);
    }
}
