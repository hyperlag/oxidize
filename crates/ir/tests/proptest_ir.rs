//! Property-based tests for IR lowering invariants using proptest.
//!
//! These tests verify structural properties that must hold for all IR nodes:
//! - Serde roundtrip fidelity (serialize → deserialize == identity)
//! - Type invariants (e.g. `ty()` on expressions always returns a known type)
//! - Display formatting produces non-empty strings for all IrType variants
//! - Structural consistency (nested types maintain invariants)

use ir::*;
use proptest::prelude::*;

// ─── Strategies for generating arbitrary IR nodes ──────────────────────────

fn arb_ir_primitive() -> impl Strategy<Value = IrType> {
    prop_oneof![
        Just(IrType::Bool),
        Just(IrType::Byte),
        Just(IrType::Short),
        Just(IrType::Int),
        Just(IrType::Long),
        Just(IrType::Float),
        Just(IrType::Double),
        Just(IrType::Char),
        Just(IrType::Void),
    ]
}

fn arb_class_name() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("Foo".to_string()),
        Just("Bar".to_string()),
        Just("java.lang.Object".to_string()),
        Just("java.util.List".to_string()),
        Just("MyClass".to_string()),
    ]
}

fn arb_ir_type() -> impl Strategy<Value = IrType> {
    let leaf = prop_oneof![
        arb_ir_primitive(),
        Just(IrType::String),
        Just(IrType::Unknown),
        Just(IrType::Null),
        arb_class_name().prop_map(IrType::Class),
        arb_class_name().prop_map(IrType::TypeVar),
    ];

    leaf.prop_recursive(3, 16, 4, |inner| {
        prop_oneof![
            inner.clone().prop_map(|t| IrType::Nullable(Box::new(t))),
            inner.clone().prop_map(|t| IrType::Array(Box::new(t))),
            inner.clone().prop_map(|t| IrType::Atomic(Box::new(t))),
            (inner.clone(), proptest::collection::vec(inner, 1..3)).prop_map(|(base, args)| {
                IrType::Generic {
                    base: Box::new(base),
                    args,
                }
            }),
        ]
    })
}

fn arb_binop() -> impl Strategy<Value = expr::BinOp> {
    prop_oneof![
        Just(expr::BinOp::Add),
        Just(expr::BinOp::Sub),
        Just(expr::BinOp::Mul),
        Just(expr::BinOp::Div),
        Just(expr::BinOp::Rem),
        Just(expr::BinOp::BitAnd),
        Just(expr::BinOp::BitOr),
        Just(expr::BinOp::BitXor),
        Just(expr::BinOp::Shl),
        Just(expr::BinOp::Shr),
        Just(expr::BinOp::UShr),
        Just(expr::BinOp::And),
        Just(expr::BinOp::Or),
        Just(expr::BinOp::Eq),
        Just(expr::BinOp::Ne),
        Just(expr::BinOp::Lt),
        Just(expr::BinOp::Le),
        Just(expr::BinOp::Gt),
        Just(expr::BinOp::Ge),
        Just(expr::BinOp::Concat),
    ]
}

fn arb_unop() -> impl Strategy<Value = expr::UnOp> {
    prop_oneof![
        Just(expr::UnOp::Neg),
        Just(expr::UnOp::Not),
        Just(expr::UnOp::BitNot),
        Just(expr::UnOp::PreInc),
        Just(expr::UnOp::PreDec),
        Just(expr::UnOp::PostInc),
        Just(expr::UnOp::PostDec),
    ]
}

fn arb_ir_expr() -> impl Strategy<Value = IrExpr> {
    // Use integer-valued doubles to avoid JSON float precision issues.
    let leaf = prop_oneof![
        any::<bool>().prop_map(IrExpr::LitBool),
        any::<i32>().prop_map(|v| IrExpr::LitInt(v as i64)),
        any::<i64>().prop_map(IrExpr::LitLong),
        (-1_000_000i32..1_000_000i32).prop_map(|v| IrExpr::LitDouble(v as f64)),
        any::<char>().prop_map(IrExpr::LitChar),
        "[a-zA-Z0-9 ]{0,20}".prop_map(IrExpr::LitString),
        Just(IrExpr::LitNull),
        (arb_class_name(), arb_ir_type()).prop_map(|(name, ty)| IrExpr::Var { name, ty }),
    ];

    leaf.prop_recursive(2, 8, 3, |inner| {
        prop_oneof![
            // BinOp
            (arb_binop(), inner.clone(), inner.clone(), arb_ir_type()).prop_map(
                |(op, lhs, rhs, ty)| {
                    IrExpr::BinOp {
                        op,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                        ty,
                    }
                }
            ),
            // UnOp
            (arb_unop(), inner.clone(), arb_ir_type()).prop_map(|(op, operand, ty)| {
                IrExpr::UnOp {
                    op,
                    operand: Box::new(operand),
                    ty,
                }
            }),
            // MethodCall
            (
                arb_class_name(),
                proptest::collection::vec(inner.clone(), 0..3),
                arb_ir_type()
            )
                .prop_map(|(method_name, args, ty)| {
                    IrExpr::MethodCall {
                        receiver: None,
                        method_name,
                        args,
                        ty,
                    }
                }),
            // New
            (
                arb_class_name(),
                proptest::collection::vec(inner.clone(), 0..3),
                arb_ir_type()
            )
                .prop_map(|(class, args, ty)| { IrExpr::New { class, args, ty } }),
            // Cast
            (arb_ir_type(), inner.clone()).prop_map(|(target, expr)| IrExpr::Cast {
                target,
                expr: Box::new(expr),
            }),
            // InstanceOf
            (inner.clone(), arb_ir_type()).prop_map(|(expr, check_type)| IrExpr::InstanceOf {
                expr: Box::new(expr),
                check_type,
            }),
        ]
    })
}

fn arb_ir_stmt() -> impl Strategy<Value = IrStmt> {
    let leaf = prop_oneof![
        Just(IrStmt::Return(None)),
        Just(IrStmt::Break(None)),
        Just(IrStmt::Continue(None)),
        (arb_class_name(), arb_ir_type()).prop_map(|(name, ty)| IrStmt::LocalVar {
            name,
            ty,
            init: None
        }),
        arb_ir_expr().prop_map(IrStmt::Expr),
        arb_ir_expr().prop_map(|e| IrStmt::Return(Some(e))),
    ];

    leaf.prop_recursive(2, 12, 4, |inner| {
        prop_oneof![
            // If statement
            (
                arb_ir_expr(),
                proptest::collection::vec(inner.clone(), 0..3)
            )
                .prop_map(|(cond, then_)| IrStmt::If {
                    cond,
                    then_,
                    else_: None,
                }),
            // While loop
            (
                arb_ir_expr(),
                proptest::collection::vec(inner.clone(), 0..3)
            )
                .prop_map(|(cond, body)| IrStmt::While { cond, body }),
            // Block
            proptest::collection::vec(inner.clone(), 0..4).prop_map(IrStmt::Block),
            // TryCatch
            proptest::collection::vec(inner.clone(), 1..3).prop_map(|body| IrStmt::TryCatch {
                body,
                catches: vec![],
                finally: None,
            }),
        ]
    })
}

fn arb_visibility() -> impl Strategy<Value = decl::Visibility> {
    prop_oneof![
        Just(decl::Visibility::Public),
        Just(decl::Visibility::Protected),
        Just(decl::Visibility::PackagePrivate),
        Just(decl::Visibility::Private),
    ]
}

fn arb_ir_param() -> impl Strategy<Value = decl::IrParam> {
    (arb_class_name(), arb_ir_type(), any::<bool>()).prop_map(|(name, ty, is_varargs)| {
        decl::IrParam {
            name,
            ty,
            is_varargs,
        }
    })
}

fn arb_ir_method() -> impl Strategy<Value = decl::IrMethod> {
    (
        arb_class_name(),
        arb_visibility(),
        any::<bool>(),
        proptest::collection::vec(arb_ir_param(), 0..3),
        arb_ir_type(),
        proptest::collection::vec(arb_ir_stmt(), 0..4),
    )
        .prop_map(
            |(name, visibility, is_static, params, return_ty, body)| decl::IrMethod {
                name,
                visibility,
                is_static,
                is_abstract: false,
                is_final: false,
                is_synchronized: false,
                type_params: vec![],
                params,
                return_ty,
                body: Some(body),
                throws: vec![],
            },
        )
}

fn arb_ir_class() -> impl Strategy<Value = decl::IrClass> {
    (
        arb_class_name(),
        arb_visibility(),
        proptest::collection::vec(arb_ir_method(), 0..3),
    )
        .prop_map(|(name, visibility, methods)| decl::IrClass {
            name,
            visibility,
            is_abstract: false,
            is_final: false,
            type_params: vec![],
            superclass: None,
            interfaces: vec![],
            fields: vec![],
            methods,
            constructors: vec![],
        })
}

fn arb_ir_module() -> impl Strategy<Value = IrModule> {
    (
        "[a-z]{0,10}".prop_map(|s| s.to_string()),
        proptest::collection::vec(arb_ir_class().prop_map(IrDecl::Class), 1..3),
    )
        .prop_map(|(package, decls)| IrModule {
            package,
            imports: vec![],
            decls,
        })
}

// ─── Property tests ───────────────────────────────────────────────────────

proptest! {
    /// All IrType values survive a JSON roundtrip.
    #[test]
    fn irtype_serde_roundtrip(ty in arb_ir_type()) {
        let json = serde_json::to_string(&ty).unwrap();
        let back: IrType = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&ty, &back);
    }

    /// Display for IrType always produces a non-empty string.
    #[test]
    fn irtype_display_non_empty(ty in arb_ir_type()) {
        let s = ty.to_string();
        prop_assert!(!s.is_empty(), "Display produced empty string for {:?}", ty);
    }

    /// Primitive types are correctly classified.
    #[test]
    fn irtype_primitive_is_primitive(ty in arb_ir_primitive()) {
        prop_assert!(ty.is_primitive());
        prop_assert!(!ty.is_reference());
    }

    /// Reference types are not primitive (excluding Unknown and Null).
    #[test]
    fn irtype_reference_not_primitive(ty in prop_oneof![
        Just(IrType::String),
        arb_class_name().prop_map(IrType::Class),
        Just(IrType::Array(Box::new(IrType::Int))),
        Just(IrType::Nullable(Box::new(IrType::String))),
    ]) {
        prop_assert!(!ty.is_primitive());
        prop_assert!(ty.is_reference());
    }

    /// All IrExpr values survive a JSON roundtrip.
    #[test]
    fn irexpr_serde_roundtrip(expr in arb_ir_expr()) {
        let json = serde_json::to_string(&expr).unwrap();
        let back: IrExpr = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&expr, &back);
    }

    /// IrExpr::ty() never returns Unknown for literal expressions.
    #[test]
    fn irexpr_literal_type_known(expr in prop_oneof![
        any::<bool>().prop_map(IrExpr::LitBool),
        any::<i32>().prop_map(|v| IrExpr::LitInt(v as i64)),
        any::<i64>().prop_map(IrExpr::LitLong),
        any::<char>().prop_map(IrExpr::LitChar),
        "[a-z]{0,10}".prop_map(IrExpr::LitString),
        Just(IrExpr::LitNull),
    ]) {
        let ty = expr.ty();
        prop_assert_ne!(ty, &IrType::Unknown,
            "Literal {:?} had Unknown type", expr);
    }

    /// BinOp expr type matches the annotated type.
    #[test]
    fn irexpr_binop_type_matches(
        op in arb_binop(),
        ty in arb_ir_type()
    ) {
        let expr = IrExpr::BinOp {
            op,
            lhs: Box::new(IrExpr::LitInt(1)),
            rhs: Box::new(IrExpr::LitInt(2)),
            ty: ty.clone(),
        };
        prop_assert_eq!(expr.ty(), &ty);
    }

    /// InstanceOf always has Bool type.
    #[test]
    fn irexpr_instanceof_is_bool(check_type in arb_ir_type()) {
        let expr = IrExpr::InstanceOf {
            expr: Box::new(IrExpr::LitNull),
            check_type,
        };
        prop_assert_eq!(expr.ty(), &IrType::Bool);
    }

    /// Cast expr type matches the target type.
    #[test]
    fn irexpr_cast_type_matches(target in arb_ir_type()) {
        let expr = IrExpr::Cast {
            target: target.clone(),
            expr: Box::new(IrExpr::LitInt(42)),
        };
        prop_assert_eq!(expr.ty(), &target);
    }

    /// All IrStmt values survive a JSON roundtrip.
    #[test]
    fn irstmt_serde_roundtrip(stmt in arb_ir_stmt()) {
        let json = serde_json::to_string(&stmt).unwrap();
        let back: IrStmt = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&stmt, &back);
    }

    /// IrModule survives a JSON roundtrip.
    #[test]
    fn irmodule_serde_roundtrip(module in arb_ir_module()) {
        let json = serde_json::to_string(&module).unwrap();
        let back: IrModule = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&module, &back);
    }

    /// An IrClass with methods preserves method count through serde roundtrip.
    #[test]
    fn irclass_method_count_preserved(cls in arb_ir_class()) {
        let count = cls.methods.len();
        let json = serde_json::to_string(&cls).unwrap();
        let back: decl::IrClass = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(back.methods.len(), count);
    }

    /// IrMethod param count preserved through serde roundtrip.
    #[test]
    fn irmethod_params_preserved(method in arb_ir_method()) {
        let count = method.params.len();
        let json = serde_json::to_string(&method).unwrap();
        let back: decl::IrMethod = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(back.params.len(), count);
    }
}
