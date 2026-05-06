//! Type-checking pass: walks an [`ir::IrModule`] and annotates every
//! [`ir::IrExpr`] with its [`ir::IrType`].

use std::collections::HashMap;

use ir::{
    decl::{IrClass, IrEnum, IrMethod},
    expr::BinOp,
    IrDecl, IrExpr, IrModule, IrStmt, IrType,
};
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

/// Run the type-checking pass over `module`, annotating all expressions in
/// place.  Returns the annotated module.
pub fn type_check(mut module: IrModule) -> Result<IrModule, TypeckError> {
    // Build a class map for name resolution
    let class_map: HashMap<String, IrClass> = module
        .decls
        .iter()
        .filter_map(|d| {
            if let IrDecl::Class(c) = d {
                Some((c.name.clone(), c.clone()))
            } else {
                None
            }
        })
        .collect();

    // Build an enum map for name resolution
    let enum_map: HashMap<String, IrEnum> = module
        .decls
        .iter()
        .filter_map(|d| {
            if let IrDecl::Enum(e) = d {
                Some((e.name.clone(), e.clone()))
            } else {
                None
            }
        })
        .collect();

    for decl in &mut module.decls {
        match decl {
            IrDecl::Class(cls) => check_class(cls, &class_map, &enum_map)?,
            IrDecl::Interface(_) => {} // interface methods are abstract
            IrDecl::Enum(enm) => check_enum(enm, &class_map, &enum_map)?,
        }
    }
    Ok(module)
}

fn check_class(
    cls: &mut IrClass,
    class_map: &HashMap<String, IrClass>,
    enum_map: &HashMap<String, IrEnum>,
) -> Result<(), TypeckError> {
    let cls_snapshot = cls.clone();

    for method in &mut cls.methods {
        check_method(method, &cls_snapshot, class_map, enum_map)?;
    }
    for ctor in &mut cls.constructors {
        let mut local_env: HashMap<String, IrType> = HashMap::new();
        local_env.insert(
            "__self__".to_owned(),
            IrType::Class(cls_snapshot.name.clone()),
        );
        for p in &ctor.params {
            local_env.insert(p.name.clone(), p.ty.clone());
        }
        for stmt in &mut ctor.body {
            check_stmt(stmt, &cls_snapshot, class_map, enum_map, &mut local_env)?;
        }
    }

    let static_env: HashMap<String, IrType> = HashMap::new();
    for field in &mut cls.fields {
        if let Some(init) = &mut field.init {
            *init = check_expr(
                init.clone(),
                &cls_snapshot,
                class_map,
                enum_map,
                &static_env,
            )?;
        }
    }
    Ok(())
}

fn check_enum(
    enm: &mut IrEnum,
    class_map: &HashMap<String, IrClass>,
    enum_map: &HashMap<String, IrEnum>,
) -> Result<(), TypeckError> {
    // Build a synthetic IrClass so we can reuse check_method/check_stmt
    let synthetic_cls = IrClass {
        name: enm.name.clone(),
        visibility: enm.visibility,
        is_abstract: false,
        is_final: true,
        type_params: vec![],
        superclass: None,
        interfaces: enm.interfaces.clone(),
        fields: enm.fields.clone(),
        methods: enm.methods.clone(),
        constructors: enm.constructors.clone(),
        is_record: false,
        captures: vec![],
    };

    for method in &mut enm.methods {
        check_method(method, &synthetic_cls, class_map, enum_map)?;
    }
    for ctor in &mut enm.constructors {
        let mut local_env: HashMap<String, IrType> = HashMap::new();
        local_env.insert("__self__".to_owned(), IrType::Class(enm.name.clone()));
        for p in &ctor.params {
            local_env.insert(p.name.clone(), p.ty.clone());
        }
        for stmt in &mut ctor.body {
            check_stmt(stmt, &synthetic_cls, class_map, enum_map, &mut local_env)?;
        }
    }
    Ok(())
}

fn check_method(
    method: &mut IrMethod,
    cls: &IrClass,
    class_map: &HashMap<String, IrClass>,
    enum_map: &HashMap<String, IrEnum>,
) -> Result<(), TypeckError> {
    let Some(body) = &mut method.body else {
        return Ok(());
    };
    let mut env: HashMap<String, IrType> = HashMap::new();
    if !method.is_static {
        env.insert("self".to_owned(), IrType::Class(cls.name.clone()));
    }
    for p in &method.params {
        env.insert(p.name.clone(), p.ty.clone());
    }
    for stmt in body {
        check_stmt(stmt, cls, class_map, enum_map, &mut env)?;
    }
    Ok(())
}

/// Returns `"self"`, `"__self__"`, or `""` depending on which self-binding
/// is currently in scope.
fn current_self(env: &HashMap<String, IrType>) -> &'static str {
    if env.contains_key("__self__") {
        "__self__"
    } else if env.contains_key("self") {
        "self"
    } else {
        ""
    }
}

/// Look up `field_name` in `cls` and its superclass chain.  Returns
/// `Some((field_type, super_hops))` where `super_hops` is the count of
/// `_super` field de-references needed to reach the owning class.
fn lookup_field_with_path(
    field_name: &str,
    cls: &IrClass,
    class_map: &HashMap<String, IrClass>,
) -> Option<(IrType, Vec<String>)> {
    if let Some(f) = cls
        .fields
        .iter()
        .find(|f| f.name == field_name && !f.is_static)
    {
        return Some((f.ty.clone(), vec![]));
    }
    if let Some(parent_name) = &cls.superclass {
        if let Some(parent_cls) = class_map.get(parent_name) {
            if let Some((ty, mut path)) = lookup_field_with_path(field_name, parent_cls, class_map)
            {
                path.insert(0, "_super".to_owned());
                return Some((ty, path));
            }
        }
    }
    None
}

/// Look up method return type in `cls` and its superclass chain.
fn lookup_method_return_type(
    method_name: &str,
    cls: &IrClass,
    class_map: &HashMap<String, IrClass>,
) -> Option<IrType> {
    if let Some(m) = cls.methods.iter().find(|m| m.name == method_name) {
        return Some(m.return_ty.clone());
    }
    if let Some(parent_name) = &cls.superclass {
        if let Some(parent_cls) = class_map.get(parent_name) {
            return lookup_method_return_type(method_name, parent_cls, class_map);
        }
    }
    None
}

/// Build a receiver expression starting from `self_name` and applying
/// `super_path` hops (each hop adds a `._super` field access).
fn build_self_path(self_name: &str, super_path: &[String]) -> IrExpr {
    let base = IrExpr::Var {
        name: self_name.to_owned(),
        ty: IrType::Unknown,
    };
    super_path
        .iter()
        .fold(base, |recv, hop| IrExpr::FieldAccess {
            receiver: Box::new(recv),
            field_name: hop.clone(),
            ty: IrType::Unknown,
        })
}

fn check_stmt(
    stmt: &mut IrStmt,
    cls: &IrClass,
    class_map: &HashMap<String, IrClass>,
    enum_map: &HashMap<String, IrEnum>,
    env: &mut HashMap<String, IrType>,
) -> Result<(), TypeckError> {
    match stmt {
        IrStmt::LocalVar { name, ty, init } => {
            if let Some(init_expr) = init {
                *init_expr = check_expr(init_expr.clone(), cls, class_map, enum_map, env)?;
                // if type is Unknown, infer from init
                if *ty == IrType::Unknown {
                    *ty = init_expr.ty().clone();
                }
            }
            env.insert(name.clone(), ty.clone());
        }
        IrStmt::Expr(e) => {
            *e = check_expr(e.clone(), cls, class_map, enum_map, env)?;
        }
        IrStmt::Return(Some(e)) => {
            *e = check_expr(e.clone(), cls, class_map, enum_map, env)?;
        }
        IrStmt::Return(None) => {}
        IrStmt::If { cond, then_, else_ } => {
            *cond = check_expr(cond.clone(), cls, class_map, enum_map, env)?;
            // Pattern instanceof: inject the binding variable into the then-branch env.
            let mut then_env = env.clone();
            if let IrExpr::InstanceOf {
                binding: Some(binding_name),
                check_type,
                ..
            } = cond
            {
                then_env.insert(binding_name.clone(), check_type.clone());
            }
            for s in then_.iter_mut() {
                check_stmt(s, cls, class_map, enum_map, &mut then_env)?;
            }
            if let Some(else_stmts) = else_ {
                for s in else_stmts.iter_mut() {
                    check_stmt(s, cls, class_map, enum_map, &mut env.clone())?;
                }
            }
        }
        IrStmt::While { cond, body, .. } | IrStmt::DoWhile { body, cond, .. } => {
            *cond = check_expr(cond.clone(), cls, class_map, enum_map, env)?;
            let mut loop_env = env.clone();
            for s in body.iter_mut() {
                check_stmt(s, cls, class_map, enum_map, &mut loop_env)?;
            }
        }
        IrStmt::For {
            init,
            cond,
            update,
            body,
            ..
        } => {
            let mut loop_env = env.clone();
            if let Some(init_stmt) = init {
                check_stmt(init_stmt, cls, class_map, enum_map, &mut loop_env)?;
            }
            if let Some(c) = cond {
                *c = check_expr(c.clone(), cls, class_map, enum_map, &loop_env)?;
            }
            for u in update.iter_mut() {
                *u = check_expr(u.clone(), cls, class_map, enum_map, &loop_env)?;
            }
            for s in body.iter_mut() {
                check_stmt(s, cls, class_map, enum_map, &mut loop_env)?;
            }
        }
        IrStmt::ForEach {
            var,
            var_ty,
            iterable,
            body,
            ..
        } => {
            *iterable = check_expr(iterable.clone(), cls, class_map, enum_map, env)?;
            let mut loop_env = env.clone();
            loop_env.insert(var.clone(), var_ty.clone());
            for s in body.iter_mut() {
                check_stmt(s, cls, class_map, enum_map, &mut loop_env)?;
            }
        }
        IrStmt::Switch {
            expr,
            cases,
            default,
        } => {
            *expr = check_expr(expr.clone(), cls, class_map, enum_map, env)?;
            for case in cases.iter_mut() {
                case.value = check_expr(case.value.clone(), cls, class_map, enum_map, env)?;
                let mut case_env = env.clone();
                for s in case.body.iter_mut() {
                    check_stmt(s, cls, class_map, enum_map, &mut case_env)?;
                }
            }
            if let Some(def_stmts) = default {
                let mut def_env = env.clone();
                for s in def_stmts.iter_mut() {
                    check_stmt(s, cls, class_map, enum_map, &mut def_env)?;
                }
            }
        }
        IrStmt::Block(stmts) => {
            let mut block_env = env.clone();
            for s in stmts.iter_mut() {
                check_stmt(s, cls, class_map, enum_map, &mut block_env)?;
            }
        }
        IrStmt::Throw(e) => {
            *e = check_expr(e.clone(), cls, class_map, enum_map, env)?;
        }
        IrStmt::TryCatch {
            body,
            catches,
            finally,
        } => {
            let mut try_env = env.clone();
            for s in body.iter_mut() {
                check_stmt(s, cls, class_map, enum_map, &mut try_env)?;
            }
            for catch in catches.iter_mut() {
                let mut catch_env = env.clone();
                catch_env.insert(
                    catch.var.clone(),
                    IrType::Class(catch.exception_types.first().cloned().unwrap_or_default()),
                );
                for s in catch.body.iter_mut() {
                    check_stmt(s, cls, class_map, enum_map, &mut catch_env)?;
                }
            }
            if let Some(fin) = finally {
                let mut fin_env = env.clone();
                for s in fin.iter_mut() {
                    check_stmt(s, cls, class_map, enum_map, &mut fin_env)?;
                }
            }
        }
        IrStmt::SuperConstructorCall { args } => {
            for a in args.iter_mut() {
                *a = check_expr(a.clone(), cls, class_map, enum_map, env)?;
            }
        }
        IrStmt::Break(_) | IrStmt::Continue(_) => {}
        IrStmt::Synchronized { monitor, body } => {
            *monitor = check_expr(monitor.clone(), cls, class_map, enum_map, env)?;
            let mut sync_env = env.clone();
            for s in body.iter_mut() {
                check_stmt(s, cls, class_map, enum_map, &mut sync_env)?;
            }
        }
    }
    Ok(())
}

#[allow(clippy::only_used_in_recursion)]
fn check_expr(
    expr: IrExpr,
    cls: &IrClass,
    class_map: &HashMap<String, IrClass>,
    enum_map: &HashMap<String, IrEnum>,
    env: &HashMap<String, IrType>,
) -> Result<IrExpr, TypeckError> {
    match expr {
        // Unit / Literals already have correct types
        IrExpr::Unit
        | IrExpr::LitBool(_)
        | IrExpr::LitInt(_)
        | IrExpr::LitLong(_)
        | IrExpr::LitFloat(_)
        | IrExpr::LitDouble(_)
        | IrExpr::LitChar(_)
        | IrExpr::LitString(_)
        | IrExpr::LitNull => Ok(expr),

        // Variable references
        IrExpr::Var { ref name, .. } => {
            let name = name.clone();

            // `super` keyword (lowered as "_super") → rewrite to self._super
            if name == "_super" {
                let self_name = current_self(env);
                if !self_name.is_empty() {
                    let super_ty = cls
                        .superclass
                        .as_ref()
                        .map(|s| IrType::Class(s.clone()))
                        .unwrap_or(IrType::Unknown);
                    return Ok(IrExpr::FieldAccess {
                        receiver: Box::new(IrExpr::Var {
                            name: self_name.to_owned(),
                            ty: IrType::Unknown,
                        }),
                        field_name: "_super".to_owned(),
                        ty: super_ty,
                    });
                }
            }

            // Java `this` in a constructor body got lowered to `self` but we
            // renamed the self-binding to `__self__` → patch it up.
            if name == "self" && env.contains_key("__self__") && !env.contains_key("self") {
                return Ok(IrExpr::Var {
                    name: "__self__".to_owned(),
                    ty: IrType::Class(cls.name.clone()),
                });
            }

            // Lookup in local env (params, locals, self, __self__)
            if let Some(t) = env.get(&name) {
                return Ok(IrExpr::Var {
                    ty: t.clone(),
                    name,
                });
            }

            // Bare instance-field reference (e.g. `count` for `this.count`) —
            // rewrite to an explicit field-access expression.
            let self_name = current_self(env);
            if !self_name.is_empty() {
                if let Some((field_ty, super_path)) = lookup_field_with_path(&name, cls, class_map)
                {
                    let receiver = build_self_path(self_name, &super_path);
                    return Ok(IrExpr::FieldAccess {
                        receiver: Box::new(receiver),
                        field_name: name,
                        ty: field_ty,
                    });
                }
            }

            // Could be a class name (e.g. System) — leave as Unknown
            Ok(IrExpr::Var {
                ty: IrType::Unknown,
                name,
            })
        }

        IrExpr::FieldAccess {
            receiver,
            field_name,
            ..
        } => {
            let receiver = check_expr(*receiver, cls, class_map, enum_map, env)?;

            // The synthetic `_super` field is not in IrClass.fields; handle it
            // specially so we always get the parent class type.
            if field_name == "_super" {
                let super_ty = cls
                    .superclass
                    .as_ref()
                    .map(|s| IrType::Class(s.clone()))
                    .unwrap_or(IrType::Unknown);
                return Ok(IrExpr::FieldAccess {
                    receiver: Box::new(receiver),
                    field_name,
                    ty: super_ty,
                });
            }

            // When the receiver has a known class type, try to resolve the
            // field through the inheritance chain and insert `_super` hops as
            // needed.
            let recv_ty = receiver.ty().clone();

            // If the receiver is a bare name that matches a known enum, resolve
            // its constants: `Color.RED` → type `Class("Color")`.
            if let IrExpr::Var { ref name, .. } = receiver {
                if let Some(enm) = enum_map.get(name.as_str()) {
                    if enm.constants.iter().any(|c| c.name == field_name) {
                        return Ok(IrExpr::FieldAccess {
                            receiver: Box::new(receiver),
                            field_name,
                            ty: IrType::Class(enm.name.clone()),
                        });
                    }
                }
            }

            // If the receiver's type is a known enum type, resolve enum-typed
            // fields (e.g. instance fields declared inside the enum body).
            if let IrType::Class(ref class_name) = recv_ty {
                if let Some(enm) = enum_map.get(class_name.as_str()) {
                    if let Some(f) = enm.fields.iter().find(|f| f.name == field_name) {
                        return Ok(IrExpr::FieldAccess {
                            receiver: Box::new(receiver),
                            field_name,
                            ty: f.ty.clone(),
                        });
                    }
                }
            }

            if let IrType::Class(class_name) = &recv_ty {
                // Use the live `cls` for the current class (not the snapshot in
                // class_map, though they have the same fields).
                let lookup_in = if class_name == &cls.name {
                    cls
                } else {
                    match class_map.get(class_name.as_str()) {
                        Some(c) => c,
                        None => {
                            let ty = resolve_field_type(&receiver, &field_name, class_map);
                            return Ok(IrExpr::FieldAccess {
                                receiver: Box::new(receiver),
                                field_name,
                                ty,
                            });
                        }
                    }
                };

                if let Some((field_ty, super_path)) =
                    lookup_field_with_path(&field_name, lookup_in, class_map)
                {
                    let new_receiver =
                        super_path
                            .iter()
                            .fold(receiver, |r, hop| IrExpr::FieldAccess {
                                receiver: Box::new(r),
                                field_name: hop.clone(),
                                ty: IrType::Unknown,
                            });
                    return Ok(IrExpr::FieldAccess {
                        receiver: Box::new(new_receiver),
                        field_name,
                        ty: field_ty,
                    });
                }
            }

            // Fall back to old resolution (System.out, array.length, etc.)
            let ty = resolve_field_type(&receiver, &field_name, class_map);
            Ok(IrExpr::FieldAccess {
                receiver: Box::new(receiver),
                field_name,
                ty,
            })
        }

        IrExpr::MethodCall {
            receiver,
            method_name,
            args,
            ..
        } => {
            let receiver = receiver
                .map(|r| check_expr(*r, cls, class_map, enum_map, env).map(Box::new))
                .transpose()?;
            let args = args
                .into_iter()
                .map(|a| check_expr(a, cls, class_map, enum_map, env))
                .collect::<Result<Vec<_>, _>>()?;
            let ty = resolve_method_return_type(
                receiver.as_deref(),
                &method_name,
                &args,
                cls,
                class_map,
            );

            // Enum built-in methods: resolve types that `resolve_method_return_type`
            // cannot infer because it has no access to enum_map.
            let ty = if ty == IrType::Unknown {
                resolve_enum_method_type(receiver.as_deref(), &method_name, enum_map).unwrap_or(ty)
            } else {
                ty
            };
            Ok(IrExpr::MethodCall {
                receiver,
                method_name,
                args,
                ty,
            })
        }

        IrExpr::BinOp { op, lhs, rhs, .. } => {
            let lhs = check_expr(*lhs, cls, class_map, enum_map, env)?;
            let rhs = check_expr(*rhs, cls, class_map, enum_map, env)?;
            let ty = binop_type(&op, lhs.ty(), rhs.ty());
            Ok(IrExpr::BinOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                ty,
            })
        }

        IrExpr::UnOp { op, operand, .. } => {
            let operand = check_expr(*operand, cls, class_map, enum_map, env)?;
            let ty = operand.ty().clone();
            Ok(IrExpr::UnOp {
                op,
                operand: Box::new(operand),
                ty,
            })
        }

        IrExpr::Ternary {
            cond, then_, else_, ..
        } => {
            let cond = check_expr(*cond, cls, class_map, enum_map, env)?;
            let then_ = check_expr(*then_, cls, class_map, enum_map, env)?;
            let else_ = check_expr(*else_, cls, class_map, enum_map, env)?;
            let ty = then_.ty().clone();
            Ok(IrExpr::Ternary {
                cond: Box::new(cond),
                then_: Box::new(then_),
                else_: Box::new(else_),
                ty,
            })
        }

        IrExpr::Assign { lhs, rhs, .. } => {
            let lhs = check_expr(*lhs, cls, class_map, enum_map, env)?;
            let rhs = check_expr(*rhs, cls, class_map, enum_map, env)?;
            let ty = lhs.ty().clone();
            Ok(IrExpr::Assign {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                ty,
            })
        }

        IrExpr::CompoundAssign { op, lhs, rhs, .. } => {
            let lhs = check_expr(*lhs, cls, class_map, enum_map, env)?;
            let rhs = check_expr(*rhs, cls, class_map, enum_map, env)?;
            let ty = lhs.ty().clone();
            Ok(IrExpr::CompoundAssign {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                ty,
            })
        }

        IrExpr::New { class, args, .. } => {
            let args = args
                .into_iter()
                .map(|a| check_expr(a, cls, class_map, enum_map, env))
                .collect::<Result<Vec<_>, _>>()?;
            let ty = IrType::Class(class.clone());
            Ok(IrExpr::New { class, args, ty })
        }

        IrExpr::NewArray { elem_ty, len, .. } => {
            let len = check_expr(*len, cls, class_map, enum_map, env)?;
            let ty = IrType::Array(Box::new(elem_ty.clone()));
            Ok(IrExpr::NewArray {
                elem_ty,
                len: Box::new(len),
                ty,
            })
        }

        IrExpr::ArrayAccess { array, index, .. } => {
            let array = check_expr(*array, cls, class_map, enum_map, env)?;
            let index = check_expr(*index, cls, class_map, enum_map, env)?;
            let ty = if let IrType::Array(elem) = array.ty() {
                *elem.clone()
            } else {
                IrType::Unknown
            };
            Ok(IrExpr::ArrayAccess {
                array: Box::new(array),
                index: Box::new(index),
                ty,
            })
        }

        IrExpr::NewArrayMultiDim { elem_ty, dims, ty } => {
            let dims = dims
                .into_iter()
                .map(|d| check_expr(d, cls, class_map, enum_map, env))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(IrExpr::NewArrayMultiDim { elem_ty, dims, ty })
        }

        IrExpr::Cast { target, expr } => {
            let expr = check_expr(*expr, cls, class_map, enum_map, env)?;
            Ok(IrExpr::Cast {
                target,
                expr: Box::new(expr),
            })
        }

        IrExpr::InstanceOf {
            expr,
            check_type,
            binding,
        } => {
            let expr = check_expr(*expr, cls, class_map, enum_map, env)?;
            Ok(IrExpr::InstanceOf {
                expr: Box::new(expr),
                check_type,
                binding,
            })
        }

        IrExpr::Lambda {
            params,
            body,
            body_stmts,
            ..
        } => {
            // Add each param as Unknown-typed into a child env
            let mut lambda_env = env.clone();
            for p in &params {
                lambda_env.insert(p.clone(), IrType::Unknown);
            }
            let mut checked_stmts = body_stmts;
            for s in checked_stmts.iter_mut() {
                check_stmt(s, cls, class_map, enum_map, &mut lambda_env)?;
            }
            let body = check_expr(*body, cls, class_map, enum_map, &lambda_env)?;
            Ok(IrExpr::Lambda {
                params,
                body: Box::new(body),
                body_stmts: checked_stmts,
                ty: IrType::Unknown,
            })
        }

        IrExpr::ClassLiteral { class_name } => Ok(IrExpr::ClassLiteral { class_name }),

        IrExpr::MethodRef {
            class_name,
            target,
            method_name,
            ..
        } => {
            let checked_target = target
                .map(|t| check_expr(*t, cls, class_map, enum_map, env).map(Box::new))
                .transpose()?;
            Ok(IrExpr::MethodRef {
                class_name,
                target: checked_target,
                method_name,
                ty: IrType::Unknown,
            })
        }

        IrExpr::SwitchExpr {
            expr,
            arms,
            default,
            ..
        } => {
            let checked_expr = check_expr(*expr, cls, class_map, enum_map, env)?;
            let checked_arms = arms
                .into_iter()
                .map(|(pat, body)| {
                    let p = check_expr(pat, cls, class_map, enum_map, env)?;
                    let b = check_expr(body, cls, class_map, enum_map, env)?;
                    Ok::<_, TypeckError>((p, b))
                })
                .collect::<Result<Vec<_>, TypeckError>>()?;
            let checked_default = default
                .map(|d| check_expr(*d, cls, class_map, enum_map, env).map(Box::new))
                .transpose()?;
            // Determine result type from the first arm or default.
            let ty = checked_arms
                .first()
                .map(|(_, b)| b.ty().clone())
                .or_else(|| checked_default.as_deref().map(|d: &IrExpr| d.ty().clone()))
                .unwrap_or(IrType::Unknown);
            Ok(IrExpr::SwitchExpr {
                expr: Box::new(checked_expr),
                arms: checked_arms,
                default: checked_default,
                ty,
            })
        }

        IrExpr::BlockExpr {
            mut stmts, expr, ..
        } => {
            let mut block_env = env.clone();
            for s in &mut stmts {
                check_stmt(s, cls, class_map, enum_map, &mut block_env)?;
            }
            let checked_expr = check_expr(*expr, cls, class_map, enum_map, &block_env)?;
            let ty = checked_expr.ty().clone();
            Ok(IrExpr::BlockExpr {
                stmts,
                expr: Box::new(checked_expr),
                ty,
            })
        }

        IrExpr::PatternSwitchExpr {
            scrutinee,
            arms,
            default,
            ..
        } => {
            let checked_scrutinee = check_expr(*scrutinee, cls, class_map, enum_map, env)?;
            let checked_arms = arms
                .into_iter()
                .map(|(arm_ty, binding, body)| {
                    // Inject the binding variable into a child env so the arm
                    // body expression can reference it.
                    let mut arm_env = env.clone();
                    arm_env.insert(binding.clone(), arm_ty.clone());
                    let checked_body = check_expr(body, cls, class_map, enum_map, &arm_env)?;
                    Ok::<_, TypeckError>((arm_ty, binding, checked_body))
                })
                .collect::<Result<Vec<_>, TypeckError>>()?;
            let checked_default = default
                .map(|d| check_expr(*d, cls, class_map, enum_map, env).map(Box::new))
                .transpose()?;
            let ty = checked_arms
                .first()
                .map(|(_, _, b)| b.ty().clone())
                .or_else(|| checked_default.as_deref().map(|d: &IrExpr| d.ty().clone()))
                .unwrap_or(IrType::Unknown);
            Ok(IrExpr::PatternSwitchExpr {
                scrutinee: Box::new(checked_scrutinee),
                arms: checked_arms,
                default: checked_default,
                ty,
            })
        }
    }
}

fn binop_type(op: &BinOp, lhs: &IrType, rhs: &IrType) -> IrType {
    match op {
        BinOp::Eq
        | BinOp::Ne
        | BinOp::Lt
        | BinOp::Le
        | BinOp::Gt
        | BinOp::Ge
        | BinOp::And
        | BinOp::Or => IrType::Bool,
        BinOp::Concat => IrType::String,
        BinOp::Add if lhs == &IrType::String || rhs == &IrType::String => IrType::String,
        _ => widen_numeric(lhs, rhs),
    }
}

fn widen_numeric(a: &IrType, b: &IrType) -> IrType {
    match (a, b) {
        (IrType::Double, _) | (_, IrType::Double) => IrType::Double,
        (IrType::Float, _) | (_, IrType::Float) => IrType::Float,
        (IrType::Long, _) | (_, IrType::Long) => IrType::Long,
        (IrType::Int, _) | (_, IrType::Int) => IrType::Int,
        (IrType::Short, _) | (_, IrType::Short) => IrType::Short,
        (IrType::Byte, _) | (_, IrType::Byte) => IrType::Byte,
        _ => a.clone(),
    }
}

fn resolve_field_type(
    receiver: &IrExpr,
    field_name: &str,
    class_map: &HashMap<String, IrClass>,
) -> IrType {
    // length field on arrays
    if field_name == "length" {
        if let IrType::Array(_) = receiver.ty() {
            return IrType::Int;
        }
    }
    // Known static fields
    match (receiver_name(receiver).as_deref(), field_name) {
        (Some("System"), "out") | (Some("System"), "err") => {
            IrType::Class("PrintStream".to_owned())
        }
        _ => {
            // Look up in class map by variable name
            if let IrExpr::Var { name, .. } = receiver {
                if let Some(cls) = class_map.get(name.as_str()) {
                    if let Some(f) = cls.fields.iter().find(|f| f.name == field_name) {
                        return f.ty.clone();
                    }
                }
            }
            IrType::Unknown
        }
    }
}

fn receiver_name(expr: &IrExpr) -> Option<String> {
    if let IrExpr::Var { name, .. } = expr {
        Some(name.clone())
    } else {
        None
    }
}

/// Resolve the return type of built-in enum methods and static enum factory
/// methods that `resolve_method_return_type` cannot infer on its own.
///
/// Returns `Some(ty)` when a type can be determined, `None` otherwise.
fn resolve_enum_method_type(
    receiver: Option<&IrExpr>,
    method_name: &str,
    enum_map: &HashMap<String, IrEnum>,
) -> Option<IrType> {
    let recv = receiver?;
    match recv.ty() {
        // Instance method on an enum value (e.g. `color.name()`)
        IrType::Class(class_name) if enum_map.contains_key(class_name.as_str()) => {
            Some(match method_name {
                "name" => IrType::String,
                "ordinal" => IrType::Int,
                "equals" => IrType::Bool,
                "compareTo" => IrType::Int, // compareTo returns int (-1, 0, or 1)
                _ => return None,
            })
        }
        // Static method called as `EnumName.values()` / `EnumName.valueOf(…)`
        // The receiver is a bare Var whose name matches an enum declaration.
        IrType::Unknown => {
            if let IrExpr::Var { name, .. } = recv {
                if let Some(enm) = enum_map.get(name.as_str()) {
                    return Some(match method_name {
                        "values" => IrType::Array(Box::new(IrType::Class(enm.name.clone()))),
                        "valueOf" => IrType::Class(enm.name.clone()),
                        _ => return None,
                    });
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract the base class name from a type, handling both `Class(name)` and
/// `Generic { base: Class(name), .. }`. Returns `None` for all other types.
fn type_class_name(ty: &IrType) -> Option<&str> {
    match ty {
        IrType::Class(name) => Some(name.as_str()),
        IrType::Generic { base, .. } => type_class_name(base),
        _ => None,
    }
}

fn resolve_method_return_type(
    receiver: Option<&IrExpr>,
    method_name: &str,
    _args: &[IrExpr],
    cls: &IrClass,
    class_map: &HashMap<String, IrClass>,
) -> IrType {
    // System.out.println / System.err.println → void
    if method_name == "println" || method_name == "print" {
        return IrType::Void;
    }

    // Math static methods
    if let Some(IrExpr::Var { name, .. }) = receiver {
        if name == "Math" {
            return match method_name {
                "abs" | "max" | "min" => IrType::Unknown,
                "pow" | "sqrt" | "floor" | "ceil" | "log" | "log10" | "sin" | "cos" | "tan"
                | "exp" | "hypot" | "atan2" => IrType::Double,
                "round" => IrType::Long,
                "random" => IrType::Double,
                _ => IrType::Double,
            };
        }
        if name == "Optional" {
            return match method_name {
                "of" | "ofNullable" | "empty" => IrType::Class("Optional".to_owned()),
                _ => IrType::Unknown,
            };
        }
        if name == "Comparator" {
            return IrType::Class("Comparator".to_owned());
        }
        if name == "Pattern" {
            return match method_name {
                "compile" => IrType::Class("Pattern".to_owned()),
                "matches" => IrType::Bool,
                _ => IrType::Unknown,
            };
        }
        if name == "LocalDate" {
            return IrType::Class("LocalDate".to_owned());
        }
        if name == "BigInteger" {
            return IrType::Class("BigInteger".to_owned());
        }
        // Files static methods
        if name == "Files" {
            return match method_name {
                "readString" => IrType::String,
                "writeString" | "createDirectory" | "createDirectories" | "copy" | "move" => {
                    IrType::Class("Path".to_owned())
                }
                "readAllLines" => IrType::Unknown, // JList<JString>
                "write" => IrType::Class("Path".to_owned()),
                "exists" | "isDirectory" | "isRegularFile" | "deleteIfExists" => IrType::Bool,
                "size" => IrType::Long,
                "delete" => IrType::Void,
                _ => IrType::Unknown,
            };
        }
        // Paths.get
        if name == "Paths" {
            return match method_name {
                "get" => IrType::Class("Path".to_owned()),
                _ => IrType::Unknown,
            };
        }
    }

    // java.util.concurrent / Thread method return types
    if let Some(recv) = receiver {
        if let Some(class_name) = type_class_name(recv.ty()) {
            match class_name {
                "AtomicInteger" => match method_name {
                    "get" | "getAndIncrement" | "incrementAndGet" | "getAndDecrement"
                    | "decrementAndGet" | "getAndAdd" | "addAndGet" | "intValue" => {
                        return IrType::Int;
                    }
                    "set" => return IrType::Void,
                    "compareAndSet" => return IrType::Bool,
                    _ => {}
                },
                "AtomicLong" => match method_name {
                    "get" | "getAndIncrement" | "incrementAndGet" | "getAndDecrement"
                    | "decrementAndGet" | "getAndAdd" | "addAndGet" | "longValue" => {
                        return IrType::Long;
                    }
                    "set" => return IrType::Void,
                    "compareAndSet" => return IrType::Bool,
                    _ => {}
                },
                "AtomicBoolean" => match method_name {
                    "get" | "getAndSet" => return IrType::Bool,
                    "set" => return IrType::Void,
                    "compareAndSet" => return IrType::Bool,
                    _ => {}
                },
                "CountDownLatch" => match method_name {
                    "getCount" => return IrType::Long,
                    "countDown" | "await" => return IrType::Void,
                    _ => {}
                },
                "Semaphore" => match method_name {
                    "availablePermits" => return IrType::Int,
                    "acquire" | "release" => return IrType::Void,
                    _ => {}
                },
                "Thread" | "JThread" => match method_name {
                    "start" | "join" | "sleep" | "run" => return IrType::Void,
                    _ => {}
                },
                // JClass reflection methods
                "JClass" => match method_name {
                    "getName" | "getSimpleName" | "getCanonicalName" => return IrType::String,
                    _ => {}
                },
                // Optional methods
                "Optional" | "JOptional" => match method_name {
                    "isPresent" | "isEmpty" => return IrType::Bool,
                    "get" | "orElse" => return IrType::Unknown,
                    _ => {}
                },
                // Pattern/Matcher methods
                "Pattern" | "JPattern" => match method_name {
                    "matcher" => return IrType::Class("Matcher".to_owned()),
                    "matches" => return IrType::Bool,
                    _ => {}
                },
                "Matcher" | "JMatcher" => match method_name {
                    "matches" | "find" | "lookingAt" => return IrType::Bool,
                    "group" => return IrType::String,
                    _ => {}
                },
                // LocalDate methods
                "LocalDate" | "JLocalDate" => match method_name {
                    "getYear" | "getMonthValue" | "getDayOfMonth" | "getDayOfYear" => {
                        return IrType::Int
                    }
                    "plusDays" | "minusDays" | "plusMonths" | "minusMonths" | "withDayOfMonth" => {
                        return IrType::Class("LocalDate".to_owned())
                    }
                    "toString" => return IrType::String,
                    _ => {}
                },
                // BigInteger methods
                "BigInteger" | "JBigInteger" => match method_name {
                    "add" | "subtract" | "multiply" | "divide" | "mod" | "pow" | "abs"
                    | "negate" | "gcd" => return IrType::Class("BigInteger".to_owned()),
                    "toString" => return IrType::String,
                    "intValue" | "compareTo" => return IrType::Int,
                    "longValue" => return IrType::Long,
                    _ => {}
                },
                // StringBuilder methods
                "StringBuilder" | "JStringBuilder" => match method_name {
                    "toString" | "substring" => return IrType::String,
                    "length" | "indexOf" => return IrType::Int,
                    "charAt" => return IrType::Char,
                    "append" | "insert" | "delete" | "deleteCharAt" | "reverse" => {
                        return IrType::Class("StringBuilder".to_owned())
                    }
                    _ => {}
                },
                // Stream methods
                "JStream" => match method_name {
                    "count" => return IrType::Long,
                    "filter" | "sorted" | "distinct" | "limit" | "skip" => {
                        return IrType::Class("JStream".to_owned())
                    }
                    "collect_to_list" | "toArray" => return IrType::Unknown,
                    "findFirst" => return IrType::Unknown,
                    _ => {}
                },
                // ArrayList / List — stream() returns a JStream
                "ArrayList" | "List" if method_name == "stream" => {
                    return IrType::Class("JStream".to_owned());
                }
                // Comparator instance methods all return Comparator
                "Comparator" => match method_name {
                    "reversed"
                    | "thenComparing"
                    | "thenComparingInt"
                    | "thenComparingLong"
                    | "thenComparingDouble" => return IrType::Class("Comparator".to_owned()),
                    _ => {}
                },
                // File methods
                "File" | "JFile" => match method_name {
                    "getName" | "getPath" | "getAbsolutePath" | "getParent" | "toString" => {
                        return IrType::String
                    }
                    "exists" | "isFile" | "isDirectory" | "delete" | "mkdir" | "mkdirs" => {
                        return IrType::Bool
                    }
                    "length" => return IrType::Long,
                    "toPath" => return IrType::Class("Path".to_owned()),
                    _ => {}
                },
                // BufferedReader methods
                "BufferedReader" | "JBufferedReader" => match method_name {
                    "readLine" => return IrType::String,
                    "read" => return IrType::Int,
                    "ready" => return IrType::Bool,
                    "close" => return IrType::Void,
                    _ => {}
                },
                // BufferedWriter methods
                "BufferedWriter" | "JBufferedWriter" => match method_name {
                    "write" | "newLine" | "flush" | "close" => return IrType::Void,
                    _ => {}
                },
                // PrintWriter methods
                "PrintWriter" | "JPrintWriter" => match method_name {
                    "println" | "print" | "write" | "flush" | "close" => return IrType::Void,
                    "printf" => return IrType::Class(class_name.to_owned()),
                    _ => {}
                },
                // FileReader methods
                "FileReader" | "JFileReader" if method_name == "close" => {
                    return IrType::Void;
                }
                // FileWriter methods
                "FileWriter" | "JFileWriter" => match method_name {
                    "write" | "flush" | "close" => return IrType::Void,
                    _ => {}
                },
                // FileInputStream methods
                "FileInputStream" | "JFileInputStream" => match method_name {
                    "read" | "available" => return IrType::Int,
                    "close" => return IrType::Void,
                    _ => {}
                },
                // FileOutputStream methods
                "FileOutputStream" | "JFileOutputStream" => match method_name {
                    "flush" | "close" => return IrType::Void,
                    _ => {}
                },
                // Scanner methods
                "Scanner" | "JScanner" => match method_name {
                    "nextLine" | "next" => return IrType::String,
                    "nextInt" => return IrType::Int,
                    "nextDouble" => return IrType::Double,
                    "nextLong" => return IrType::Long,
                    "hasNextLine" | "hasNext" | "hasNextInt" => return IrType::Bool,
                    "close" => return IrType::Void,
                    _ => {}
                },
                // Path methods
                "Path" | "JPath" => match method_name {
                    "toString" => return IrType::String,
                    "toFile" => return IrType::Class("File".to_owned()),
                    "getFileName" | "getParent" | "resolve" | "toAbsolutePath" => {
                        return IrType::Class("Path".to_owned())
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
    // getClass() on any object → JClass
    if method_name == "getClass" {
        return IrType::Class("JClass".to_owned());
    }
    // String methods
    match method_name {
        "length" => return IrType::Int,
        "charAt" => return IrType::Char,
        "substring" | "toString" | "trim" | "toLowerCase" | "toUpperCase" | "concat"
        | "replace" | "valueOf" => return IrType::String,
        "equals" | "equalsIgnoreCase" | "contains" | "startsWith" | "endsWith" | "isEmpty"
        | "matches" => return IrType::Bool,
        "indexOf" | "lastIndexOf" | "compareTo" => return IrType::Int,
        "parseInt" => return IrType::Int,
        "parseLong" => return IrType::Long,
        "parseDouble" => return IrType::Double,
        "parseFloat" => return IrType::Float,
        _ => {}
    }

    // Universal Object methods
    match method_name {
        "hashCode" => return IrType::Int,
        "equals" => return IrType::Bool,
        _ => {}
    }

    // Collection methods (JList / JMap / JSet / JLinkedList / JTreeMap / JTreeSet / JPriorityQueue)
    match method_name {
        "size" => return IrType::Int,
        "isEmpty" | "containsKey" | "containsValue" | "contains" | "add" | "remove" | "offer" => {
            return IrType::Bool
        }
        "clear" | "put" | "sort" | "reverse" | "addFirst" | "addLast" | "push" => {
            return IrType::Void
        }
        "get" | "getFirst" | "getLast" | "peek" | "poll" | "pop" | "removeFirst" | "removeLast"
        | "first" | "last" | "firstKey" | "lastKey" => return IrType::Unknown,
        "iterator" => return IrType::Class("JIterator".to_owned()),
        _ => {}
    }

    // Exception methods (JException)
    match method_name {
        "getMessage" | "toString" => return IrType::String,
        "getClassName" => return IrType::String,
        _ => {}
    }

    // Unqualified call (no receiver) → look in same class first, then traversing
    // its superclass chain.
    if receiver.is_none() {
        if let Some(ty) = lookup_method_return_type(method_name, cls, class_map) {
            return ty;
        }
    }

    // Qualified call — look in the receiver's class.
    if let Some(recv) = receiver {
        let recv_ty = recv.ty();
        if let Some(class_name) = type_class_name(recv_ty) {
            if let Some(recv_cls) = class_map.get(class_name) {
                if let Some(ty) = lookup_method_return_type(method_name, recv_cls, class_map) {
                    return ty;
                }
            }
        }
    }

    IrType::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;
    use ir::decl::*;
    use ir::expr::BinOp as IrBinOp;
    use ir::expr::UnOp as IrUnOp;
    use ir::stmt::{CatchClause, SwitchCase};
    use ir::{IrExpr, IrModule, IrStmt, IrType};

    /// Helper: build a minimal class with a single static main method.
    fn make_class(name: &str, body: Vec<IrStmt>) -> IrClass {
        IrClass {
            name: name.into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            type_params: vec![],
            superclass: None,
            interfaces: vec![],
            fields: vec![],
            methods: vec![IrMethod {
                name: "main".into(),
                visibility: Visibility::Public,
                is_static: true,
                is_abstract: false,
                is_final: false,
                is_synchronized: false,
                type_params: vec![],
                params: vec![IrParam {
                    name: "args".into(),
                    ty: IrType::Array(Box::new(IrType::String)),
                    is_varargs: false,
                }],
                return_ty: IrType::Void,
                body: Some(body),
                throws: vec![],
                is_default: false,
            }],
            constructors: vec![],
            is_record: false,
            captures: vec![],
        }
    }

    fn tc(module: IrModule) -> IrModule {
        type_check(module).expect("type_check should succeed")
    }

    #[test]
    fn check_simple_module() {
        let module = IrModule::new("test");
        let result = type_check(module);
        assert!(result.is_ok());
    }

    #[test]
    fn check_empty_class() {
        let mut module = IrModule::new("");
        module
            .decls
            .push(IrDecl::Class(make_class("Empty", vec![])));
        tc(module);
    }

    #[test]
    fn check_local_var_with_literal() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "x".into(),
            ty: IrType::Int,
            init: Some(IrExpr::LitInt(42)),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("LocalTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_arithmetic_typing() {
        let mut module = IrModule::new("");
        let init = IrStmt::LocalVar {
            name: "x".into(),
            ty: IrType::Int,
            init: Some(IrExpr::BinOp {
                op: IrBinOp::Add,
                lhs: Box::new(IrExpr::LitInt(1)),
                rhs: Box::new(IrExpr::LitInt(2)),
                ty: IrType::Unknown,
            }),
        };
        module
            .decls
            .push(IrDecl::Class(make_class("ArithTyping", vec![init])));
        let result = tc(module);
        // After type checking, the BinOp should be typed as Int
        if let IrDecl::Class(cls) = &result.decls[0] {
            if let Some(body) = &cls.methods[0].body {
                if let IrStmt::LocalVar {
                    init: Some(expr), ..
                } = &body[0]
                {
                    assert_eq!(*expr.ty(), IrType::Int, "Add of ints should be Int");
                }
            }
        }
    }

    #[test]
    fn check_string_concat_typing() {
        let mut module = IrModule::new("");
        let init = IrStmt::LocalVar {
            name: "s".into(),
            ty: IrType::String,
            init: Some(IrExpr::BinOp {
                op: IrBinOp::Concat,
                lhs: Box::new(IrExpr::LitString("a".into())),
                rhs: Box::new(IrExpr::LitString("b".into())),
                ty: IrType::Unknown,
            }),
        };
        module
            .decls
            .push(IrDecl::Class(make_class("ConcatTyping", vec![init])));
        let result = tc(module);
        if let IrDecl::Class(cls) = &result.decls[0] {
            if let Some(body) = &cls.methods[0].body {
                if let IrStmt::LocalVar {
                    init: Some(expr), ..
                } = &body[0]
                {
                    assert_eq!(*expr.ty(), IrType::String, "Concat should be String");
                }
            }
        }
    }

    #[test]
    fn check_comparison_typing() {
        let mut module = IrModule::new("");
        let init = IrStmt::LocalVar {
            name: "b".into(),
            ty: IrType::Bool,
            init: Some(IrExpr::BinOp {
                op: IrBinOp::Lt,
                lhs: Box::new(IrExpr::LitInt(1)),
                rhs: Box::new(IrExpr::LitInt(2)),
                ty: IrType::Unknown,
            }),
        };
        module
            .decls
            .push(IrDecl::Class(make_class("CompTyping", vec![init])));
        let result = tc(module);
        if let IrDecl::Class(cls) = &result.decls[0] {
            if let Some(body) = &cls.methods[0].body {
                if let IrStmt::LocalVar {
                    init: Some(expr), ..
                } = &body[0]
                {
                    assert_eq!(*expr.ty(), IrType::Bool, "Comparison should be Bool");
                }
            }
        }
    }

    #[test]
    fn check_if_statement() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::If {
            cond: IrExpr::LitBool(true),
            then_: vec![IrStmt::Return(None)],
            else_: Some(vec![IrStmt::Return(None)]),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("IfTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_while_loop() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::While {
            cond: IrExpr::LitBool(false),
            body: vec![IrStmt::Break(None)],
            label: None,
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("WhileTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_for_loop() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::For {
            init: Some(Box::new(IrStmt::LocalVar {
                name: "i".into(),
                ty: IrType::Int,
                init: Some(IrExpr::LitInt(0)),
            })),
            cond: Some(IrExpr::BinOp {
                op: IrBinOp::Lt,
                lhs: Box::new(IrExpr::Var {
                    name: "i".into(),
                    ty: IrType::Int,
                }),
                rhs: Box::new(IrExpr::LitInt(10)),
                ty: IrType::Unknown,
            }),
            update: vec![IrExpr::UnOp {
                op: IrUnOp::PostInc,
                operand: Box::new(IrExpr::Var {
                    name: "i".into(),
                    ty: IrType::Int,
                }),
                ty: IrType::Unknown,
            }],
            body: vec![],
            label: None,
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("ForTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_do_while() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::DoWhile {
            body: vec![IrStmt::Continue(None)],
            cond: IrExpr::LitBool(false),
            label: None,
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("DoWhileTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_switch() {
        let mut module = IrModule::new("");
        let init = IrStmt::LocalVar {
            name: "x".into(),
            ty: IrType::Int,
            init: Some(IrExpr::LitInt(1)),
        };
        let switch = IrStmt::Switch {
            expr: IrExpr::Var {
                name: "x".into(),
                ty: IrType::Int,
            },
            cases: vec![SwitchCase {
                value: IrExpr::LitInt(1),
                body: vec![IrStmt::Break(None)],
            }],
            default: Some(vec![IrStmt::Break(None)]),
        };
        module
            .decls
            .push(IrDecl::Class(make_class("SwitchTest", vec![init, switch])));
        tc(module);
    }

    #[test]
    fn check_try_catch() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::TryCatch {
            body: vec![],
            catches: vec![CatchClause {
                exception_types: vec!["Exception".into()],
                var: "e".into(),
                body: vec![],
            }],
            finally: Some(vec![]),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("TryTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_constructor() {
        let mut module = IrModule::new("");
        let mut cls = make_class("WithCtor", vec![]);
        cls.fields.push(IrField {
            name: "val".into(),
            ty: IrType::Int,
            visibility: Visibility::Public,
            is_static: false,
            is_final: false,
            is_volatile: false,
            init: None,
        });
        cls.constructors.push(IrConstructor {
            visibility: Visibility::Public,
            params: vec![IrParam {
                name: "val".into(),
                ty: IrType::Int,
                is_varargs: false,
            }],
            body: vec![],
            throws: vec![],
        });
        module.decls.push(IrDecl::Class(cls));
        tc(module);
    }

    #[test]
    fn check_class_with_instance_method() {
        let mut module = IrModule::new("");
        let mut cls = make_class("Inst", vec![]);
        cls.fields.push(IrField {
            name: "count".into(),
            ty: IrType::Int,
            visibility: Visibility::Public,
            is_static: false,
            is_final: false,
            is_volatile: false,
            init: None,
        });
        cls.methods.push(IrMethod {
            name: "getCount".into(),
            visibility: Visibility::Public,
            is_static: false,
            is_abstract: false,
            is_final: false,
            is_synchronized: false,
            type_params: vec![],
            params: vec![],
            return_ty: IrType::Int,
            body: Some(vec![IrStmt::Return(Some(IrExpr::FieldAccess {
                receiver: Box::new(IrExpr::Var {
                    name: "this".into(),
                    ty: IrType::Class("Inst".into()),
                }),
                field_name: "count".into(),
                ty: IrType::Unknown,
            }))]),
            throws: vec![],
            is_default: false,
        });
        module.decls.push(IrDecl::Class(cls));
        tc(module);
    }

    #[test]
    fn check_method_call_typing() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::Expr(IrExpr::MethodCall {
            receiver: None,
            method_name: "System.out.println".into(),
            args: vec![IrExpr::LitString("hello".into())],
            ty: IrType::Unknown,
        })];
        module
            .decls
            .push(IrDecl::Class(make_class("CallTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_ternary() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "x".into(),
            ty: IrType::Int,
            init: Some(IrExpr::Ternary {
                cond: Box::new(IrExpr::LitBool(true)),
                then_: Box::new(IrExpr::LitInt(1)),
                else_: Box::new(IrExpr::LitInt(2)),
                ty: IrType::Unknown,
            }),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("TernaryTest", stmts)));
        let result = tc(module);
        if let IrDecl::Class(cls) = &result.decls[0] {
            if let Some(body) = &cls.methods[0].body {
                if let IrStmt::LocalVar {
                    init: Some(expr), ..
                } = &body[0]
                {
                    assert_eq!(*expr.ty(), IrType::Int, "Ternary should infer Int");
                }
            }
        }
    }

    #[test]
    fn check_field_initialiser() {
        let mut module = IrModule::new("");
        let mut cls = make_class("FieldInit", vec![]);
        cls.fields.push(IrField {
            name: "value".into(),
            ty: IrType::Int,
            visibility: Visibility::Public,
            is_static: true,
            is_final: true,
            is_volatile: false,
            init: Some(IrExpr::BinOp {
                op: IrBinOp::Add,
                lhs: Box::new(IrExpr::LitInt(1)),
                rhs: Box::new(IrExpr::LitInt(2)),
                ty: IrType::Unknown,
            }),
        });
        module.decls.push(IrDecl::Class(cls));
        let result = tc(module);
        if let IrDecl::Class(cls) = &result.decls[0] {
            if let Some(init) = &cls.fields[0].init {
                assert_eq!(*init.ty(), IrType::Int, "Field init should be typed");
            }
        }
    }

    #[test]
    fn check_cast_expression() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "d".into(),
            ty: IrType::Double,
            init: Some(IrExpr::Cast {
                target: IrType::Double,
                expr: Box::new(IrExpr::LitInt(42)),
            }),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("CastTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_unary_ops() {
        let mut module = IrModule::new("");
        let stmts = vec![
            IrStmt::LocalVar {
                name: "x".into(),
                ty: IrType::Int,
                init: Some(IrExpr::UnOp {
                    op: IrUnOp::Neg,
                    operand: Box::new(IrExpr::LitInt(5)),
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "b".into(),
                ty: IrType::Bool,
                init: Some(IrExpr::UnOp {
                    op: IrUnOp::Not,
                    operand: Box::new(IrExpr::LitBool(true)),
                    ty: IrType::Unknown,
                }),
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("UnaryTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_new_expression() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "list".into(),
            ty: IrType::Class("ArrayList".into()),
            init: Some(IrExpr::New {
                class: "ArrayList".into(),
                args: vec![],
                ty: IrType::Unknown,
            }),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("NewTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_array_access() {
        let mut module = IrModule::new("");
        let stmts = vec![
            IrStmt::LocalVar {
                name: "arr".into(),
                ty: IrType::Array(Box::new(IrType::Int)),
                init: Some(IrExpr::NewArray {
                    elem_ty: IrType::Int,
                    len: Box::new(IrExpr::LitInt(5)),
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "val".into(),
                ty: IrType::Int,
                init: Some(IrExpr::ArrayAccess {
                    array: Box::new(IrExpr::Var {
                        name: "arr".into(),
                        ty: IrType::Array(Box::new(IrType::Int)),
                    }),
                    index: Box::new(IrExpr::LitInt(0)),
                    ty: IrType::Unknown,
                }),
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("ArrayTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_block_scoping() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::Block(vec![IrStmt::LocalVar {
            name: "inner".into(),
            ty: IrType::Int,
            init: Some(IrExpr::LitInt(1)),
        }])];
        module
            .decls
            .push(IrDecl::Class(make_class("BlockTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_throw() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::Throw(IrExpr::New {
            class: "RuntimeException".into(),
            args: vec![IrExpr::LitString("err".into())],
            ty: IrType::Class("RuntimeException".into()),
        })];
        module
            .decls
            .push(IrDecl::Class(make_class("ThrowTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_for_each() {
        let mut module = IrModule::new("");
        let stmts = vec![
            IrStmt::LocalVar {
                name: "list".into(),
                ty: IrType::Class("ArrayList".into()),
                init: Some(IrExpr::New {
                    class: "ArrayList".into(),
                    args: vec![],
                    ty: IrType::Class("ArrayList".into()),
                }),
            },
            IrStmt::ForEach {
                var: "x".into(),
                var_ty: IrType::Int,
                iterable: IrExpr::Var {
                    name: "list".into(),
                    ty: IrType::Class("ArrayList".into()),
                },
                body: vec![],
                label: None,
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("ForEachTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_assign() {
        let mut module = IrModule::new("");
        let stmts = vec![
            IrStmt::LocalVar {
                name: "x".into(),
                ty: IrType::Int,
                init: Some(IrExpr::LitInt(0)),
            },
            IrStmt::Expr(IrExpr::Assign {
                lhs: Box::new(IrExpr::Var {
                    name: "x".into(),
                    ty: IrType::Int,
                }),
                rhs: Box::new(IrExpr::LitInt(42)),
                ty: IrType::Unknown,
            }),
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("AssignTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_compound_assign() {
        let mut module = IrModule::new("");
        let stmts = vec![
            IrStmt::LocalVar {
                name: "x".into(),
                ty: IrType::Int,
                init: Some(IrExpr::LitInt(10)),
            },
            IrStmt::Expr(IrExpr::CompoundAssign {
                op: IrBinOp::Add,
                lhs: Box::new(IrExpr::Var {
                    name: "x".into(),
                    ty: IrType::Int,
                }),
                rhs: Box::new(IrExpr::LitInt(5)),
                ty: IrType::Unknown,
            }),
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("CompoundTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_instanceof() {
        let mut module = IrModule::new("");
        let stmts = vec![
            IrStmt::LocalVar {
                name: "obj".into(),
                ty: IrType::Class("Object".into()),
                init: None,
            },
            IrStmt::If {
                cond: IrExpr::InstanceOf {
                    expr: Box::new(IrExpr::Var {
                        name: "obj".into(),
                        ty: IrType::Class("Object".into()),
                    }),
                    check_type: IrType::Class("String".into()),
                    binding: None,
                },
                then_: vec![],
                else_: None,
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("InstanceOfTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_lambda() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "f".into(),
            ty: IrType::Class("Function".into()),
            init: Some(IrExpr::Lambda {
                params: vec!["x".into()],
                body: Box::new(IrExpr::LitInt(1)),
                body_stmts: vec![],
                ty: IrType::Class("Function".into()),
            }),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("LambdaTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_return_with_value() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::Return(Some(IrExpr::LitInt(42)))];
        module
            .decls
            .push(IrDecl::Class(make_class("ReturnTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_interface_skipped() {
        let mut module = IrModule::new("");
        module.decls.push(IrDecl::Interface(IrInterface {
            name: "Runnable".into(),
            visibility: Visibility::Public,
            type_params: vec![],
            extends: vec![],
            methods: vec![IrMethod {
                name: "run".into(),
                visibility: Visibility::Public,
                is_static: false,
                is_abstract: true,
                is_final: false,
                is_synchronized: false,
                type_params: vec![],
                params: vec![],
                return_ty: IrType::Void,
                body: None,
                throws: vec![],
                is_default: false,
            }],
        }));
        tc(module);
    }

    // ── resolve_method_return_type branches ───────────────────────────────

    #[test]
    fn check_math_static_methods() {
        let mut module = IrModule::new("");
        let stmts = vec![
            IrStmt::LocalVar {
                name: "a".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "Math".into(),
                        ty: IrType::Class("Math".into()),
                    })),
                    method_name: "abs".into(),
                    args: vec![IrExpr::LitInt(-5)],
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "b".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "Math".into(),
                        ty: IrType::Class("Math".into()),
                    })),
                    method_name: "sqrt".into(),
                    args: vec![IrExpr::LitDouble(4.0)],
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "c".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "Math".into(),
                        ty: IrType::Class("Math".into()),
                    })),
                    method_name: "random".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "d".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "Math".into(),
                        ty: IrType::Class("Math".into()),
                    })),
                    method_name: "round".into(),
                    args: vec![IrExpr::LitDouble(1.5)],
                    ty: IrType::Unknown,
                }),
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("MathTest", stmts)));
        let checked = tc(module);
        // sqrt should type to Double
        if let IrDecl::Class(cls) = &checked.decls[0] {
            if let IrStmt::LocalVar { init: Some(e), .. } =
                &cls.methods[0].body.as_ref().unwrap()[1]
            {
                assert_eq!(*e.ty(), IrType::Double, "Math.sqrt should return Double");
            }
            // round should return Long
            if let IrStmt::LocalVar { init: Some(e), .. } =
                &cls.methods[0].body.as_ref().unwrap()[3]
            {
                assert_eq!(*e.ty(), IrType::Long, "Math.round should return Long");
            }
        }
    }

    #[test]
    fn check_optional_static() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "opt".into(),
            ty: IrType::Unknown,
            init: Some(IrExpr::MethodCall {
                receiver: Some(Box::new(IrExpr::Var {
                    name: "Optional".into(),
                    ty: IrType::Class("Optional".into()),
                })),
                method_name: "of".into(),
                args: vec![IrExpr::LitInt(1)],
                ty: IrType::Unknown,
            }),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("OptTest", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            if let IrStmt::LocalVar { init: Some(e), .. } =
                &cls.methods[0].body.as_ref().unwrap()[0]
            {
                assert_eq!(*e.ty(), IrType::Class("Optional".into()));
            }
        }
    }

    #[test]
    fn check_pattern_compile() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "p".into(),
            ty: IrType::Unknown,
            init: Some(IrExpr::MethodCall {
                receiver: Some(Box::new(IrExpr::Var {
                    name: "Pattern".into(),
                    ty: IrType::Class("Pattern".into()),
                })),
                method_name: "compile".into(),
                args: vec![IrExpr::LitString("\\d+".into())],
                ty: IrType::Unknown,
            }),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("PatternTest", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            if let IrStmt::LocalVar { init: Some(e), .. } =
                &cls.methods[0].body.as_ref().unwrap()[0]
            {
                assert_eq!(*e.ty(), IrType::Class("Pattern".into()));
            }
        }
    }

    #[test]
    fn check_atomic_integer_methods() {
        let mut module = IrModule::new("");
        let stmts = vec![
            IrStmt::LocalVar {
                name: "ai".into(),
                ty: IrType::Class("AtomicInteger".into()),
                init: Some(IrExpr::New {
                    class: "AtomicInteger".into(),
                    args: vec![IrExpr::LitInt(0)],
                    ty: IrType::Class("AtomicInteger".into()),
                }),
            },
            IrStmt::LocalVar {
                name: "v".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "ai".into(),
                        ty: IrType::Class("AtomicInteger".into()),
                    })),
                    method_name: "get".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "b".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "ai".into(),
                        ty: IrType::Class("AtomicInteger".into()),
                    })),
                    method_name: "compareAndSet".into(),
                    args: vec![IrExpr::LitInt(0), IrExpr::LitInt(1)],
                    ty: IrType::Unknown,
                }),
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("AtomicTest", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            let body = cls.methods[0].body.as_ref().unwrap();
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[1] {
                assert_eq!(*e.ty(), IrType::Int, "AtomicInteger.get should return Int");
            }
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[2] {
                assert_eq!(*e.ty(), IrType::Bool, "compareAndSet should return Bool");
            }
        }
    }

    #[test]
    fn check_string_methods_return_types() {
        let mut module = IrModule::new("");
        let str_var = Box::new(IrExpr::Var {
            name: "s".into(),
            ty: IrType::String,
        });
        let stmts = vec![
            IrStmt::LocalVar {
                name: "s".into(),
                ty: IrType::String,
                init: Some(IrExpr::LitString("hello".into())),
            },
            IrStmt::LocalVar {
                name: "len".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(str_var.clone()),
                    method_name: "length".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "sub".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(str_var.clone()),
                    method_name: "substring".into(),
                    args: vec![IrExpr::LitInt(0), IrExpr::LitInt(3)],
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "eq".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(str_var.clone()),
                    method_name: "equals".into(),
                    args: vec![IrExpr::LitString("hello".into())],
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "idx".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(str_var.clone()),
                    method_name: "indexOf".into(),
                    args: vec![IrExpr::LitString("l".into())],
                    ty: IrType::Unknown,
                }),
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("StrMethodTest", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            let body = cls.methods[0].body.as_ref().unwrap();
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[1] {
                assert_eq!(*e.ty(), IrType::Int, "String.length should return Int");
            }
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[2] {
                assert_eq!(*e.ty(), IrType::String, "substring should return String");
            }
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[3] {
                assert_eq!(*e.ty(), IrType::Bool, "equals should return Bool");
            }
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[4] {
                assert_eq!(*e.ty(), IrType::Int, "indexOf should return Int");
            }
        }
    }

    #[test]
    fn check_collection_method_types() {
        let mut module = IrModule::new("");
        let list_var = Box::new(IrExpr::Var {
            name: "list".into(),
            ty: IrType::Class("ArrayList".into()),
        });
        let stmts = vec![
            IrStmt::LocalVar {
                name: "list".into(),
                ty: IrType::Class("ArrayList".into()),
                init: Some(IrExpr::New {
                    class: "ArrayList".into(),
                    args: vec![],
                    ty: IrType::Class("ArrayList".into()),
                }),
            },
            IrStmt::LocalVar {
                name: "sz".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(list_var.clone()),
                    method_name: "size".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "empty".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(list_var.clone()),
                    method_name: "isEmpty".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("CollTest", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            let body = cls.methods[0].body.as_ref().unwrap();
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[1] {
                assert_eq!(*e.ty(), IrType::Int, "size should return Int");
            }
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[2] {
                assert_eq!(*e.ty(), IrType::Bool, "isEmpty should return Bool");
            }
        }
    }

    #[test]
    fn check_getclass_returns_jclass() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "cls".into(),
            ty: IrType::Unknown,
            init: Some(IrExpr::MethodCall {
                receiver: Some(Box::new(IrExpr::Var {
                    name: "obj".into(),
                    ty: IrType::Class("Object".into()),
                })),
                method_name: "getClass".into(),
                args: vec![],
                ty: IrType::Unknown,
            }),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("GetClassTest", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            if let IrStmt::LocalVar { init: Some(e), .. } =
                &cls.methods[0].body.as_ref().unwrap()[0]
            {
                assert_eq!(*e.ty(), IrType::Class("JClass".into()));
            }
        }
    }

    #[test]
    fn check_hashcode_returns_int() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "h".into(),
            ty: IrType::Unknown,
            init: Some(IrExpr::MethodCall {
                receiver: Some(Box::new(IrExpr::Var {
                    name: "obj".into(),
                    ty: IrType::Class("Object".into()),
                })),
                method_name: "hashCode".into(),
                args: vec![],
                ty: IrType::Unknown,
            }),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("HashTest", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            if let IrStmt::LocalVar { init: Some(e), .. } =
                &cls.methods[0].body.as_ref().unwrap()[0]
            {
                assert_eq!(*e.ty(), IrType::Int, "hashCode should return Int");
            }
        }
    }

    #[test]
    fn check_resolve_field_array_length() {
        let mut module = IrModule::new("");
        let stmts = vec![
            IrStmt::LocalVar {
                name: "arr".into(),
                ty: IrType::Array(Box::new(IrType::Int)),
                init: Some(IrExpr::NewArray {
                    elem_ty: IrType::Int,
                    len: Box::new(IrExpr::LitInt(5)),
                    ty: IrType::Array(Box::new(IrType::Int)),
                }),
            },
            IrStmt::LocalVar {
                name: "len".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::FieldAccess {
                    receiver: Box::new(IrExpr::Var {
                        name: "arr".into(),
                        ty: IrType::Array(Box::new(IrType::Int)),
                    }),
                    field_name: "length".into(),
                    ty: IrType::Unknown,
                }),
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("ArrLenTest", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            if let IrStmt::LocalVar { init: Some(e), .. } =
                &cls.methods[0].body.as_ref().unwrap()[1]
            {
                assert_eq!(*e.ty(), IrType::Int, "array.length should be Int");
            }
        }
    }

    #[test]
    fn check_system_out_field() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "out".into(),
            ty: IrType::Unknown,
            init: Some(IrExpr::FieldAccess {
                receiver: Box::new(IrExpr::Var {
                    name: "System".into(),
                    ty: IrType::Class("System".into()),
                }),
                field_name: "out".into(),
                ty: IrType::Unknown,
            }),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("SysOutTest", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            if let IrStmt::LocalVar { init: Some(e), .. } =
                &cls.methods[0].body.as_ref().unwrap()[0]
            {
                assert_eq!(*e.ty(), IrType::Class("PrintStream".into()));
            }
        }
    }

    #[test]
    fn check_widen_numeric_double() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "x".into(),
            ty: IrType::Unknown,
            init: Some(IrExpr::BinOp {
                op: ir::expr::BinOp::Add,
                lhs: Box::new(IrExpr::LitInt(1)),
                rhs: Box::new(IrExpr::LitDouble(2.0)),
                ty: IrType::Unknown,
            }),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("WidenTest", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            if let IrStmt::LocalVar { init: Some(e), .. } =
                &cls.methods[0].body.as_ref().unwrap()[0]
            {
                assert_eq!(
                    *e.ty(),
                    IrType::Double,
                    "int + double should widen to Double"
                );
            }
        }
    }

    #[test]
    fn check_widen_numeric_long() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "x".into(),
            ty: IrType::Unknown,
            init: Some(IrExpr::BinOp {
                op: ir::expr::BinOp::Mul,
                lhs: Box::new(IrExpr::LitInt(1)),
                rhs: Box::new(IrExpr::LitLong(2)),
                ty: IrType::Unknown,
            }),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("LongWidenTest", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            if let IrStmt::LocalVar { init: Some(e), .. } =
                &cls.methods[0].body.as_ref().unwrap()[0]
            {
                assert_eq!(*e.ty(), IrType::Long, "int * long should widen to Long");
            }
        }
    }

    #[test]
    fn check_concat_typing() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "x".into(),
            ty: IrType::Unknown,
            init: Some(IrExpr::BinOp {
                op: ir::expr::BinOp::Concat,
                lhs: Box::new(IrExpr::LitString("a".into())),
                rhs: Box::new(IrExpr::LitString("b".into())),
                ty: IrType::Unknown,
            }),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("ConcatType", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            if let IrStmt::LocalVar { init: Some(e), .. } =
                &cls.methods[0].body.as_ref().unwrap()[0]
            {
                assert_eq!(*e.ty(), IrType::String, "concat should be String");
            }
        }
    }

    #[test]
    fn check_thread_method_types() {
        let mut module = IrModule::new("");
        let stmts = vec![
            IrStmt::LocalVar {
                name: "t".into(),
                ty: IrType::Class("Thread".into()),
                init: Some(IrExpr::New {
                    class: "Thread".into(),
                    args: vec![],
                    ty: IrType::Class("Thread".into()),
                }),
            },
            IrStmt::Expr(IrExpr::MethodCall {
                receiver: Some(Box::new(IrExpr::Var {
                    name: "t".into(),
                    ty: IrType::Class("Thread".into()),
                })),
                method_name: "start".into(),
                args: vec![],
                ty: IrType::Unknown,
            }),
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("ThreadTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_countdown_latch_types() {
        let mut module = IrModule::new("");
        let stmts = vec![
            IrStmt::LocalVar {
                name: "latch".into(),
                ty: IrType::Class("CountDownLatch".into()),
                init: Some(IrExpr::New {
                    class: "CountDownLatch".into(),
                    args: vec![IrExpr::LitInt(1)],
                    ty: IrType::Class("CountDownLatch".into()),
                }),
            },
            IrStmt::LocalVar {
                name: "cnt".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "latch".into(),
                        ty: IrType::Class("CountDownLatch".into()),
                    })),
                    method_name: "getCount".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("LatchTest", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            if let IrStmt::LocalVar { init: Some(e), .. } =
                &cls.methods[0].body.as_ref().unwrap()[1]
            {
                assert_eq!(*e.ty(), IrType::Long, "getCount should return Long");
            }
        }
    }

    #[test]
    fn check_stringbuilder_methods() {
        let mut module = IrModule::new("");
        let sb_var = Box::new(IrExpr::Var {
            name: "sb".into(),
            ty: IrType::Class("StringBuilder".into()),
        });
        let stmts = vec![
            IrStmt::LocalVar {
                name: "sb".into(),
                ty: IrType::Class("StringBuilder".into()),
                init: Some(IrExpr::New {
                    class: "StringBuilder".into(),
                    args: vec![],
                    ty: IrType::Class("StringBuilder".into()),
                }),
            },
            IrStmt::LocalVar {
                name: "s".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(sb_var.clone()),
                    method_name: "toString".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "n".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(sb_var.clone()),
                    method_name: "length".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("SBTest", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            let body = cls.methods[0].body.as_ref().unwrap();
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[1] {
                assert_eq!(*e.ty(), IrType::String, "SB.toString should be String");
            }
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[2] {
                assert_eq!(*e.ty(), IrType::Int, "SB.length should be Int");
            }
        }
    }

    #[test]
    fn check_biginteger_methods() {
        let mut module = IrModule::new("");
        let big_var = Box::new(IrExpr::Var {
            name: "bi".into(),
            ty: IrType::Class("BigInteger".into()),
        });
        let stmts = vec![
            IrStmt::LocalVar {
                name: "bi".into(),
                ty: IrType::Class("BigInteger".into()),
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "BigInteger".into(),
                        ty: IrType::Class("BigInteger".into()),
                    })),
                    method_name: "valueOf".into(),
                    args: vec![IrExpr::LitLong(42)],
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "sum".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(big_var.clone()),
                    method_name: "add".into(),
                    args: vec![IrExpr::Var {
                        name: "bi".into(),
                        ty: IrType::Class("BigInteger".into()),
                    }],
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "s".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(big_var.clone()),
                    method_name: "toString".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("BigIntTest", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            let body = cls.methods[0].body.as_ref().unwrap();
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[1] {
                assert_eq!(
                    *e.ty(),
                    IrType::Class("BigInteger".into()),
                    "add should return BigInteger"
                );
            }
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[2] {
                assert_eq!(*e.ty(), IrType::String, "toString should return String");
            }
        }
    }

    #[test]
    fn check_exception_methods() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::TryCatch {
            body: vec![IrStmt::Throw(IrExpr::New {
                class: "Exception".into(),
                args: vec![IrExpr::LitString("err".into())],
                ty: IrType::Class("Exception".into()),
            })],
            catches: vec![ir::stmt::CatchClause {
                exception_types: vec!["Exception".into()],
                var: "e".into(),
                body: vec![IrStmt::LocalVar {
                    name: "msg".into(),
                    ty: IrType::Unknown,
                    init: Some(IrExpr::MethodCall {
                        receiver: Some(Box::new(IrExpr::Var {
                            name: "e".into(),
                            ty: IrType::Class("Exception".into()),
                        })),
                        method_name: "getMessage".into(),
                        args: vec![],
                        ty: IrType::Unknown,
                    }),
                }],
            }],
            finally: None,
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("ExcTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_super_constructor_call() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::SuperConstructorCall {
            args: vec![IrExpr::LitInt(1)],
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("SuperCallTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_synchronized_stmt() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::Synchronized {
            monitor: IrExpr::Var {
                name: "lock".into(),
                ty: IrType::Class("Object".into()),
            },
            body: vec![IrStmt::Expr(IrExpr::LitInt(1))],
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("SyncTest", stmts)));
        tc(module);
    }

    #[test]
    fn check_class_field_resolve() {
        let mut module = IrModule::new("");
        let mut cls = make_class("Pt", vec![]);
        cls.fields.push(IrField {
            name: "x".into(),
            ty: IrType::Int,
            visibility: Visibility::Public,
            is_static: false,
            is_final: false,
            is_volatile: false,
            init: None,
        });
        cls.methods[0].body = Some(vec![IrStmt::LocalVar {
            name: "v".into(),
            ty: IrType::Unknown,
            init: Some(IrExpr::FieldAccess {
                receiver: Box::new(IrExpr::Var {
                    name: "Pt".into(),
                    ty: IrType::Class("Pt".into()),
                }),
                field_name: "x".into(),
                ty: IrType::Unknown,
            }),
        }]);
        module.decls.push(IrDecl::Class(cls));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            if let IrStmt::LocalVar { init: Some(e), .. } =
                &cls.methods[0].body.as_ref().unwrap()[0]
            {
                assert_eq!(*e.ty(), IrType::Int, "field x should resolve to Int");
            }
        }
    }

    #[test]
    fn check_local_date_methods() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "d".into(),
            ty: IrType::Unknown,
            init: Some(IrExpr::MethodCall {
                receiver: Some(Box::new(IrExpr::Var {
                    name: "LocalDate".into(),
                    ty: IrType::Class("LocalDate".into()),
                })),
                method_name: "now".into(),
                args: vec![],
                ty: IrType::Unknown,
            }),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("DateTest", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            if let IrStmt::LocalVar { init: Some(e), .. } =
                &cls.methods[0].body.as_ref().unwrap()[0]
            {
                assert_eq!(*e.ty(), IrType::Class("LocalDate".into()));
            }
        }
    }

    #[test]
    fn check_parse_methods() {
        let mut module = IrModule::new("");
        let stmts = vec![
            IrStmt::LocalVar {
                name: "i".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: None,
                    method_name: "parseInt".into(),
                    args: vec![IrExpr::LitString("1".into())],
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "d".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: None,
                    method_name: "parseDouble".into(),
                    args: vec![IrExpr::LitString("1.0".into())],
                    ty: IrType::Unknown,
                }),
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("ParseTest", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            let body = cls.methods[0].body.as_ref().unwrap();
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[0] {
                assert_eq!(*e.ty(), IrType::Int, "parseInt should return Int");
            }
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[1] {
                assert_eq!(*e.ty(), IrType::Double, "parseDouble should return Double");
            }
        }
    }

    // ── AtomicLong method types ───────────────────────────────────────────

    #[test]
    fn check_atomic_long_methods() {
        let mut module = IrModule::new("");
        let stmts = vec![
            IrStmt::LocalVar {
                name: "al".into(),
                ty: IrType::Class("AtomicLong".into()),
                init: None,
            },
            IrStmt::LocalVar {
                name: "v".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "al".into(),
                        ty: IrType::Class("AtomicLong".into()),
                    })),
                    method_name: "get".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "v2".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "al".into(),
                        ty: IrType::Class("AtomicLong".into()),
                    })),
                    method_name: "incrementAndGet".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("AtomLong", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            let body = cls.methods[0].body.as_ref().unwrap();
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[1] {
                assert_eq!(*e.ty(), IrType::Long, "AtomicLong.get should return Long");
            }
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[2] {
                assert_eq!(
                    *e.ty(),
                    IrType::Long,
                    "AtomicLong.incrementAndGet should return Long"
                );
            }
        }
    }

    // ── AtomicBoolean method types ────────────────────────────────────────

    #[test]
    fn check_atomic_boolean_methods() {
        let mut module = IrModule::new("");
        let stmts = vec![
            IrStmt::LocalVar {
                name: "ab".into(),
                ty: IrType::Class("AtomicBoolean".into()),
                init: None,
            },
            IrStmt::LocalVar {
                name: "v".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "ab".into(),
                        ty: IrType::Class("AtomicBoolean".into()),
                    })),
                    method_name: "get".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "v2".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "ab".into(),
                        ty: IrType::Class("AtomicBoolean".into()),
                    })),
                    method_name: "compareAndSet".into(),
                    args: vec![IrExpr::LitBool(true), IrExpr::LitBool(false)],
                    ty: IrType::Unknown,
                }),
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("AtomBool", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            let body = cls.methods[0].body.as_ref().unwrap();
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[1] {
                assert_eq!(
                    *e.ty(),
                    IrType::Bool,
                    "AtomicBoolean.get should return Bool"
                );
            }
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[2] {
                assert_eq!(
                    *e.ty(),
                    IrType::Bool,
                    "AtomicBoolean.compareAndSet should return Bool"
                );
            }
        }
    }

    // ── Semaphore method types ────────────────────────────────────────────

    #[test]
    fn check_semaphore_methods() {
        let mut module = IrModule::new("");
        let stmts = vec![
            IrStmt::LocalVar {
                name: "sem".into(),
                ty: IrType::Class("Semaphore".into()),
                init: None,
            },
            IrStmt::LocalVar {
                name: "p".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "sem".into(),
                        ty: IrType::Class("Semaphore".into()),
                    })),
                    method_name: "availablePermits".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("SemTest", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            let body = cls.methods[0].body.as_ref().unwrap();
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[1] {
                assert_eq!(
                    *e.ty(),
                    IrType::Int,
                    "Semaphore.availablePermits should return Int"
                );
            }
        }
    }

    // ── JClass method types ───────────────────────────────────────────────

    #[test]
    fn check_jclass_methods() {
        let mut module = IrModule::new("");
        let stmts = vec![
            IrStmt::LocalVar {
                name: "cls".into(),
                ty: IrType::Class("JClass".into()),
                init: None,
            },
            IrStmt::LocalVar {
                name: "n".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "cls".into(),
                        ty: IrType::Class("JClass".into()),
                    })),
                    method_name: "getName".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "s".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "cls".into(),
                        ty: IrType::Class("JClass".into()),
                    })),
                    method_name: "getSimpleName".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("JClassTest", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            let body = cls.methods[0].body.as_ref().unwrap();
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[1] {
                assert_eq!(
                    *e.ty(),
                    IrType::String,
                    "JClass.getName should return String"
                );
            }
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[2] {
                assert_eq!(
                    *e.ty(),
                    IrType::String,
                    "JClass.getSimpleName should return String"
                );
            }
        }
    }

    // ── Optional instance method types ────────────────────────────────────

    #[test]
    fn check_optional_instance_methods() {
        let mut module = IrModule::new("");
        let stmts = vec![
            IrStmt::LocalVar {
                name: "opt".into(),
                ty: IrType::Class("Optional".into()),
                init: None,
            },
            IrStmt::LocalVar {
                name: "p".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "opt".into(),
                        ty: IrType::Class("Optional".into()),
                    })),
                    method_name: "isPresent".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "e".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "opt".into(),
                        ty: IrType::Class("Optional".into()),
                    })),
                    method_name: "isEmpty".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("OptInst", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            let body = cls.methods[0].body.as_ref().unwrap();
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[1] {
                assert_eq!(
                    *e.ty(),
                    IrType::Bool,
                    "Optional.isPresent should return Bool"
                );
            }
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[2] {
                assert_eq!(
                    *e.ty(),
                    IrType::Bool,
                    "JOptional.isEmpty should return Bool"
                );
            }
        }
    }

    // ── Pattern/Matcher method types ──────────────────────────────────────

    #[test]
    fn check_matcher_methods() {
        let mut module = IrModule::new("");
        let stmts = vec![
            IrStmt::LocalVar {
                name: "p".into(),
                ty: IrType::Class("Pattern".into()),
                init: None,
            },
            IrStmt::LocalVar {
                name: "mat".into(),
                ty: IrType::Class("Matcher".into()),
                init: None,
            },
            IrStmt::LocalVar {
                name: "m".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "p".into(),
                        ty: IrType::Class("Pattern".into()),
                    })),
                    method_name: "matcher".into(),
                    args: vec![IrExpr::LitString("test".into())],
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "b".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "mat".into(),
                        ty: IrType::Class("Matcher".into()),
                    })),
                    method_name: "find".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "g".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "mat".into(),
                        ty: IrType::Class("Matcher".into()),
                    })),
                    method_name: "group".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("MatcherTest", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            let body = cls.methods[0].body.as_ref().unwrap();
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[2] {
                assert_eq!(
                    *e.ty(),
                    IrType::Class("Matcher".into()),
                    "Pattern.matcher should return Matcher"
                );
            }
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[3] {
                assert_eq!(*e.ty(), IrType::Bool, "Matcher.find should return Bool");
            }
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[4] {
                assert_eq!(
                    *e.ty(),
                    IrType::String,
                    "Matcher.group should return String"
                );
            }
        }
    }

    // ── File method types ─────────────────────────────────────────────────

    #[test]
    fn check_file_methods() {
        let mut module = IrModule::new("");
        let stmts = vec![
            IrStmt::LocalVar {
                name: "f".into(),
                ty: IrType::Class("File".into()),
                init: None,
            },
            IrStmt::LocalVar {
                name: "n".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "f".into(),
                        ty: IrType::Class("File".into()),
                    })),
                    method_name: "getName".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "e".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "f".into(),
                        ty: IrType::Class("File".into()),
                    })),
                    method_name: "exists".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "l".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "f".into(),
                        ty: IrType::Class("File".into()),
                    })),
                    method_name: "length".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("FileTest", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            let body = cls.methods[0].body.as_ref().unwrap();
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[1] {
                assert_eq!(*e.ty(), IrType::String, "File.getName should return String");
            }
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[2] {
                assert_eq!(*e.ty(), IrType::Bool, "File.exists should return Bool");
            }
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[3] {
                assert_eq!(*e.ty(), IrType::Long, "File.length should return Long");
            }
        }
    }

    // ── JStream method types ──────────────────────────────────────────────

    #[test]
    fn check_jstream_methods() {
        let mut module = IrModule::new("");
        let stmts = vec![
            IrStmt::LocalVar {
                name: "s".into(),
                ty: IrType::Class("JStream".into()),
                init: None,
            },
            IrStmt::LocalVar {
                name: "c".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "s".into(),
                        ty: IrType::Class("JStream".into()),
                    })),
                    method_name: "count".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
            IrStmt::LocalVar {
                name: "f".into(),
                ty: IrType::Unknown,
                init: Some(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "s".into(),
                        ty: IrType::Class("JStream".into()),
                    })),
                    method_name: "filter".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }),
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("StreamTest", stmts)));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            let body = cls.methods[0].body.as_ref().unwrap();
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[1] {
                assert_eq!(*e.ty(), IrType::Long, "JStream.count should return Long");
            }
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[2] {
                assert_eq!(
                    *e.ty(),
                    IrType::Class("JStream".into()),
                    "JStream.filter should return JStream"
                );
            }
        }
    }

    // ── Inherited field resolution ────────────────────────────────────────

    #[test]
    fn check_inherited_field_super_path() {
        let mut module = IrModule::new("");
        let parent = IrClass {
            name: "Base".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            type_params: vec![],
            superclass: None,
            interfaces: vec![],
            fields: vec![IrField {
                name: "value".into(),
                ty: IrType::Int,
                visibility: Visibility::Public,
                is_static: false,
                is_final: false,
                is_volatile: false,
                init: None,
            }],
            methods: vec![],
            constructors: vec![],
            is_record: false,
            captures: vec![],
        };
        let child = IrClass {
            name: "Child".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            type_params: vec![],
            superclass: Some("Base".into()),
            interfaces: vec![],
            fields: vec![],
            methods: vec![IrMethod {
                name: "getVal".into(),
                visibility: Visibility::Public,
                is_static: false,
                is_abstract: false,
                is_final: false,
                is_synchronized: false,
                type_params: vec![],
                params: vec![],
                return_ty: IrType::Int,
                body: Some(vec![IrStmt::Return(Some(IrExpr::Var {
                    name: "value".into(),
                    ty: IrType::Unknown,
                }))]),
                throws: vec![],
                is_default: false,
            }],
            constructors: vec![],
            is_record: false,
            captures: vec![],
        };
        module.decls.push(IrDecl::Class(parent));
        module.decls.push(IrDecl::Class(child));
        let checked = tc(module);
        // The child's method should resolve `value` through _super
        if let IrDecl::Class(cls) = &checked.decls[1] {
            let body = cls.methods[0].body.as_ref().unwrap();
            if let IrStmt::Return(Some(e)) = &body[0] {
                assert_eq!(*e.ty(), IrType::Int, "inherited field should have type Int");
            }
        }
    }

    // ── Inherited method lookup ───────────────────────────────────────────

    #[test]
    fn check_inherited_method_lookup() {
        let mut module = IrModule::new("");
        let parent = IrClass {
            name: "Animal".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            type_params: vec![],
            superclass: None,
            interfaces: vec![],
            fields: vec![],
            methods: vec![IrMethod {
                name: "speak".into(),
                visibility: Visibility::Public,
                is_static: false,
                is_abstract: false,
                is_final: false,
                is_synchronized: false,
                type_params: vec![],
                params: vec![],
                return_ty: IrType::String,
                body: Some(vec![IrStmt::Return(Some(IrExpr::LitString("...".into())))]),
                throws: vec![],
                is_default: false,
            }],
            constructors: vec![],
            is_record: false,
            captures: vec![],
        };
        let child = IrClass {
            name: "Dog".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            type_params: vec![],
            superclass: Some("Animal".into()),
            interfaces: vec![],
            fields: vec![],
            methods: vec![IrMethod {
                name: "bark".into(),
                visibility: Visibility::Public,
                is_static: false,
                is_abstract: false,
                is_final: false,
                is_synchronized: false,
                type_params: vec![],
                params: vec![],
                return_ty: IrType::String,
                body: Some(vec![IrStmt::Return(Some(IrExpr::MethodCall {
                    receiver: None,
                    method_name: "speak".into(),
                    args: vec![],
                    ty: IrType::Unknown,
                }))]),
                throws: vec![],
                is_default: false,
            }],
            constructors: vec![],
            is_record: false,
            captures: vec![],
        };
        module.decls.push(IrDecl::Class(parent));
        module.decls.push(IrDecl::Class(child));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[1] {
            let body = cls.methods[0].body.as_ref().unwrap();
            if let IrStmt::Return(Some(e)) = &body[0] {
                assert_eq!(
                    *e.ty(),
                    IrType::String,
                    "inherited method call should resolve return type"
                );
            }
        }
    }

    // ── _super variable reference ─────────────────────────────────────────

    #[test]
    fn check_super_variable_reference() {
        let mut module = IrModule::new("");
        let parent = IrClass {
            name: "Parent".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            type_params: vec![],
            superclass: None,
            interfaces: vec![],
            fields: vec![],
            methods: vec![],
            constructors: vec![],
            is_record: false,
            captures: vec![],
        };
        let child = IrClass {
            name: "Sub".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            type_params: vec![],
            superclass: Some("Parent".into()),
            interfaces: vec![],
            fields: vec![],
            methods: vec![IrMethod {
                name: "test".into(),
                visibility: Visibility::Public,
                is_static: false,
                is_abstract: false,
                is_final: false,
                is_synchronized: false,
                type_params: vec![],
                params: vec![],
                return_ty: IrType::Void,
                body: Some(vec![IrStmt::LocalVar {
                    name: "s".into(),
                    ty: IrType::Unknown,
                    init: Some(IrExpr::Var {
                        name: "_super".into(),
                        ty: IrType::Unknown,
                    }),
                }]),
                throws: vec![],
                is_default: false,
            }],
            constructors: vec![],
            is_record: false,
            captures: vec![],
        };
        module.decls.push(IrDecl::Class(parent));
        module.decls.push(IrDecl::Class(child));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[1] {
            let body = cls.methods[0].body.as_ref().unwrap();
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[0] {
                // _super should be rewritten to self._super with parent class type
                assert_eq!(
                    *e.ty(),
                    IrType::Class("Parent".into()),
                    "_super should resolve to parent type"
                );
            }
        }
    }

    // ── Qualified method call on known class ──────────────────────────────

    #[test]
    fn check_qualified_method_call() {
        let mut module = IrModule::new("");
        let cls = IrClass {
            name: "Helper".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            type_params: vec![],
            superclass: None,
            interfaces: vec![],
            fields: vec![],
            methods: vec![
                IrMethod {
                    name: "compute".into(),
                    visibility: Visibility::Public,
                    is_static: false,
                    is_abstract: false,
                    is_final: false,
                    is_synchronized: false,
                    type_params: vec![],
                    params: vec![],
                    return_ty: IrType::Int,
                    body: Some(vec![IrStmt::Return(Some(IrExpr::LitInt(42)))]),
                    throws: vec![],
                    is_default: false,
                },
                IrMethod {
                    name: "test".into(),
                    visibility: Visibility::Public,
                    is_static: false,
                    is_abstract: false,
                    is_final: false,
                    is_synchronized: false,
                    type_params: vec![],
                    params: vec![],
                    return_ty: IrType::Void,
                    body: Some(vec![
                        IrStmt::LocalVar {
                            name: "h".into(),
                            ty: IrType::Class("Helper".into()),
                            init: None,
                        },
                        IrStmt::LocalVar {
                            name: "r".into(),
                            ty: IrType::Unknown,
                            init: Some(IrExpr::MethodCall {
                                receiver: Some(Box::new(IrExpr::Var {
                                    name: "h".into(),
                                    ty: IrType::Class("Helper".into()),
                                })),
                                method_name: "compute".into(),
                                args: vec![],
                                ty: IrType::Unknown,
                            }),
                        },
                    ]),
                    throws: vec![],
                    is_default: false,
                },
            ],
            constructors: vec![],
            is_record: false,
            captures: vec![],
        };
        module.decls.push(IrDecl::Class(cls));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[0] {
            let body = cls.methods[1].body.as_ref().unwrap();
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[1] {
                assert_eq!(
                    *e.ty(),
                    IrType::Int,
                    "qualified method call should resolve return type"
                );
            }
        }
    }

    // ── _super field access ───────────────────────────────────────────────

    #[test]
    fn check_super_field_access() {
        let mut module = IrModule::new("");
        let parent = IrClass {
            name: "Base2".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            type_params: vec![],
            superclass: None,
            interfaces: vec![],
            fields: vec![],
            methods: vec![],
            constructors: vec![],
            is_record: false,
            captures: vec![],
        };
        let child = IrClass {
            name: "Derived2".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            type_params: vec![],
            superclass: Some("Base2".into()),
            interfaces: vec![],
            fields: vec![],
            methods: vec![IrMethod {
                name: "test".into(),
                visibility: Visibility::Public,
                is_static: false,
                is_abstract: false,
                is_final: false,
                is_synchronized: false,
                type_params: vec![],
                params: vec![],
                return_ty: IrType::Void,
                body: Some(vec![IrStmt::LocalVar {
                    name: "s".into(),
                    ty: IrType::Unknown,
                    init: Some(IrExpr::FieldAccess {
                        receiver: Box::new(IrExpr::Var {
                            name: "self".into(),
                            ty: IrType::Class("Derived2".into()),
                        }),
                        field_name: "_super".into(),
                        ty: IrType::Unknown,
                    }),
                }]),
                throws: vec![],
                is_default: false,
            }],
            constructors: vec![],
            is_record: false,
            captures: vec![],
        };
        module.decls.push(IrDecl::Class(parent));
        module.decls.push(IrDecl::Class(child));
        let checked = tc(module);
        if let IrDecl::Class(cls) = &checked.decls[1] {
            let body = cls.methods[0].body.as_ref().unwrap();
            if let IrStmt::LocalVar { init: Some(e), .. } = &body[0] {
                assert_eq!(
                    *e.ty(),
                    IrType::Class("Base2".into()),
                    "_super field access should have parent type"
                );
            }
        }
    }
}
