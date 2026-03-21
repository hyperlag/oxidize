//! Type-checking pass: walks an [`ir::IrModule`] and annotates every
//! [`ir::IrExpr`] with its [`ir::IrType`].
//!
//! At Stage 0 this crate is a skeleton. The full implementation lives in
//! Stage 1.

use thiserror::Error;

/// Errors produced during type-checking.
#[derive(Debug, Error)]
pub enum TypeckError {
    #[error("undefined variable: `{0}`")]
    UndefinedVariable(String),

    #[error("type mismatch: expected `{expected}`, found `{found}`")]
    TypeMismatch { expected: String, found: String },

    #[error("undefined class or interface: `{0}`")]
    UndefinedClass(String),
}
