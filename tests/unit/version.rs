// 0.1.4: the shape and the order of [version] minimum (D18).
use metagente::runtime::version::{check, parse_minimum};
use std::path::Path;

#[test]
fn three_numbers_without_leading_zeros() {
    assert_eq!(parse_minimum("0.1.4"), Some((0, 1, 4)));
    assert_eq!(parse_minimum("10.20.30"), Some((10, 20, 30)));
    for bad in [
        "",
        ">=0.1.4",
        "0.1",
        "0.1.4-beta",
        "00.1.4",
        "0.01.4",
        "0.1.4.1",
        "0.1.x",
        "v0.1.4",
        " 0.1.4",
        "0..4",
        "-1.0.0",
    ] {
        assert_eq!(parse_minimum(bad), None, "{:?}", bad);
    }
}

#[test]
fn versions_compare_as_numbers() {
    assert!(parse_minimum("0.1.10") > parse_minimum("0.1.9"));
    assert!(parse_minimum("1.0.0") > parse_minimum("0.99.99"));
}

fn gate(toml_text: &str, running: (u32, u32, u32)) -> Result<Vec<String>, String> {
    let table: toml::Table = toml_text.parse().unwrap();
    check(&table, toml_text, Path::new("metagente.toml"), running).map_err(|d| d.message)
}

#[test]
fn the_gate_is_a_floor() {
    assert!(gate("[version]\nminimum = \"0.1.4\"\n", (0, 1, 4)).is_ok());
    assert!(gate("[version]\nminimum = \"0.1.4\"\n", (0, 2, 0)).is_ok());
    assert!(gate("", (0, 0, 1)).is_ok(), "no section, no check");
    let err = gate("[version]\nminimum = \"0.1.5\"\n", (0, 1, 4)).unwrap_err();
    assert_eq!(
        err,
        "This project needs Metagente 0.1.5 or newer, and this is 0.1.4. Nothing was run."
    );
}

#[test]
fn a_wrong_shape_shows_the_right_one_and_another_key_is_a_warning() {
    let err = gate("[version]\nminimum = \">=0.1.4\"\n", (0, 1, 4)).unwrap_err();
    assert_eq!(
        err,
        "[version] minimum must be three numbers like 0.1.4, but it says \">=0.1.4\"."
    );
    assert!(gate("[version]\nminimum = 3\n", (0, 1, 4)).is_err());
    let warnings = gate(
        "[version]\nminimum = \"0.1.4\"\nmaximum = \"1.0.0\"\n",
        (0, 1, 4),
    )
    .unwrap();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("`maximum` in [version]"));
}
