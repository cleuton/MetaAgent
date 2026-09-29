//! The syntax tree of a `.ag` file. Every node knows where it came from.

use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

/// A source file with its text, kept so errors can show the offending line.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub name: String,
    pub path: Option<PathBuf>,
    pub text: Arc<str>,
}

impl SourceFile {
    pub fn from_text(name: &str, text: &str) -> Arc<SourceFile> {
        Arc::new(SourceFile {
            name: name.to_string(),
            path: None,
            text: Arc::from(text),
        })
    }
}

#[derive(Debug, Clone)]
pub struct AgentDef {
    pub name: String,
    pub span: Span,
    pub goal: Option<(String, Span)>,
    pub tools: Vec<ToolDecl>,
    pub links: Vec<LinkDecl>,
    pub remotes: Vec<RemoteDecl>,
    pub accepts: Vec<Accept>,
    pub handlers: Vec<Handler>,
    /// The optional `on start` handler. It is not part of `accepts`.
    pub start: Option<Handler>,
    pub source: Arc<SourceFile>,
}

impl AgentDef {
    pub fn handler(&self, message: &str) -> Option<&Handler> {
        self.handlers.iter().find(|h| h.message == message)
    }
    pub fn accept(&self, message: &str) -> Option<&Accept> {
        self.accepts.iter().find(|a| a.message == message)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolKind {
    File { scope: Option<String> },
    Http,
    State,
    Clock,
    Env { names: Vec<String> },
    Mcp { command: String },
}

#[derive(Debug, Clone)]
pub struct ToolDecl {
    /// The name used in calls, for example `file` or `weather`.
    pub name: String,
    pub kind: ToolKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct LinkDecl {
    pub name: String,
    pub path: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct RemoteDecl {
    pub name: String,
    pub url: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Accept {
    pub message: String,
    pub params: Vec<String>,
    pub description: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Handler {
    pub message: String,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Assign {
        name: String,
        value: Expr,
        span: Span,
    },
    Expr {
        expr: Expr,
        span: Span,
    },
    Reply {
        value: Expr,
        span: Span,
    },
    If {
        cond: Expr,
        then: Vec<Stmt>,
        otherwise: Vec<Stmt>,
        span: Span,
    },
    For {
        var: String,
        iter: Expr,
        body: Vec<Stmt>,
        span: Span,
    },
    Fail {
        value: Expr,
        span: Span,
    },
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Assign { span, .. }
            | Stmt::Expr { span, .. }
            | Stmt::Reply { span, .. }
            | Stmt::If { span, .. }
            | Stmt::For { span, .. }
            | Stmt::Fail { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TextPart {
    Lit(String),
    Var(Vec<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    Is,
    IsNot,
    MoreThan,
    LessThan,
    Contains,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Text(Vec<TextPart>, Span),
    Number(f64, Span),
    Bool(bool, Span),
    Nothing(Span),
    List(Vec<Expr>, Span),
    Path(Vec<String>, Span),
    Call(CallExpr),
    Think {
        prompt: Box<Expr>,
        span: Span,
    },
    Not(Box<Expr>, Span),
    And(Box<Expr>, Box<Expr>, Span),
    Or(Box<Expr>, Box<Expr>, Span),
    Compare {
        op: CompareOp,
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Text(_, s)
            | Expr::Number(_, s)
            | Expr::Bool(_, s)
            | Expr::Nothing(s)
            | Expr::List(_, s)
            | Expr::Path(_, s)
            | Expr::Not(_, s)
            | Expr::And(_, _, s)
            | Expr::Or(_, _, s) => *s,
            Expr::Call(c) => c.span,
            Expr::Think { span, .. } | Expr::Compare { span, .. } => *span,
        }
    }
}

/// `target.action key: value key: value [within N seconds]`
#[derive(Debug, Clone)]
pub struct CallExpr {
    pub target: String,
    pub action: String,
    pub args: Vec<(String, Expr)>,
    pub within: Option<f64>,
    pub span: Span,
}
