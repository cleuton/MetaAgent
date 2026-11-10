//! The Metagente language front end: lexer, parser, syntax tree, and checks.

pub mod ast;
pub mod check;
pub mod lexer;
pub mod params; // 0.1.2: parameters of metagente.toml
pub mod parser;

pub use ast::*;
pub use params::Params; // 0.1.2: exported for the runtime
pub use parser::{parse_file, parse_file_with}; // 0.1.2: parse_file_with takes parameters

/// 0.1.5: the words of the language that cannot be the nickname of a model (`[llm.NAME]`, D1). One list for the parser
/// side and the models side. It holds the words the parser treats specially, and `using`, which stays an ordinary name
/// everywhere else but is refused as a nickname so that a nickname is never the word that introduces it.
pub const RESERVED_WORDS: &[&str] = &[
    "accepts",
    "agent",
    "and",
    "at",
    "clock",
    "contains",
    "env",
    "fail",
    "file",
    "for",
    "from",
    "goal",
    "http",
    "if",
    "in",
    "is",
    "less",
    "link",
    "mcp",
    "more",
    "no",
    "not",
    "nothing",
    "on",
    "or",
    "otherwise",
    "remote",
    "reply",
    "second",
    "seconds",
    "start",
    "state",
    "than",
    "think",
    "tool",
    "using",
    "within",
    "yes",
];
