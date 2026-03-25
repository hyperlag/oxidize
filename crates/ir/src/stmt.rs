//! IR statements.

use crate::{IrExpr, IrType};
use serde::{Deserialize, Serialize};

/// A statement node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IrStmt {
    // ── Declarations ──────────────────────────────────────────────────────
    /// Local variable declaration: `Type name = init;`
    LocalVar {
        name: String,
        ty: IrType,
        init: Option<IrExpr>,
    },

    // ── Control flow ──────────────────────────────────────────────────────
    /// `if (cond) { then_ } else { else_ }`
    If {
        cond: IrExpr,
        then_: Vec<IrStmt>,
        else_: Option<Vec<IrStmt>>,
    },
    /// `while (cond) { body }`
    While {
        cond: IrExpr,
        body: Vec<IrStmt>,
    },
    /// `do { body } while (cond);`
    DoWhile {
        body: Vec<IrStmt>,
        cond: IrExpr,
    },
    /// Traditional `for (init; cond; update) { body }`
    For {
        init: Option<Box<IrStmt>>,
        cond: Option<IrExpr>,
        update: Vec<IrExpr>,
        body: Vec<IrStmt>,
    },
    /// Enhanced `for (Type var : iterable) { body }`
    ForEach {
        var: String,
        var_ty: IrType,
        iterable: IrExpr,
        body: Vec<IrStmt>,
    },
    /// Traditional `switch (expr) { case ... }`
    Switch {
        expr: IrExpr,
        cases: Vec<SwitchCase>,
        default: Option<Vec<IrStmt>>,
    },
    /// `return expr;` or `return;`
    Return(Option<IrExpr>),
    /// `break;` (with optional label)
    Break(Option<String>),
    /// `continue;` (with optional label)
    Continue(Option<String>),
    /// `throw expr;`
    Throw(IrExpr),
    /// `try { body } catch (E e) { handler } finally { finally_ }`
    TryCatch {
        body: Vec<IrStmt>,
        catches: Vec<CatchClause>,
        finally: Option<Vec<IrStmt>>,
    },

    // ── Concurrency ───────────────────────────────────────────────────────
    /// `synchronized (monitor) { body }`
    Synchronized {
        monitor: IrExpr,
        body: Vec<IrStmt>,
    },

    // ── OOP ───────────────────────────────────────────────────────────────
    /// Super constructor invocation: `super(args)` — only valid as the first
    /// statement in a sub-class constructor body.
    SuperConstructorCall {
        args: Vec<IrExpr>,
    },

    // ── Expressions as statements ─────────────────────────────────────────
    /// Expression statement (method call, assignment, etc.)
    Expr(IrExpr),

    // ── Blocks ────────────────────────────────────────────────────────────
    Block(Vec<IrStmt>),
}

/// One `case` arm inside a `switch`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwitchCase {
    /// The case value (a constant expression).
    pub value: IrExpr,
    pub body: Vec<IrStmt>,
}

/// One `catch` clause.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatchClause {
    /// Exception types caught (multi-catch allowed: `catch (A | B e)`).
    pub exception_types: Vec<String>,
    /// Bound variable name.
    pub var: String,
    pub body: Vec<IrStmt>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_roundtrip_if() {
        let stmt = IrStmt::If {
            cond: IrExpr::LitBool(true),
            then_: vec![IrStmt::Return(None)],
            else_: None,
        };
        let json = serde_json::to_string(&stmt).unwrap();
        let back: IrStmt = serde_json::from_str(&json).unwrap();
        assert_eq!(stmt, back);
    }

    #[test]
    fn serde_roundtrip_while() {
        let stmt = IrStmt::While {
            cond: IrExpr::LitBool(false),
            body: vec![],
        };
        let json = serde_json::to_string(&stmt).unwrap();
        let back: IrStmt = serde_json::from_str(&json).unwrap();
        assert_eq!(stmt, back);
    }
}
