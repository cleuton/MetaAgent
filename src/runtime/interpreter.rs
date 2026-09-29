//! The tree walking evaluator. It runs one handler of one agent.

use super::Runtime;
use super::permissions::Capabilities;
use super::task::{CallContext, Task, TaskState, new_task_id};
use super::value::Value;
use crate::diagnostics::{Diagnostic, MgResult};
use crate::lang::{AgentDef, CallExpr, CompareOp, Expr, Span, Stmt, TextPart};
use crate::tools::{Args, Registry, build_registry, closest_name};
use futures::future::BoxFuture;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// A running agent: its definition, what it may use, and the shared runtime.
pub struct Agent {
    pub def: Arc<AgentDef>,
    pub rt: Arc<Runtime>,
    pub caps: Capabilities,
    pub tools: Registry,
}

#[derive(Default)]
struct Env {
    vars: HashMap<String, Value>,
}

enum Flow {
    Next,
    Reply(Value),
}

impl Agent {
    pub fn new(rt: Arc<Runtime>, def: Arc<AgentDef>) -> MgResult<Agent> {
        let tools = build_registry(&rt, &def)?;
        let caps = Capabilities::from_agent(&def);
        Ok(Agent {
            def,
            rt,
            caps,
            tools,
        })
    }

    /// Runs the handler for `message`, after checking it against the agent's interface.
    pub async fn handle(&self, message: &str, args: Args, ctx: &CallContext) -> MgResult<Value> {
        crate::link::interface::check(&self.def, message, &args)?;
        let Some(handler) = self.def.handler(message) else {
            return Err(self
                .diag(
                    self.def.span,
                    format!(
                        "agent {} accepts `{}` but has no `on {}` section",
                        self.def.name, message, message
                    ),
                )
                .fix(format!(
                    "add an `on {}` section with the lines to run",
                    message
                )));
        };
        let ctx = ctx.entering(&self.def.name);
        let mut env = Env::default();
        for (key, value) in args {
            env.vars.insert(key, value);
        }
        match self.exec_block(&handler.body, &mut env, &ctx).await? {
            Flow::Reply(value) => Ok(value),
            Flow::Next => Ok(Value::Nothing),
        }
    }

    /// Runs the optional `on start` section.
    pub async fn start(&self, ctx: &CallContext) -> MgResult<Option<Value>> {
        let Some(handler) = &self.def.start else {
            return Ok(None);
        };
        let ctx = ctx.entering(&self.def.name);
        let mut env = Env::default();
        match self.exec_block(&handler.body, &mut env, &ctx).await? {
            Flow::Reply(value) => Ok(Some(value)),
            Flow::Next => Ok(None),
        }
    }

    pub(crate) fn diag(&self, span: Span, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(message)
            .at(&self.def.source.name, span.line, span.col)
            .with_source(self.def.source.text.clone())
    }

    fn exec_block<'a>(
        &'a self,
        stmts: &'a [Stmt],
        env: &'a mut Env,
        ctx: &'a CallContext,
    ) -> BoxFuture<'a, MgResult<Flow>> {
        Box::pin(async move {
            for stmt in stmts {
                match stmt {
                    Stmt::Assign { name, value, .. } => {
                        let v = self.eval(value, env, ctx).await?;
                        env.vars.insert(name.clone(), v);
                    }
                    Stmt::Expr { expr, .. } => {
                        self.eval(expr, env, ctx).await?;
                    }
                    Stmt::Reply { value, .. } => {
                        return Ok(Flow::Reply(self.eval(value, env, ctx).await?));
                    }
                    Stmt::If {
                        cond,
                        then,
                        otherwise,
                        ..
                    } => {
                        let branch = if self.eval(cond, env, ctx).await?.is_truthy() {
                            then
                        } else {
                            otherwise
                        };
                        if let Flow::Reply(v) = self.exec_block(branch, env, ctx).await? {
                            return Ok(Flow::Reply(v));
                        }
                    }
                    Stmt::For {
                        var,
                        iter,
                        body,
                        span,
                    } => {
                        let list = self.eval(iter, env, ctx).await?;
                        let Value::List(items) = list else {
                            return Err(self
                                .diag(*span, format!("`for` needs a list, but it got {}", describe(&list)))
                                .fix("give it a list such as [1, 2, 3], or a value that holds a list"));
                        };
                        for item in items {
                            env.vars.insert(var.clone(), item);
                            if let Flow::Reply(v) = self.exec_block(body, env, ctx).await? {
                                return Ok(Flow::Reply(v));
                            }
                        }
                    }
                    Stmt::Fail { value, span } => {
                        let message = self.eval(value, env, ctx).await?.to_display();
                        return Err(self.diag(*span, message));
                    }
                }
            }
            Ok(Flow::Next)
        })
    }

    fn eval<'a>(
        &'a self,
        expr: &'a Expr,
        env: &'a Env,
        ctx: &'a CallContext,
    ) -> BoxFuture<'a, MgResult<Value>> {
        Box::pin(async move {
            match expr {
                Expr::Text(parts, span) => {
                    let mut out = String::new();
                    for part in parts {
                        match part {
                            TextPart::Lit(s) => out.push_str(s),
                            TextPart::Var(path) => out.push_str(
                                &self.eval_path(path, *span, env, ctx).await?.to_display(),
                            ),
                        }
                    }
                    Ok(Value::Text(out))
                }
                Expr::Number(n, _) => Ok(Value::Number(*n)),
                Expr::Bool(b, _) => Ok(Value::Bool(*b)),
                Expr::Nothing(_) => Ok(Value::Nothing),
                Expr::List(items, _) => {
                    let mut out = Vec::new();
                    for item in items {
                        out.push(self.eval(item, env, ctx).await?);
                    }
                    Ok(Value::List(out))
                }
                Expr::Path(path, span) => self.eval_path(path, *span, env, ctx).await,
                Expr::Call(call) => self.eval_call(call, env, ctx).await,
                Expr::Think { prompt, span } => {
                    let text = self.eval(prompt, env, ctx).await?.to_display();
                    super::think::think(self, text, ctx, *span).await
                }
                Expr::Not(inner, _) => {
                    Ok(Value::Bool(!self.eval(inner, env, ctx).await?.is_truthy()))
                }
                Expr::And(a, b, _) => {
                    let left = self.eval(a, env, ctx).await?;
                    if !left.is_truthy() {
                        return Ok(Value::Bool(false));
                    }
                    Ok(Value::Bool(self.eval(b, env, ctx).await?.is_truthy()))
                }
                Expr::Or(a, b, _) => {
                    let left = self.eval(a, env, ctx).await?;
                    if left.is_truthy() {
                        return Ok(Value::Bool(true));
                    }
                    Ok(Value::Bool(self.eval(b, env, ctx).await?.is_truthy()))
                }
                Expr::Compare {
                    op,
                    left,
                    right,
                    span,
                } => {
                    let l = self.eval(left, env, ctx).await?;
                    let r = self.eval(right, env, ctx).await?;
                    self.compare(*op, &l, &r, *span)
                }
            }
        })
    }

    async fn eval_path(
        &self,
        path: &[String],
        span: Span,
        env: &Env,
        ctx: &CallContext,
    ) -> MgResult<Value> {
        let first = &path[0];
        if let Some(value) = env.vars.get(first) {
            let mut current = value.clone();
            for (i, seg) in path.iter().enumerate().skip(1) {
                current = match &current {
                    Value::Record(map) => match map.get(seg) {
                        Some(v) => v.clone(),
                        None => {
                            let fields: Vec<&str> = map.keys().map(String::as_str).collect();
                            let mut d = self.diag(
                                span,
                                format!("`{}` has no field called `{}`", path[..i].join("."), seg),
                            );
                            d = match closest_name(seg, fields.iter().copied()) {
                                Some(s) => {
                                    d.fix(format!("did you mean `{}.{}`?", path[..i].join("."), s))
                                }
                                None => d.fix(format!("it has: {}", fields.join(", "))),
                            };
                            return Err(d);
                        }
                    },
                    other => {
                        return Err(self
                            .diag(
                                span,
                                format!(
                                    "`{}` is {}, so it has no field `{}`",
                                    path[..i].join("."),
                                    describe(other),
                                    seg
                                ),
                            )
                            .fix("only records (like results from tools) have fields"));
                    }
                };
            }
            return Ok(current);
        }
        // A declared tool, link, or remote used without arguments: `clock.now`.
        if self.caps.allows(first) || super::permissions::BUILTIN_NAMES.contains(&first.as_str()) {
            if path.len() >= 2 {
                let call = CallExpr {
                    target: first.clone(),
                    action: path[1..].join("."),
                    args: vec![],
                    within: None,
                    span,
                };
                return self.eval_call(&call, &Env::default(), ctx).await;
            }
            if path.len() == 1 {
                self.caps.check(first)?;
                return Err(self
                    .diag(span, format!("`{}` is a tool, not a value", first))
                    .fix(format!(
                        "call one of its actions, for example: {}.action",
                        first
                    )));
            }
        }
        let mut names: Vec<&str> = env.vars.keys().map(String::as_str).collect();
        let declared = self.caps.declared_names();
        names.extend(declared.iter().map(String::as_str));
        let mut d = self.diag(span, format!("I do not know what `{}` is here", first));
        d = match closest_name(first, names.iter().copied()) {
            Some(s) => d.fix(format!("did you mean `{}`?", s)),
            None => d.fix("give it a value first, for example: name = \"...\"  or list it after `accepts` at the top"),
        };
        Err(d)
    }

    async fn eval_call(&self, call: &CallExpr, env: &Env, ctx: &CallContext) -> MgResult<Value> {
        if let Err(d) = self.caps.check(&call.target) {
            return Err(self.located(d, call.span));
        }
        let mut args = Args::new();
        for (key, expr) in &call.args {
            args.insert(key.clone(), self.eval(expr, env, ctx).await?);
        }
        let Some(tool) = self.tools.get(&call.target) else {
            return Err(self.diag(
                call.span,
                format!("`{}` is declared but could not be set up", call.target),
            ));
        };
        let seconds = call
            .within
            .unwrap_or(self.rt.config.timeout_seconds as f64)
            .max(0.001);
        let mut task = Task::new(new_task_id(), format!("{}.{}", call.target, call.action));
        task.advance(TaskState::Running);
        let outcome = tokio::time::timeout(
            Duration::from_secs_f64(seconds),
            tool.call(&call.action, args, ctx),
        )
        .await;
        match outcome {
            Ok(Ok(value)) => {
                task.advance(TaskState::Completed);
                Ok(value)
            }
            Ok(Err(e)) => {
                task.advance(TaskState::Failed);
                let mut d = self.diag(call.span, e.message);
                d.suggestion = e.fix;
                d.related = e.related;
                Err(d)
            }
            Err(_) => {
                task.advance(TaskState::TimedOut);
                Err(self
                    .diag(
                        call.span,
                        format!(
                            "`{}.{}` did not finish within {} seconds",
                            call.target,
                            call.action,
                            trim_number(seconds)
                        ),
                    )
                    .fix(
                        "try again, or allow more time by ending the line with: within 60 seconds",
                    ))
            }
        }
    }

    fn located(&self, d: Diagnostic, span: Span) -> Diagnostic {
        d.located(
            &self.def.source.name,
            span.line,
            span.col,
            &self.def.source.text,
        )
    }

    fn compare(&self, op: CompareOp, l: &Value, r: &Value, span: Span) -> MgResult<Value> {
        let equal = |a: &Value, b: &Value| match (a, b) {
            (Value::Number(x), Value::Number(y)) => x == y,
            (Value::Text(_), Value::Number(y)) => a.as_number() == Some(*y),
            (Value::Number(x), Value::Text(_)) => b.as_number() == Some(*x),
            _ => a == b,
        };
        Ok(Value::Bool(match op {
            CompareOp::Is => equal(l, r),
            CompareOp::IsNot => !equal(l, r),
            CompareOp::MoreThan | CompareOp::LessThan => {
                let (Some(a), Some(b)) = (l.as_number(), r.as_number()) else {
                    return Err(self
                        .diag(
                            span,
                            format!(
                                "I can only compare numbers, but I got {} and {}",
                                describe(l),
                                describe(r)
                            ),
                        )
                        .fix("compare two numbers, for example: count is more than 3"));
                };
                if op == CompareOp::MoreThan {
                    a > b
                } else {
                    a < b
                }
            }
            CompareOp::Contains => match l {
                Value::Text(t) => t.contains(&r.to_display()),
                Value::List(items) => items.iter().any(|i| equal(i, r)),
                other => {
                    return Err(self
                        .diag(
                            span,
                            format!(
                                "`contains` needs text or a list on its left, but it got {}",
                                describe(other)
                            ),
                        )
                        .fix("use it like: message contains \"hello\""));
                }
            },
        }))
    }
}

pub(crate) fn describe(value: &Value) -> String {
    match value {
        Value::Nothing => "nothing".to_string(),
        Value::Bool(_) => "a yes/no value".to_string(),
        Value::Number(_) => "a number".to_string(),
        Value::Text(_) => "text".to_string(),
        Value::List(_) => "a list".to_string(),
        Value::Record(_) => "a record".to_string(),
    }
}

fn trim_number(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}
