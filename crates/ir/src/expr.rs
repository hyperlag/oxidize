//! IR expressions.

use crate::IrType;
use serde::{Deserialize, Serialize};

/// A typed expression node.
///
/// Every variant that produces a value carries an implicit `ty` field provided
/// by the type-checker pass. Before type-checking, expressions may have
/// [`IrType::Unknown`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IrExpr {
    // ── Literals ──────────────────────────────────────────────────────────
    /// Unit value `()` — used for void-returning block lambdas.
    Unit,
    LitBool(bool),
    LitInt(i64),
    LitLong(i64),
    LitFloat(f64),
    LitDouble(f64),
    LitChar(char),
    LitString(String),
    LitNull,

    // ── Variables & fields ────────────────────────────────────────────────
    /// Local variable or parameter reference.
    Var {
        name: String,
        ty: IrType,
    },
    /// Field access: `receiver.field_name`.
    FieldAccess {
        receiver: Box<IrExpr>,
        field_name: String,
        ty: IrType,
    },

    // ── Operations ────────────────────────────────────────────────────────
    /// Binary operation: `lhs op rhs`.
    BinOp {
        op: BinOp,
        lhs: Box<IrExpr>,
        rhs: Box<IrExpr>,
        ty: IrType,
    },
    /// Unary operation: `op operand`.
    UnOp {
        op: UnOp,
        operand: Box<IrExpr>,
        ty: IrType,
    },
    /// Ternary / conditional expression: `cond ? then_ : else_`.
    Ternary {
        cond: Box<IrExpr>,
        then_: Box<IrExpr>,
        else_: Box<IrExpr>,
        ty: IrType,
    },

    // ── Calls ─────────────────────────────────────────────────────────────
    /// Static or instance method call.
    MethodCall {
        receiver: Option<Box<IrExpr>>,
        method_name: String,
        args: Vec<IrExpr>,
        ty: IrType,
    },
    /// Object construction: `new ClassName(args)`.
    New {
        class: String,
        args: Vec<IrExpr>,
        ty: IrType,
    },
    /// Array creation: `new int[len]`.
    NewArray {
        elem_ty: IrType,
        len: Box<IrExpr>,
        ty: IrType,
    },
    /// Multi-dimensional array creation: `new int[rows][cols]`.
    NewArrayMultiDim {
        elem_ty: IrType,
        dims: Vec<IrExpr>, // outer → inner dimension expressions
        ty: IrType,
    },

    /// Lambda expression: `x -> body` or `(x, y) -> body`.
    /// For block lambdas `(x) -> { stmts; return expr; }`, `body_stmts`
    /// holds the leading statements and `body` holds the final expression.
    Lambda {
        params: Vec<String>,
        body: Box<IrExpr>,
        body_stmts: Vec<crate::stmt::IrStmt>,
        ty: IrType,
    },

    // ── Casts & tests ─────────────────────────────────────────────────────
    /// Explicit cast: `(TargetType) expr`.
    Cast {
        target: IrType,
        expr: Box<IrExpr>,
    },
    /// `instanceof` check.
    InstanceOf {
        expr: Box<IrExpr>,
        check_type: IrType,
        /// Optional pattern-matching binding variable (Java 16+ pattern instanceof).
        #[serde(default)]
        binding: Option<String>,
    },

    // ── Arrays ────────────────────────────────────────────────────────────
    /// Array element access: `array[index]`.
    ArrayAccess {
        array: Box<IrExpr>,
        index: Box<IrExpr>,
        ty: IrType,
    },

    // ── Assignment ────────────────────────────────────────────────────────
    /// Assignment expression (returns the value assigned).
    Assign {
        lhs: Box<IrExpr>,
        rhs: Box<IrExpr>,
        ty: IrType,
    },
    /// Compound assignment: `lhs op= rhs`.
    CompoundAssign {
        op: BinOp,
        lhs: Box<IrExpr>,
        rhs: Box<IrExpr>,
        ty: IrType,
    },

    // ── Reflection ────────────────────────────────────────────────────────
    /// Class literal: `Foo.class` — produces a `JClass` descriptor.
    ClassLiteral {
        class_name: String,
    },
}

impl IrExpr {
    /// Returns the static type of this expression.
    pub fn ty(&self) -> &IrType {
        match self {
            IrExpr::Unit => &IrType::Void,
            IrExpr::LitBool(_) => &IrType::Bool,
            IrExpr::LitInt(_) => &IrType::Int,
            IrExpr::LitLong(_) => &IrType::Long,
            IrExpr::LitFloat(_) => &IrType::Float,
            IrExpr::LitDouble(_) => &IrType::Double,
            IrExpr::LitChar(_) => &IrType::Char,
            IrExpr::LitString(_) => &IrType::String,
            IrExpr::LitNull => &IrType::Null,
            IrExpr::Var { ty, .. }
            | IrExpr::FieldAccess { ty, .. }
            | IrExpr::BinOp { ty, .. }
            | IrExpr::UnOp { ty, .. }
            | IrExpr::Ternary { ty, .. }
            | IrExpr::MethodCall { ty, .. }
            | IrExpr::New { ty, .. }
            | IrExpr::NewArray { ty, .. }
            | IrExpr::NewArrayMultiDim { ty, .. }
            | IrExpr::ArrayAccess { ty, .. }
            | IrExpr::Assign { ty, .. }
            | IrExpr::CompoundAssign { ty, .. }
            | IrExpr::Lambda { ty, .. } => ty,
            IrExpr::Cast { target, .. } => target,
            IrExpr::InstanceOf { .. } => &IrType::Bool,
            IrExpr::ClassLiteral { .. } => &IrType::Unknown,
        }
    }
}

/// Binary operators.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    // Bitwise
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    UShr,
    // Logical
    And,
    Or,
    // Comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    // String concatenation (Java `+` when one operand is String)
    Concat,
}

/// Unary operators.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnOp {
    Neg,
    Not,
    BitNot,
    PreInc,
    PreDec,
    PostInc,
    PostDec,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lit_types() {
        assert_eq!(IrExpr::LitBool(true).ty(), &IrType::Bool);
        assert_eq!(IrExpr::LitString("hi".into()).ty(), &IrType::String);
        assert_eq!(IrExpr::LitNull.ty(), &IrType::Null);
    }

    #[test]
    fn serde_roundtrip() {
        let e = IrExpr::BinOp {
            op: BinOp::Add,
            lhs: Box::new(IrExpr::LitInt(1)),
            rhs: Box::new(IrExpr::LitInt(2)),
            ty: IrType::Int,
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: IrExpr = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }
}
