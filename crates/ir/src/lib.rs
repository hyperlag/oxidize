//! Typed Intermediate Representation (IR) for the Java→Rust translator.
//!
//! All IR nodes derive `serde::Serialize` / `Deserialize` so that they can be
//! round-tripped to JSON for debugging, snapshot-testing, and fuzzing.

pub mod decl;
pub mod expr;
pub mod stmt;
pub mod types;

pub use decl::{IrDecl, IrEnum, IrEnumConstant};
pub use expr::IrExpr;
pub use stmt::IrStmt;
pub use types::{IrType, IrTypeParam, WildcardBound};

use serde::{Deserialize, Serialize};

/// A compiled translation unit — corresponds to one `.java` source file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrModule {
    /// Fully-qualified package name, e.g. `"com.example"`. Empty for the
    /// default package.
    pub package: String,
    /// Import list (not yet resolved — kept for diagnostic messages).
    pub imports: Vec<String>,
    /// Top-level declarations in source order.
    pub decls: Vec<IrDecl>,
}

impl IrModule {
    pub fn new(package: impl Into<String>) -> Self {
        Self {
            package: package.into(),
            imports: Vec::new(),
            decls: Vec::new(),
        }
    }
}
