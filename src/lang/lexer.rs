//! Turns source text into logical lines of tokens. Indentation is two spaces per level.

use super::ast::TextPart;
use crate::diagnostics::Diagnostic;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Word(String),
    Number(f64),
    Str(Vec<TextPart>),
    Sym(char),
}

#[derive(Debug, Clone)]
pub struct Token {
    pub tok: Tok,
    /// 1-based column.
    pub col: usize,
}

impl Token {
    /// How the token reads in an error message.
    pub fn describe(&self) -> String {
        match &self.tok {
            Tok::Word(w) => format!("`{}`", w),
            Tok::Number(n) => format!("`{}`", n),
            Tok::Str(_) => "a piece of text".to_string(),
            Tok::Sym(c) => format!("`{}`", c),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Line {
    pub number: usize,
    /// Indentation level (each level is two spaces).
    pub indent: usize,
    pub tokens: Vec<Token>,
    /// Text after `#`, if any.
    pub comment: Option<String>,
}

pub fn lex(file: &str, source: &Arc<str>) -> Result<Vec<Line>, Diagnostic> {
    let mut lines = Vec::new();
    for (index, raw) in source.split('\n').enumerate() {
        let number = index + 1;
        let raw = raw.trim_end_matches('\r');
        let fail = |col: usize, msg: String, fix: &str| {
            Diagnostic::new(msg)
                .at(file, number, col)
                .with_source(source.clone())
                .fix(fix)
        };
        if raw.trim().is_empty() {
            continue;
        }
        if let Some(pos) = raw
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .position(|c| c == '\t')
        {
            return Err(fail(
                pos + 1,
                "this line is indented with a tab".to_string(),
                "use two spaces for each level of indentation instead of tabs",
            ));
        }
        let spaces = raw.chars().take_while(|c| *c == ' ').count();
        let chars: Vec<char> = raw.chars().collect();
        if chars.get(spaces) == Some(&'#') {
            continue;
        }
        if spaces % 2 != 0 {
            return Err(fail(
                1,
                format!(
                    "this line is indented by {} spaces, but each level needs exactly 2",
                    spaces
                ),
                "indent with 2, 4, 6 ... spaces",
            ));
        }
        let mut tokens = Vec::new();
        let mut comment = None;
        let mut i = spaces;
        while i < chars.len() {
            let c = chars[i];
            let col = i + 1;
            if c == ' ' {
                i += 1;
            } else if c == '#' {
                comment = Some(chars[i + 1..].iter().collect::<String>().trim().to_string());
                break;
            } else if c == '"' {
                let (parts, next) = lex_string(&chars, i, |msg, fix| fail(col, msg, fix))?;
                tokens.push(Token {
                    tok: Tok::Str(parts),
                    col,
                });
                i = next;
            } else if c.is_ascii_digit() {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    // A dot only belongs to the number if a digit follows it.
                    if chars[i] == '.' && !chars.get(i + 1).is_some_and(|n| n.is_ascii_digit()) {
                        break;
                    }
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                let n = text.parse::<f64>().map_err(|_| {
                    fail(
                        col,
                        format!("`{}` is not a number I can read", text),
                        "write it like 5 or 2.5",
                    )
                })?;
                tokens.push(Token {
                    tok: Tok::Number(n),
                    col,
                });
            } else if c.is_alphabetic() || c == '_' {
                let start = i;
                while i < chars.len()
                    && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '-')
                {
                    i += 1;
                }
                tokens.push(Token {
                    tok: Tok::Word(chars[start..i].iter().collect()),
                    col,
                });
            } else if ".:=,[]".contains(c) {
                tokens.push(Token {
                    tok: Tok::Sym(c),
                    col,
                });
                i += 1;
            } else {
                return Err(fail(
                    col,
                    format!("I do not understand the character `{}`", c),
                    "remove it, or put it inside quotes if it is part of a text",
                ));
            }
        }
        lines.push(Line {
            number,
            indent: spaces / 2,
            tokens,
            comment,
        });
    }
    Ok(lines)
}

/// Reads a quoted text starting at `chars[start] == '"'`. Returns the parts and the next index.
fn lex_string(
    chars: &[char],
    start: usize,
    fail: impl Fn(String, &str) -> Diagnostic,
) -> Result<(Vec<TextPart>, usize), Diagnostic> {
    let mut parts: Vec<TextPart> = Vec::new();
    let mut lit = String::new();
    let mut i = start + 1;
    loop {
        let Some(&c) = chars.get(i) else {
            return Err(fail(
                "this text starts with a quote but never ends".to_string(),
                "add a closing quote (\") at the end of the text",
            ));
        };
        match c {
            '"' => {
                i += 1;
                break;
            }
            '\\' => {
                let Some(&n) = chars.get(i + 1) else {
                    return Err(fail(
                        "this text ends with a backslash".to_string(),
                        "remove the backslash, or write \\\\ for a real one",
                    ));
                };
                lit.push(match n {
                    'n' => '\n',
                    't' => '\t',
                    other => other,
                });
                i += 2;
            }
            '{' => {
                let close = chars[i..].iter().position(|c| *c == '}');
                let Some(close) = close else {
                    return Err(fail(
                        "a { in this text is never closed".to_string(),
                        "close it with }, or write \\{ if you want a real brace",
                    ));
                };
                let inner: String = chars[i + 1..i + close].iter().collect();
                let path: Vec<String> = inner
                    .trim()
                    .split('.')
                    .map(|s| s.trim().to_string())
                    .collect();
                let valid = !inner.trim().is_empty()
                    && path.iter().all(|p| {
                        !p.is_empty()
                            && p.chars()
                                .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
                    });
                if !valid {
                    return Err(fail(
                        format!(
                            "`{{{}}}` inside this text is not a name I can fill in",
                            inner
                        ),
                        "put a name between the braces, like {city} or {forecast.summary}",
                    ));
                }
                if !lit.is_empty() {
                    parts.push(TextPart::Lit(std::mem::take(&mut lit)));
                }
                parts.push(TextPart::Var(path));
                i += close + 1;
            }
            other => {
                lit.push(other);
                i += 1;
            }
        }
    }
    if !lit.is_empty() || parts.is_empty() {
        parts.push(TextPart::Lit(lit));
    }
    Ok((parts, i))
}
