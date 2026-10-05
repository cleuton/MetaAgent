//! Metagente: an interpreted 4GL for building AI agents.
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
// Diagnostics are the cold path and carry everything a person needs, so their size is fine.
#![allow(clippy::result_large_err)]
// Nested `if let` blocks read more plainly to beginners reading this code than let-chains.
#![allow(clippy::collapsible_if)]

pub mod a2a;
pub mod diagnostics;
pub mod lang;
pub mod link;
pub mod llm;
pub mod mcp;
pub mod runtime;
pub mod tools;
