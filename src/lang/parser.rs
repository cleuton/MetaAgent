//! Recursive descent parser. Every error says what is wrong and how to fix it.

use super::ast::*;
use super::lexer::{Line, Tok, Token, lex};
use crate::diagnostics::Diagnostic;
use std::path::PathBuf;
use std::sync::Arc;

const AGENT_WORDS: &[&str] = &["goal", "tool", "link", "remote", "accepts", "on"];
const BUILTIN_TOOLS: &[&str] = &["file", "http", "env", "state", "clock"];

/// Parses a whole `.ag` file into its agents.
pub fn parse_file(
    name: &str,
    path: Option<PathBuf>,
    text: &str,
) -> Result<Vec<AgentDef>, Diagnostic> {
    let source = Arc::new(SourceFile {
        name: name.to_string(),
        path,
        text: Arc::from(text),
    });
    let lines = lex(name, &source.text)?;
    let mut parser = Parser {
        lines,
        pos: 0,
        source: source.clone(),
    };
    parser.file()
}

struct Parser {
    lines: Vec<Line>,
    pos: usize,
    source: Arc<SourceFile>,
}

/// Cursor over the tokens of one line.
struct Cursor<'a> {
    tokens: &'a [Token],
    i: usize,
    line: usize,
    /// Column just after the last token, for "missing something" errors.
    end_col: usize,
}

fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i];
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur.push((prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost));
        }
        prev = cur;
    }
    prev[b.len()]
}

fn closest<'a>(word: &str, options: &[&'a str]) -> Option<&'a str> {
    options
        .iter()
        .map(|o| (distance(word, o), *o))
        .filter(|(d, _)| *d <= 2 && *d < word.chars().count())
        .min_by_key(|(d, _)| *d)
        .map(|(_, o)| o)
}

impl Parser {
    fn err(
        &self,
        line: usize,
        col: usize,
        msg: impl Into<String>,
        fix: impl Into<String>,
    ) -> Diagnostic {
        Diagnostic::new(msg)
            .at(&self.source.name, line, col)
            .with_source(self.source.text.clone())
            .fix(fix)
    }

    fn file(&mut self) -> Result<Vec<AgentDef>, Diagnostic> {
        let mut agents = Vec::new();
        if self.lines.is_empty() {
            return Err(Diagnostic::new("this file has no agent in it")
                .at(&self.source.name, 0, 0)
                .fix("start with a line like: agent Helper"));
        }
        while self.pos < self.lines.len() {
            let line = self.lines[self.pos].clone();
            if line.indent != 0
                || !matches!(line.tokens.first(), Some(Token { tok: Tok::Word(w), .. }) if w == "agent")
            {
                let col = line.tokens.first().map(|t| t.col).unwrap_or(1);
                return Err(self.err(
                    line.number,
                    col.max(line.indent * 2 + 1),
                    "every agent must begin with the word `agent` at the start of a line",
                    "write: agent Name   (then indent the lines that belong to it by 2 spaces)",
                ));
            }
            agents.push(self.agent()?);
        }
        Ok(agents)
    }

    fn agent(&mut self) -> Result<AgentDef, Diagnostic> {
        let header = self.lines[self.pos].clone();
        self.pos += 1;
        let mut c = self.cursor(&header, 1);
        let name = self.name(&mut c, "the agent needs a name", "write: agent Weather")?;
        self.finish(&c)?;
        let span = Span {
            line: header.number,
            col: header.tokens[0].col,
        };
        let mut agent = AgentDef {
            name,
            span,
            goal: None,
            tools: vec![],
            links: vec![],
            remotes: vec![],
            accepts: vec![],
            handlers: vec![],
            start: None,
            source: self.source.clone(),
        };
        while self.pos < self.lines.len() && self.lines[self.pos].indent > 0 {
            let line = self.lines[self.pos].clone();
            if line.indent != 1 {
                return Err(self.err(
                    line.number,
                    1,
                    "this line is indented too far for the agent's own lines",
                    "indent it by 2 spaces under `agent`, or move it under an `on` handler",
                ));
            }
            self.pos += 1;
            self.declaration(&mut agent, &line)?;
        }
        Ok(agent)
    }

    fn declaration(&mut self, agent: &mut AgentDef, line: &Line) -> Result<(), Diagnostic> {
        let first = &line.tokens[0];
        let word = match &first.tok {
            Tok::Word(w) => w.clone(),
            _ => {
                return Err(self.err(
                    line.number,
                    first.col,
                    format!(
                        "I did not expect {} at the start of this line",
                        first.describe()
                    ),
                    "start the line with one of: goal, tool, link, remote, accepts, on",
                ));
            }
        };
        let span = Span {
            line: line.number,
            col: first.col,
        };
        let mut c = self.cursor(line, 1);
        match word.as_str() {
            "goal" => {
                let parts = self.text(
                    &mut c,
                    "the goal needs a text",
                    "write: goal \"Answer questions\"",
                )?;
                let mut goal = String::new();
                for p in parts {
                    match p {
                        TextPart::Lit(s) => goal.push_str(&s),
                        TextPart::Var(_) => {
                            return Err(self.err(
                                line.number,
                                first.col,
                                "the goal is fixed text and cannot contain {names}",
                                "remove the braces",
                            ));
                        }
                    }
                }
                self.finish(&c)?;
                agent.goal = Some((goal, span));
            }
            "tool" => agent.tools.push(self.tool_decl(&mut c, span)?),
            "link" => {
                let name = self.name(
                    &mut c,
                    "`link` needs the name of the agent to call",
                    "write: link Weather",
                )?;
                let path = if c.peek_word("from") {
                    c.i += 1;
                    let p = self.text(
                        &mut c,
                        "`from` needs a file path in quotes",
                        "write: link Weather from \"weather.ag\"",
                    )?;
                    Some(join_lit(p))
                } else {
                    None
                };
                self.finish(&c)?;
                agent.links.push(LinkDecl { name, path, span });
            }
            "remote" => {
                let name = self.name(
                    &mut c,
                    "`remote` needs a name for the other agent",
                    "write: remote Bob at \"https://...\"",
                )?;
                if !c.peek_word("at") {
                    return Err(self.missing(
                        &c,
                        "`remote` needs `at` and the address",
                        "write: remote Bob at \"https://host:8080\"",
                    ));
                }
                c.i += 1;
                let url = join_lit(self.text(
                    &mut c,
                    "the address must be in quotes",
                    "write: remote Bob at \"https://host:8080\"",
                )?);
                self.finish(&c)?;
                agent.remotes.push(RemoteDecl { name, url, span });
            }
            "accepts" => {
                let message = self.name(
                    &mut c,
                    "`accepts` needs the name of a message",
                    "write: accepts ask city",
                )?;
                let mut params = Vec::new();
                while let Some(Token {
                    tok: Tok::Word(w), ..
                }) = c.tokens.get(c.i)
                {
                    params.push(w.clone());
                    c.i += 1;
                }
                self.finish(&c)?;
                agent.accepts.push(Accept {
                    message,
                    params,
                    description: line.comment.clone().filter(|s| !s.is_empty()),
                    span,
                });
            }
            "on" => {
                let message =
                    self.name(&mut c, "`on` needs the name of a message", "write: on ask")?;
                self.finish(&c)?;
                let body = self.block(line.indent)?;
                if body.is_empty() {
                    return Err(self.err(
                        line.number,
                        first.col,
                        format!("`on {}` has nothing under it", message),
                        "indent at least one line under it, for example: reply \"Hello\"",
                    ));
                }
                let handler = Handler {
                    message: message.clone(),
                    body,
                    span,
                };
                if message == "start" {
                    agent.start = Some(handler);
                } else {
                    agent.handlers.push(handler);
                }
            }
            "agent" => {
                return Err(self.err(
                    line.number,
                    first.col,
                    "an agent cannot be defined inside another agent",
                    "move it to the start of a line, with no indentation",
                ));
            }
            other => {
                let hint = closest(other, AGENT_WORDS)
                    .map(|s| format!("did you mean `{}`?", s))
                    .unwrap_or_else(|| {
                        "use one of: goal, tool, link, remote, accepts, on".to_string()
                    });
                return Err(self.err(
                    line.number,
                    first.col,
                    format!("I do not know the word `{}` here", other),
                    hint,
                ));
            }
        }
        Ok(())
    }

    fn tool_decl(&mut self, c: &mut Cursor, span: Span) -> Result<ToolDecl, Diagnostic> {
        let name = self.name(c, "`tool` needs the name of a tool", "write: tool file")?;
        let bad_end = |p: &Parser, c: &Cursor| p.finish(c);
        match name.as_str() {
            "file" => {
                let scope = if matches!(
                    c.tokens.get(c.i),
                    Some(Token {
                        tok: Tok::Str(_),
                        ..
                    })
                ) {
                    Some(join_lit(self.text(c, "", "")?))
                } else {
                    None
                };
                bad_end(self, c)?;
                Ok(ToolDecl {
                    name,
                    kind: ToolKind::File { scope },
                    span,
                })
            }
            "http" | "state" | "clock" => {
                bad_end(self, c)?;
                let kind = match name.as_str() {
                    "http" => ToolKind::Http,
                    "state" => ToolKind::State,
                    _ => ToolKind::Clock,
                };
                Ok(ToolDecl { name, kind, span })
            }
            "env" => {
                let mut names = Vec::new();
                while matches!(
                    c.tokens.get(c.i),
                    Some(Token {
                        tok: Tok::Str(_),
                        ..
                    })
                ) {
                    names.push(join_lit(self.text(c, "", "")?));
                }
                if names.is_empty() {
                    return Err(self.err(
                        span.line,
                        span.col,
                        "`tool env` needs the names of the variables this agent may read",
                        "write the names in quotes, for example: tool env \"HOME\" \"USER\"",
                    ));
                }
                bad_end(self, c)?;
                Ok(ToolDecl {
                    name,
                    kind: ToolKind::Env { names },
                    span,
                })
            }
            _ => {
                if c.peek_word("from") {
                    c.i += 1;
                    if !c.peek_word("mcp") {
                        return Err(self.missing(
                            c,
                            "after `from` I expected `mcp`",
                            "write: tool weather from mcp \"command or address\"",
                        ));
                    }
                    c.i += 1;
                    let command = join_lit(self.text(
                        c,
                        "the MCP command or address must be in quotes",
                        "write: tool weather from mcp \"npx -y weather-mcp\"",
                    )?);
                    bad_end(self, c)?;
                    Ok(ToolDecl {
                        name,
                        kind: ToolKind::Mcp { command },
                        span,
                    })
                } else {
                    let hint = closest(&name, BUILTIN_TOOLS)
                        .map(|s| format!("did you mean `tool {}`?", s))
                        .unwrap_or_else(|| {
                            format!(
                                "built in tools are: {}. For an external MCP tool write: tool {} from mcp \"command\"",
                                BUILTIN_TOOLS.join(", "),
                                name
                            )
                        });
                    Err(self.err(
                        span.line,
                        c.tokens[1].col,
                        format!("I do not know a built in tool called `{}`", name),
                        hint,
                    ))
                }
            }
        }
    }

    /// Parses the statements indented one level deeper than `parent_indent`.
    fn block(&mut self, parent_indent: usize) -> Result<Vec<Stmt>, Diagnostic> {
        let mut stmts = Vec::new();
        while self.pos < self.lines.len() && self.lines[self.pos].indent > parent_indent {
            let line = self.lines[self.pos].clone();
            if line.indent != parent_indent + 1 {
                return Err(self.err(
                    line.number,
                    1,
                    "this line is indented more than the line above it allows",
                    "indent by exactly one level (2 spaces) more than the line it belongs to",
                ));
            }
            self.pos += 1;
            stmts.push(self.statement(&line)?);
        }
        Ok(stmts)
    }

    fn statement(&mut self, line: &Line) -> Result<Stmt, Diagnostic> {
        let first = &line.tokens[0];
        let span = Span {
            line: line.number,
            col: first.col,
        };
        let mut c = self.cursor(line, 0);
        let word = match &first.tok {
            Tok::Word(w) => w.clone(),
            _ => {
                let expr = self.expr(&mut c)?;
                self.finish(&c)?;
                return Ok(Stmt::Expr { expr, span });
            }
        };
        match word.as_str() {
            "reply" => {
                c.i += 1;
                let value = self.expr(&mut c)?;
                self.finish(&c)?;
                Ok(Stmt::Reply { value, span })
            }
            "fail" => {
                c.i += 1;
                let value = self.expr(&mut c)?;
                self.finish(&c)?;
                Ok(Stmt::Fail { value, span })
            }
            "if" => {
                c.i += 1;
                let cond = self.expr(&mut c)?;
                self.finish(&c)?;
                let then = self.block(line.indent)?;
                if then.is_empty() {
                    return Err(self.err(
                        line.number,
                        first.col,
                        "this `if` has nothing under it",
                        "indent the lines to run when the condition is true",
                    ));
                }
                let mut otherwise = Vec::new();
                if self.pos < self.lines.len() {
                    let next = self.lines[self.pos].clone();
                    if next.indent == line.indent
                        && matches!(next.tokens.first(), Some(Token { tok: Tok::Word(w), .. }) if w == "otherwise")
                    {
                        self.pos += 1;
                        let oc = self.cursor(&next, 1);
                        self.finish(&oc)?;
                        otherwise = self.block(line.indent)?;
                    }
                }
                Ok(Stmt::If {
                    cond,
                    then,
                    otherwise,
                    span,
                })
            }
            "otherwise" => Err(self.err(
                line.number,
                first.col,
                "`otherwise` must come right after an `if` block, at the same indentation",
                "move it directly under the `if` it belongs to",
            )),
            "for" => {
                c.i += 1;
                let var = self.name(
                    &mut c,
                    "`for` needs a name for each item",
                    "write: for city in cities",
                )?;
                if !c.peek_word("in") {
                    return Err(self.missing(
                        &c,
                        "`for` needs `in` and a list",
                        "write: for city in cities",
                    ));
                }
                c.i += 1;
                let iter = self.expr(&mut c)?;
                self.finish(&c)?;
                let body = self.block(line.indent)?;
                if body.is_empty() {
                    return Err(self.err(
                        line.number,
                        first.col,
                        "this `for` has nothing under it",
                        "indent the lines to repeat",
                    ));
                }
                Ok(Stmt::For {
                    var,
                    iter,
                    body,
                    span,
                })
            }
            _ => {
                // `name = expression`
                if matches!(
                    c.tokens.get(1),
                    Some(Token {
                        tok: Tok::Sym('='),
                        ..
                    })
                ) {
                    c.i = 2;
                    let value = self.expr(&mut c)?;
                    self.finish(&c)?;
                    return Ok(Stmt::Assign {
                        name: word,
                        value,
                        span,
                    });
                }
                // `target.action ...` or `think ...`
                if word == "think"
                    || matches!(
                        c.tokens.get(1),
                        Some(Token {
                            tok: Tok::Sym('.'),
                            ..
                        })
                    )
                {
                    let expr = self.expr(&mut c)?;
                    self.finish(&c)?;
                    return Ok(Stmt::Expr { expr, span });
                }
                let hint = closest(&word, &["reply", "fail", "if", "otherwise", "for", "think"])
                    .map(|s| format!("did you mean `{}`?", s))
                    .unwrap_or_else(|| {
                        "a line can be: name = value, target.action key: value, reply, think, if, for, or fail".to_string()
                    });
                Err(self.err(
                    line.number,
                    first.col,
                    format!("I do not understand the line starting with `{}`", word),
                    hint,
                ))
            }
        }
    }

    // ---------- expressions ----------

    fn expr(&mut self, c: &mut Cursor) -> Result<Expr, Diagnostic> {
        self.or_expr(c)
    }

    fn or_expr(&mut self, c: &mut Cursor) -> Result<Expr, Diagnostic> {
        let mut left = self.and_expr(c)?;
        while c.peek_word("or") {
            let span = left.span();
            c.i += 1;
            let right = self.and_expr(c)?;
            left = Expr::Or(Box::new(left), Box::new(right), span);
        }
        Ok(left)
    }

    fn and_expr(&mut self, c: &mut Cursor) -> Result<Expr, Diagnostic> {
        let mut left = self.not_expr(c)?;
        while c.peek_word("and") {
            let span = left.span();
            c.i += 1;
            let right = self.not_expr(c)?;
            left = Expr::And(Box::new(left), Box::new(right), span);
        }
        Ok(left)
    }

    fn not_expr(&mut self, c: &mut Cursor) -> Result<Expr, Diagnostic> {
        if c.peek_word("not") {
            let span = c.span();
            c.i += 1;
            let inner = self.not_expr(c)?;
            return Ok(Expr::Not(Box::new(inner), span));
        }
        self.compare(c)
    }

    fn compare(&mut self, c: &mut Cursor) -> Result<Expr, Diagnostic> {
        let left = self.primary(c, true)?;
        let span = left.span();
        if c.peek_word("is") {
            c.i += 1;
            let op = if c.peek_word("not") {
                c.i += 1;
                CompareOp::IsNot
            } else if c.peek_word("more") {
                c.i += 1;
                self.expect_word(c, "than", "write: is more than")?;
                CompareOp::MoreThan
            } else if c.peek_word("less") {
                c.i += 1;
                self.expect_word(c, "than", "write: is less than")?;
                CompareOp::LessThan
            } else {
                CompareOp::Is
            };
            let right = self.primary(c, true)?;
            return Ok(Expr::Compare {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            });
        }
        if c.peek_word("contains") {
            c.i += 1;
            let right = self.primary(c, true)?;
            return Ok(Expr::Compare {
                op: CompareOp::Contains,
                left: Box::new(left),
                right: Box::new(right),
                span,
            });
        }
        Ok(left)
    }

    fn expect_word(&self, c: &mut Cursor, word: &str, fix: &str) -> Result<(), Diagnostic> {
        if c.peek_word(word) {
            c.i += 1;
            Ok(())
        } else {
            Err(self.missing(c, &format!("I expected `{}` here", word), fix))
        }
    }

    fn primary(&mut self, c: &mut Cursor, allow_call: bool) -> Result<Expr, Diagnostic> {
        let Some(token) = c.tokens.get(c.i).cloned() else {
            return Err(self.missing(
                c,
                "the line ends but a value is missing",
                "add a value: text in quotes, a number, or a name",
            ));
        };
        let span = Span {
            line: c.line,
            col: token.col,
        };
        match token.tok {
            Tok::Str(parts) => {
                c.i += 1;
                Ok(Expr::Text(parts, span))
            }
            Tok::Number(n) => {
                c.i += 1;
                Ok(Expr::Number(n, span))
            }
            Tok::Sym('[') => {
                c.i += 1;
                let mut items = Vec::new();
                loop {
                    if matches!(
                        c.tokens.get(c.i),
                        Some(Token {
                            tok: Tok::Sym(']'),
                            ..
                        })
                    ) {
                        c.i += 1;
                        break;
                    }
                    items.push(self.primary(c, false)?);
                    match c.tokens.get(c.i) {
                        Some(Token {
                            tok: Tok::Sym(','), ..
                        }) => c.i += 1,
                        Some(Token {
                            tok: Tok::Sym(']'), ..
                        }) => {}
                        _ => {
                            return Err(self.missing(
                                c,
                                "this list is not closed",
                                "close it with ] and separate items with commas",
                            ));
                        }
                    }
                }
                Ok(Expr::List(items, span))
            }
            Tok::Word(w) => {
                match w.as_str() {
                    "yes" => {
                        c.i += 1;
                        return Ok(Expr::Bool(true, span));
                    }
                    "no" => {
                        c.i += 1;
                        return Ok(Expr::Bool(false, span));
                    }
                    "nothing" => {
                        c.i += 1;
                        return Ok(Expr::Nothing(span));
                    }
                    "think" if allow_call => {
                        c.i += 1;
                        let prompt = self.primary(c, false)?;
                        return Ok(Expr::Think {
                            prompt: Box::new(prompt),
                            span,
                        });
                    }
                    _ => {}
                }
                c.i += 1;
                let mut segments = vec![w];
                while matches!(
                    c.tokens.get(c.i),
                    Some(Token {
                        tok: Tok::Sym('.'),
                        ..
                    })
                ) {
                    match c.tokens.get(c.i + 1) {
                        Some(Token {
                            tok: Tok::Word(next),
                            ..
                        }) => {
                            segments.push(next.clone());
                            c.i += 2;
                        }
                        _ => {
                            return Err(self.missing(
                                c,
                                "a name is missing after the dot",
                                "write it like forecast.summary",
                            ));
                        }
                    }
                }
                if !allow_call {
                    return Ok(Expr::Path(segments, span));
                }
                // Arguments turn a path into a call.
                let has_args = self.at_pair(c);
                let has_within = c.peek_word("within");
                if has_args || has_within {
                    if segments.len() < 2 {
                        return Err(self.err(
                            c.line,
                            span.col,
                            "a call has the form target.action, followed by key: value pairs",
                            "for example: weather.forecast city: \"Lisbon\"",
                        ));
                    }
                    let mut args = Vec::new();
                    while self.at_pair(c) {
                        let Some(Token {
                            tok: Tok::Word(key),
                            ..
                        }) = c.tokens.get(c.i).cloned()
                        else {
                            break;
                        };
                        c.i += 2;
                        let value = self.primary(c, false)?;
                        args.push((key, value));
                    }
                    let mut within = None;
                    if c.peek_word("within") {
                        c.i += 1;
                        let Some(Token {
                            tok: Tok::Number(n),
                            ..
                        }) = c.tokens.get(c.i).cloned()
                        else {
                            return Err(self.missing(
                                c,
                                "`within` needs a number of seconds",
                                "write: within 30 seconds",
                            ));
                        };
                        c.i += 1;
                        if !(c.peek_word("seconds") || c.peek_word("second")) {
                            return Err(self.missing(
                                c,
                                "after the number I expected `seconds`",
                                "write: within 30 seconds",
                            ));
                        }
                        c.i += 1;
                        within = Some(n);
                    }
                    return Ok(Expr::Call(CallExpr {
                        target: segments[0].clone(),
                        action: segments[1..].join("."),
                        args,
                        within,
                        span,
                    }));
                }
                Ok(Expr::Path(segments, span))
            }
            Tok::Sym(ch) => Err(self.err(
                c.line,
                token.col,
                format!("I did not expect `{}` here", ch),
                "a value can be text in quotes, a number, yes, no, a list, or a name",
            )),
        }
    }

    fn at_pair(&self, c: &Cursor) -> bool {
        matches!(
            (c.tokens.get(c.i), c.tokens.get(c.i + 1)),
            (
                Some(Token {
                    tok: Tok::Word(_),
                    ..
                }),
                Some(Token {
                    tok: Tok::Sym(':'),
                    ..
                })
            )
        )
    }

    // ---------- small helpers ----------

    fn cursor<'a>(&self, line: &'a Line, start: usize) -> Cursor<'a> {
        let end_col = line.tokens.last().map(|t| t.col + 1).unwrap_or(1);
        Cursor {
            tokens: &line.tokens,
            i: start,
            line: line.number,
            end_col,
        }
    }

    fn missing(&self, c: &Cursor, msg: &str, fix: &str) -> Diagnostic {
        let col = c.tokens.get(c.i).map(|t| t.col).unwrap_or(c.end_col);
        self.err(c.line, col, msg, fix)
    }

    fn name(&self, c: &mut Cursor, msg: &str, fix: &str) -> Result<String, Diagnostic> {
        match c.tokens.get(c.i) {
            Some(Token {
                tok: Tok::Word(w), ..
            }) => {
                c.i += 1;
                Ok(w.clone())
            }
            _ => Err(self.missing(c, msg, fix)),
        }
    }

    fn text(&self, c: &mut Cursor, msg: &str, fix: &str) -> Result<Vec<TextPart>, Diagnostic> {
        match c.tokens.get(c.i) {
            Some(Token {
                tok: Tok::Str(parts),
                ..
            }) => {
                c.i += 1;
                Ok(parts.clone())
            }
            _ => Err(self.missing(c, msg, fix)),
        }
    }

    /// The line must be fully consumed.
    fn finish(&self, c: &Cursor) -> Result<(), Diagnostic> {
        if let Some(extra) = c.tokens.get(c.i) {
            return Err(self.err(
                c.line,
                extra.col,
                format!(
                    "I did not expect {} here; the line should have ended",
                    extra.describe()
                ),
                "remove it, or check for a missing quote or colon before it",
            ));
        }
        Ok(())
    }
}

impl Cursor<'_> {
    fn peek_word(&self, word: &str) -> bool {
        matches!(self.tokens.get(self.i), Some(Token { tok: Tok::Word(w), .. }) if w == word)
    }
    fn span(&self) -> Span {
        Span {
            line: self.line,
            col: self.tokens.get(self.i).map(|t| t.col).unwrap_or(1),
        }
    }
}

fn join_lit(parts: Vec<TextPart>) -> String {
    parts
        .into_iter()
        .map(|p| match p {
            TextPart::Lit(s) => s,
            TextPart::Var(v) => format!("{{{}}}", v.join(".")),
        })
        .collect()
}
