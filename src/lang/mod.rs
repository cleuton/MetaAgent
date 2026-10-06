//! The Metagente language front end: lexer, parser, syntax tree, and checks.

pub mod ast;
pub mod check;
pub mod lexer;
pub mod params; // 0.1.2: parameters of metagente.toml
pub mod parser;

pub use ast::*;
pub use params::Params; // 0.1.2: exported for the runtime
pub use parser::{parse_file, parse_file_with}; // 0.1.2: parse_file_with takes parameters
