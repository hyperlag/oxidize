//! Java source → IR lowering using tree-sitter-java.
//!
//! This crate is responsible for:
//! 1. Invoking the tree-sitter Java parser to produce a concrete syntax tree.
//! 2. Walking that CST and emitting [`ir::IrModule`] nodes.
//!
//! At Stage 0 the walker is minimal — it only confirms a file can be parsed
//! without errors (used by the smoke test). Full lowering is implemented in
//! Stage 1.

pub mod from_node;
pub mod walker;

pub use walker::parse_source;

use thiserror::Error;

/// Errors produced by the parser crate.
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("tree-sitter returned a parse error at byte offset {offset}: {message}")]
    SyntaxError { offset: usize, message: String },

    #[error("unsupported Java feature: {0}")]
    Unsupported(String),

    #[error("UTF-8 error in source byte range")]
    Utf8(#[from] std::str::Utf8Error),
}
