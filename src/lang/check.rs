//! Checks an agent file before it runs: structure, names, tool actions, and permissions.
//! Every problem found is reported, not just the first one.

use super::ast::*;
use crate::diagnostics::Diagnostic;
use crate::runtime::permissions::{BUILTIN_NAMES, Capabilities};
use crate::tools::{builtin_actions, closest_name};
use std::collections::HashSet;
use std::sync::Arc;

/// Structure, names and tool actions. Does not look at files on disk.
pub fn check_agents(agents: &[Arc<AgentDef>]) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    let mut seen_names: HashSet<&str> = HashSet::new();
    for agent in agents {
        if !seen_names.insert(agent.name.as_str()) {
            problems.push(
                at(
                    agent,
                    agent.span,
                    format!("there are two agents called {} in this file", agent.name),
                )
                .fix("give each agent its own name"),
            );
        }
        check_structure(agent, &mut problems);
        let mut walker = Walker {
            agent,
            caps: Capabilities::from_agent(agent),
            problems: &mut problems,
        };
        walker.run();
    }
    problems
}

fn at(agent: &AgentDef, span: Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(message)
        .at(&agent.source.name, span.line, span.col)
        .with_source(agent.source.text.clone())
}

fn check_structure(agent: &AgentDef, problems: &mut Vec<Diagnostic>) {
    if agent.goal.is_none() {
        problems.push(
            at(
                agent,
                agent.span,
                format!("agent {} has no goal", agent.name),
            )
            .fix(format!(
                "add a line under `agent {}` such as: goal \"what this agent is for\"",
                agent.name
            )),
        );
    }
    let mut accepted: HashSet<&str> = HashSet::new();
    for accept in &agent.accepts {
        if !accepted.insert(accept.message.as_str()) {
            problems.push(
                at(
                    agent,
                    accept.span,
                    format!("`accepts {}` appears twice", accept.message),
                )
                .fix("remove one of them"),
            );
        }
        if agent.handler(&accept.message).is_none() {
            problems.push(
                at(
                    agent,
                    accept.span,
                    format!(
                        "agent {} accepts `{}` but nothing says what to do with it",
                        agent.name, accept.message
                    ),
                )
                .fix(format!(
                    "add a section: on {}   (then indent the lines to run under it)",
                    accept.message
                )),
            );
        }
    }
    let mut handled: HashSet<&str> = HashSet::new();
    for handler in &agent.handlers {
        if !handled.insert(handler.message.as_str()) {
            problems.push(
                at(
                    agent,
                    handler.span,
                    format!("`on {}` appears twice", handler.message),
                )
                .fix("keep one of them"),
            );
        }
        if !accepted.contains(handler.message.as_str()) {
            let names: Vec<&str> = agent.accepts.iter().map(|a| a.message.as_str()).collect();
            let fix = match closest_name(&handler.message, names.iter().copied()) {
                Some(s) => format!(
                    "did you mean `on {}`? Otherwise add the line: accepts {}",
                    s, handler.message
                ),
                None => format!(
                    "add the line `accepts {}` under `agent {}`",
                    handler.message, agent.name
                ),
            };
            problems.push(
                at(
                    agent,
                    handler.span,
                    format!(
                        "`on {}` has no matching `accepts {}` line",
                        handler.message, handler.message
                    ),
                )
                .fix(fix),
            );
        }
    }
    let mut names: HashSet<&str> = HashSet::new();
    for n in agent
        .tools
        .iter()
        .map(|t| t.name.as_str())
        .chain(agent.links.iter().map(|l| l.name.as_str()))
        .chain(agent.remotes.iter().map(|r| r.name.as_str()))
    {
        if !names.insert(n) {
            problems.push(
                at(
                    agent,
                    agent.span,
                    format!(
                        "the name `{}` is declared more than once in agent {}",
                        n, agent.name
                    ),
                )
                .fix("use each name for only one tool, link, or remote"),
            );
        }
    }
}

struct Walker<'a> {
    agent: &'a AgentDef,
    caps: Capabilities,
    problems: &'a mut Vec<Diagnostic>,
}

impl Walker<'_> {
    fn run(&mut self) {
        let agent = self.agent;
        for handler in &agent.handlers {
            let mut vars: HashSet<String> = agent
                .accept(&handler.message)
                .map(|a| a.params.iter().cloned().collect())
                .unwrap_or_default();
            self.stmts(&handler.body, &mut vars);
        }
        if let Some(start) = &agent.start {
            let mut vars = HashSet::new();
            self.stmts(&start.body, &mut vars);
        }
    }

    fn stmts(&mut self, stmts: &[Stmt], vars: &mut HashSet<String>) {
        for stmt in stmts {
            match stmt {
                Stmt::Assign { name, value, span } => {
                    self.expr(value, vars);
                    if self.caps.allows(name) {
                        let d = at(
                            self.agent,
                            *span,
                            format!("`{}` is already the name of a tool, link, or remote", name),
                        )
                        .fix("choose a different name for this value");
                        self.problems.push(d);
                    }
                    vars.insert(name.clone());
                }
                Stmt::Expr { expr, .. } => self.expr(expr, vars),
                Stmt::Reply { value, .. } | Stmt::Fail { value, .. } => self.expr(value, vars),
                Stmt::If {
                    cond,
                    then,
                    otherwise,
                    ..
                } => {
                    self.expr(cond, vars);
                    self.stmts(then, vars);
                    self.stmts(otherwise, vars);
                }
                Stmt::For {
                    var, iter, body, ..
                } => {
                    self.expr(iter, vars);
                    vars.insert(var.clone());
                    self.stmts(body, vars);
                }
            }
        }
    }

    fn expr(&mut self, expr: &Expr, vars: &HashSet<String>) {
        match expr {
            Expr::Text(parts, span) => {
                for part in parts {
                    if let TextPart::Var(path) = part {
                        self.path(path, *span, vars);
                    }
                }
            }
            Expr::Number(..) | Expr::Bool(..) | Expr::Nothing(_) => {}
            Expr::List(items, _) => items.iter().for_each(|i| self.expr(i, vars)),
            Expr::Path(path, span) => self.path(path, *span, vars),
            Expr::Call(call) => self.call(call, vars),
            Expr::Think { prompt, .. } => self.expr(prompt, vars),
            Expr::Not(inner, _) => self.expr(inner, vars),
            Expr::And(a, b, _) | Expr::Or(a, b, _) => {
                self.expr(a, vars);
                self.expr(b, vars);
            }
            Expr::Compare { left, right, .. } => {
                self.expr(left, vars);
                self.expr(right, vars);
            }
        }
    }

    fn path(&mut self, path: &[String], span: Span, vars: &HashSet<String>) {
        let first = &path[0];
        if vars.contains(first) {
            return;
        }
        if path.len() >= 2 && (self.caps.allows(first) || BUILTIN_NAMES.contains(&first.as_str())) {
            let call = CallExpr {
                target: first.clone(),
                action: path[1..].join("."),
                args: vec![],
                within: None,
                span,
            };
            self.call(&call, vars);
            return;
        }
        if self.caps.allows(first) || BUILTIN_NAMES.contains(&first.as_str()) {
            return; // a bare tool name; the permission pass or the runtime explains it
        }
        let mut names: Vec<&str> = vars.iter().map(String::as_str).collect();
        let declared = self.caps.declared_names();
        names.extend(declared.iter().map(String::as_str));
        let d = at(
            self.agent,
            span,
            format!("I do not know what `{}` is here", first),
        );
        let d = match closest_name(first, names.iter().copied()) {
            Some(s) => d.fix(format!("did you mean `{}`?", s)),
            None => d.fix("give it a value first, for example: name = \"...\"  or list it after `accepts` at the top"),
        };
        self.problems.push(d);
    }

    fn call(&mut self, call: &CallExpr, vars: &HashSet<String>) {
        for (_, value) in &call.args {
            self.expr(value, vars);
        }
        // Built in tools have fixed actions, so mistakes can be caught before running.
        let declared_builtin = self
            .agent
            .tools
            .iter()
            .find(|t| t.name == call.target && !matches!(t.kind, ToolKind::Mcp { .. }));
        if declared_builtin.is_none() {
            return;
        }
        let Some(actions) = builtin_actions(&call.target) else {
            return;
        };
        let names: Vec<&str> = actions.iter().map(|a| a.name.as_str()).collect();
        let Some(info) = actions.iter().find(|a| a.name == call.action) else {
            let fix = match closest_name(&call.action, names.iter().copied()) {
                Some(s) => format!(
                    "did you mean `{}.{}`? (`{}` can do: {})",
                    call.target,
                    s,
                    call.target,
                    names.join(", ")
                ),
                None => format!("`{}` can do: {}", call.target, names.join(", ")),
            };
            self.problems.push(
                at(
                    self.agent,
                    call.span,
                    format!("`{}` has no action called `{}`", call.target, call.action),
                )
                .fix(fix),
            );
            return;
        };
        for param in info.params.iter().filter(|p| p.required) {
            if !call.args.iter().any(|(k, _)| k == &param.name) {
                self.problems.push(
                    at(
                        self.agent,
                        call.span,
                        format!(
                            "`{}.{}` needs a value for `{}`",
                            call.target, call.action, param.name
                        ),
                    )
                    .fix(format!(
                        "add it to the line, for example: {}.{} {}: \"...\"",
                        call.target, call.action, param.name
                    )),
                );
            }
        }
        let known: Vec<&str> = info.params.iter().map(|p| p.name.as_str()).collect();
        for (key, _) in &call.args {
            if !known.contains(&key.as_str()) {
                let fix = match closest_name(key, known.iter().copied()) {
                    Some(s) => format!("did you mean `{}`?", s),
                    None => format!(
                        "`{}.{}` takes: {}",
                        call.target,
                        call.action,
                        known.join(", ")
                    ),
                };
                self.problems.push(
                    at(
                        self.agent,
                        call.span,
                        format!("`{}.{}` does not take `{}`", call.target, call.action, key),
                    )
                    .fix(fix),
                );
            }
        }
    }
}

/// Finds uses of built in tools the agent never declared.
pub fn check_permissions(agents: &[Arc<AgentDef>]) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    for agent in agents {
        let caps = Capabilities::from_agent(agent);
        let mut targets: Vec<(String, Span)> = Vec::new();
        for handler in agent.handlers.iter().chain(agent.start.iter()) {
            collect_targets(&handler.body, &mut targets);
        }
        let mut reported: HashSet<(String, usize)> = HashSet::new();
        for (target, span) in targets {
            if BUILTIN_NAMES.contains(&target.as_str())
                && !caps.allows(&target)
                && reported.insert((target.clone(), span.line))
            {
                if let Err(d) = caps.check(&target) {
                    problems.push(d.located(
                        &agent.source.name,
                        span.line,
                        span.col,
                        &agent.source.text,
                    ));
                }
            }
        }
    }
    problems
}

/// Every call in a list of statements (`a.b` without values counts as a call too).
pub fn collect_calls(stmts: &[Stmt]) -> Vec<CallExpr> {
    fn in_expr(expr: &Expr, out: &mut Vec<CallExpr>) {
        match expr {
            Expr::Call(c) => {
                out.push(c.clone());
                c.args.iter().for_each(|(_, v)| in_expr(v, out));
            }
            Expr::Path(p, span) if p.len() >= 2 => out.push(CallExpr {
                target: p[0].clone(),
                action: p[1..].join("."),
                args: vec![],
                within: None,
                span: *span,
            }),
            Expr::List(items, _) => items.iter().for_each(|i| in_expr(i, out)),
            Expr::Think { prompt, .. } => in_expr(prompt, out),
            Expr::Not(e, _) => in_expr(e, out),
            Expr::And(a, b, _) | Expr::Or(a, b, _) => {
                in_expr(a, out);
                in_expr(b, out);
            }
            Expr::Compare { left, right, .. } => {
                in_expr(left, out);
                in_expr(right, out);
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for stmt in stmts {
        match stmt {
            Stmt::Assign { value, .. } | Stmt::Reply { value, .. } | Stmt::Fail { value, .. } => {
                in_expr(value, &mut out)
            }
            Stmt::Expr { expr, .. } => in_expr(expr, &mut out),
            Stmt::If {
                cond,
                then,
                otherwise,
                ..
            } => {
                in_expr(cond, &mut out);
                out.extend(collect_calls(then));
                out.extend(collect_calls(otherwise));
            }
            Stmt::For { iter, body, .. } => {
                in_expr(iter, &mut out);
                out.extend(collect_calls(body));
            }
        }
    }
    out
}

fn collect_targets(stmts: &[Stmt], out: &mut Vec<(String, Span)>) {
    fn in_expr(expr: &Expr, out: &mut Vec<(String, Span)>) {
        match expr {
            Expr::Call(c) => {
                out.push((c.target.clone(), c.span));
                c.args.iter().for_each(|(_, v)| in_expr(v, out));
            }
            Expr::Path(p, span) if p.len() >= 2 => out.push((p[0].clone(), *span)),
            Expr::Text(parts, span) => {
                for part in parts {
                    if let TextPart::Var(p) = part {
                        if p.len() >= 2 {
                            out.push((p[0].clone(), *span));
                        }
                    }
                }
            }
            Expr::List(items, _) => items.iter().for_each(|i| in_expr(i, out)),
            Expr::Think { prompt, .. } => in_expr(prompt, out),
            Expr::Not(e, _) => in_expr(e, out),
            Expr::And(a, b, _) | Expr::Or(a, b, _) => {
                in_expr(a, out);
                in_expr(b, out);
            }
            Expr::Compare { left, right, .. } => {
                in_expr(left, out);
                in_expr(right, out);
            }
            _ => {}
        }
    }
    for stmt in stmts {
        match stmt {
            Stmt::Assign { value, .. } | Stmt::Reply { value, .. } | Stmt::Fail { value, .. } => {
                in_expr(value, out)
            }
            Stmt::Expr { expr, .. } => in_expr(expr, out),
            Stmt::If {
                cond,
                then,
                otherwise,
                ..
            } => {
                in_expr(cond, out);
                collect_targets(then, out);
                collect_targets(otherwise, out);
            }
            Stmt::For { iter, body, .. } => {
                in_expr(iter, out);
                collect_targets(body, out);
            }
        }
    }
}

/// Everything `metagente check` looks at, in the order a person would fix it.
pub fn check_all(agents: &[Arc<AgentDef>]) -> Vec<Diagnostic> {
    let mut problems = check_permissions(agents);
    problems.extend(check_agents(agents));
    problems.sort_by_key(|d| (d.line, d.column));
    problems
}
