//! 0.1.4: `[version] minimum` in metagente.toml: the oldest Metagente that may run the project.

use crate::diagnostics::Diagnostic;
use std::path::Path;

pub type Triple = (u32, u32, u32);

/// 0.1.4: exactly three whole numbers separated by dots, without leading zeros (D18).
pub fn parse_minimum(text: &str) -> Option<Triple> {
    let parts: Vec<&str> = text.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let mut numbers = [0u32; 3];
    for (i, p) in parts.iter().enumerate() {
        if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        if p.len() > 1 && p.starts_with('0') {
            return None;
        }
        numbers[i] = p.parse().ok()?;
    }
    Some((numbers[0], numbers[1], numbers[2]))
}

/// 0.1.4: the version of the running program, the one `metagente --version` prints.
pub fn running() -> Triple {
    parse_minimum(env!("CARGO_PKG_VERSION")).unwrap_or((0, 0, 0))
}

fn shown(t: Triple) -> String {
    format!("{}.{}.{}", t.0, t.1, t.2)
}

/// 0.1.4: the first thing read in metagente.toml (D18). Without `[version]` nothing is checked. Returns the warnings
/// (another key in `[version]`).
pub fn check(
    table: &toml::Table,
    text: &str,
    path: &Path,
    running: Triple,
) -> Result<Vec<String>, Diagnostic> {
    let Some(section) = table.get("version") else {
        return Ok(Vec::new());
    };
    let line = |key: &str| super::config::line_of(text, "version", key);
    let located = |d: Diagnostic, line: usize| {
        if line > 0 {
            d.at(&path.display().to_string(), line, 1)
        } else {
            d
        }
    };
    let Some(section) = section.as_table() else {
        return Err(located(
            Diagnostic::new("`version` must be a section, written [version]")
                .fix("write [version] on a line of its own, then minimum = \"0.1.4\" under it"),
            super::config::line_of(text, "", "version"),
        ));
    };
    let mut warnings = Vec::new();
    for key in section.keys() {
        if key != "minimum" {
            warnings.push(format!(
                "metagente.toml has an unknown setting `{}` in [version]; I ignored it",
                key
            ));
        }
    }
    let Some(value) = section.get("minimum") else {
        return Ok(warnings);
    };
    let bad = |said: String| {
        located(
            Diagnostic::new(format!(
                "[version] minimum must be three numbers like 0.1.4, but it says {}.",
                said
            ))
            .fix("write minimum = \"0.1.4\""),
            line("minimum"),
        )
    };
    let Some(text_value) = value.as_str() else {
        return Err(bad(value.to_string()));
    };
    let Some(minimum) = parse_minimum(text_value) else {
        return Err(bad(format!("\"{}\"", text_value)));
    };
    if minimum > running {
        return Err(located(
            Diagnostic::new(format!(
                "This project needs Metagente {} or newer, and this is {}. Nothing was run.",
                shown(minimum),
                shown(running)
            ))
            .fix("download the latest release at https://github.com/cleuton/MetaAgent/releases/latest"),
            line("minimum"),
        ));
    }
    Ok(warnings)
}
