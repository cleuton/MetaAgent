//! The Metagente language front end: lexer, parser, syntax tree, and checks.

pub mod ast;
pub mod check;
pub mod lexer;
pub mod parser;

pub use ast::*;
pub use parser::parse_file;
