//! Natural language errors. This is the only error shape users ever see.

pub mod internal;

use std::sync::Arc;

/// A problem in plain words: where it is, what went wrong, how to fix it.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub file: String,
    /// 1-based line, 0 when the place is unknown.
    pub line: usize,
    /// 1-based column, 0 when unknown.
    pub column: usize,
    pub message: String,
    pub suggestion: Option<String>,
    pub related: Vec<String>,
    pub source: Option<Arc<str>>,
}

pub type MgResult<T> = Result<T, Diagnostic>;

impl Diagnostic {
    pub fn new(message: impl Into<String>) -> Self {
        Diagnostic {
            file: String::new(),
            line: 0,
            column: 0,
            message: message.into(),
            suggestion: None,
            related: Vec::new(),
            source: None,
        }
    }

    pub fn at(mut self, file: &str, line: usize, column: usize) -> Self {
        self.file = file.to_string();
        self.line = line;
        self.column = column;
        self
    }

    pub fn with_source(mut self, source: Arc<str>) -> Self {
        self.source = Some(source);
        self
    }

    pub fn fix(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    pub fn related(mut self, line: impl Into<String>) -> Self {
        self.related.push(line.into());
        self
    }

    /// Adds a place to a diagnostic that does not have one yet.
    pub fn located(mut self, file: &str, line: usize, column: usize, source: &Arc<str>) -> Self {
        if self.line == 0 {
            self.file = file.to_string();
            self.line = line;
            self.column = column;
        }
        if self.source.is_none() {
            self.source = Some(source.clone());
        }
        self
    }

    /// Plain text with no colors, suitable for terminals, logs, and tests.
    pub fn render(&self) -> String {
        let mut out = String::new();
        if self.line > 0 {
            let file = if self.file.is_empty() {
                "your file"
            } else {
                &self.file
            };
            out.push_str(&format!(
                "Problem on line {} of {}: {}\n",
                self.line, file, self.message
            ));
            if let Some(snippet) = self.snippet() {
                out.push_str(&snippet);
            }
        } else {
            out.push_str(&format!("Problem: {}\n", self.message));
        }
        for line in &self.related {
            out.push_str(&format!("  {}\n", line));
        }
        if let Some(fix) = &self.suggestion {
            out.push_str(&format!("Fix: {}\n", fix));
        }
        out
    }

    /// The offending line with a marker under the place, drawn with plain characters.
    fn snippet(&self) -> Option<String> {
        let source = self.source.as_ref()?;
        let text = source
            .split('\n')
            .nth(self.line.checked_sub(1)?)?
            .trim_end_matches('\r');
        let number = self.line.to_string();
        let gutter = " ".repeat(number.len());
        let pad = " ".repeat(self.column.saturating_sub(1));
        Some(format!(
            "  {} | {}\n  {} | {}^\n",
            number, text, gutter, pad
        ))
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.render())
    }
}

impl std::error::Error for Diagnostic {}
