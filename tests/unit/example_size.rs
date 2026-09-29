#[test]
fn the_reference_weather_agent_has_fewer_than_ten_lines() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/weather.ag"))
        .unwrap();
    let lines = text.lines().count();
    assert!(lines < 10, "examples/weather.ag has {} lines", lines);
}
