//! Dynamic link: one Metagente agent calling another, resolved when the call happens.

pub mod check;
pub mod cycle;
pub mod interface;
pub mod loader;
pub mod resolve;
pub mod tool;

pub use loader::Linker;
pub use tool::LinkTool;
