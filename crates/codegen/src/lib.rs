//! Code-generation pass: lowers a typed [`ir::IrModule`] to a Rust token
//! stream using `proc-macro2` and `quote`.
//!
//! At Stage 0 this crate is a skeleton. The full implementation lives in
//! Stage 1.

use thiserror::Error;

/// Errors produced during code-generation.
#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("unsupported IR node: {0}")]
    Unsupported(String),
}
