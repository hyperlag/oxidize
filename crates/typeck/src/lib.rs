//! Type-checking pass: walks an [`ir::IrModule`] and annotates every
//! [`ir::IrExpr`] with its [`ir::IrType`].

use std::collections::HashMap;

use ir::{
    decl::{IrClass, IrMethod},
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

    for decl in &mut module.decls {
        match decl {
            IrDecl::Class(cls) => check_class(cls, &class_map)?,
            IrDecl::Interface(_) => {} // interfaces have no bodies at Stage 1
        }
    }
    Ok(module)
}

fn check_class(
    cls: &mut IrClass,
    class_map: &HashMap<String, IrClass>,
) -> Result<(), TypeckError> {
    // Snapshot the class env before mutably iterating
    let class_env: HashMap<String, IrType> = cls
        .fields
        .iter()
        .map(|f| (f.name.clone(), f.ty.clone()))
        .collect();

    // Clone what we need to avoid simultaneous mutable+immutable borrows
    let cls_snapshot = cls.clone();

    for method in &mut cls.methods {
        check_method(method, &cls_snapshot, class_map, &class_env)?;
    }
    for ctor in &mut cls.constructors {
        let mut local_env = class_env.clone();
        for p in &ctor.params {
            local_env.insert(p.name.clone(), p.ty.clone());
        }
        for stmt in &mut ctor.body {
            check_stmt(stmt, &cls_snapshot, class_map, &mut local_env)?;
        }
    }

    // annotate field initialisers
    for field in &mut cls.fields {
        if let Some(init) = &mut field.init {
            *init = check_expr(init.clone(), &cls_snapshot, class_map, &class_env)?;
        }
    }
    Ok(())
}

fn check_method(
    method: &mut IrMethod,
    cls: &IrClass,
    class_map: &HashMap<String, IrClass>,
    class_env: &HashMap<String, IrType>,
) -> Result<(), TypeckError> {
    let Some(body) = &mut method.body else {
        return Ok(());
    };
    let mut env = class_env.clone();
    for p in &method.params {
        env.insert(p.name.clone(), p.ty.clone());
    }
    for stmt in body {
        check_stmt(stmt, cls, class_map, &mut env)?;
    }
    Ok(())
}

fn check_stmt(
    stmt: &mut IrStmt,
    cls: &IrClass,
    class_map: &HashMap<String, IrClass>,
    env: &mut HashMap<String, IrType>,
) -> Result<(), TypeckError> {
    match stmt {
        IrStmt::LocalVar { name, ty, init } => {
            if let Some(init_expr) = init {
                *init_expr = check_expr(init_expr.clone(), cls, class_map, env)?;
                // if type is Unknown, infer from init
                if *ty == IrType::Unknown {
                    *ty = init_expr.ty().clone();
                }
            }
            env.insert(name.clone(), ty.clone());
        }
        IrStmt::Expr(e) => {
            *e = check_expr(e.clone(), cls, class_map, env)?;
        }
        IrStmt::Return(Some(e)) => {
            *e = check_expr(e.clone(), cls, class_map, env)?;
        }
        IrStmt::Return(None) => {}
        IrStmt::If { cond, then_, else_ } => {
            *cond = check_expr(cond.clone(), cls, class_map, env)?;
            for s in then_.iter_mut() {
                check_stmt(s, cls, class_map, &mut env.clone())?;
            }
            if let Some(else_stmts) = else_ {
                for s in else_stmts.iter_mut() {
                    check_stmt(s, cls, class_map, &mut env.clone())?;
                }
            }
        }
        IrStmt::While { cond, body } | IrStmt::DoWhile { body, cond } => {
            *cond = check_expr(cond.clone(), cls, class_map, env)?;
            let mut loop_env = env.clone();
            for s in body.iter_mut() {
                check_stmt(s, cls, class_map, &mut loop_env)?;
            }
        }
        IrStmt::For {
            init,
            cond,
            update,
            body,
        } => {
            let mut loop_env = env.clone();
            if let Some(init_stmt) = init {
                check_stmt(init_stmt, cls, class_map, &mut loop_env)?;
            }
            if let Some(c) = cond {
                *c = check_expr(c.clone(), cls, class_map, &loop_env)?;
            }
            for u in update.iter_mut() {
                *u = check_expr(u.clone(), cls, class_map, &loop_env)?;
            }
            for s in body.iter_mut() {
                check_stmt(s, cls, class_map, &mut loop_env)?;
            }
        }
        IrStmt::ForEach {
            var,
            var_ty,
            iterable,
            body,
        } => {
            *iterable = check_expr(iterable.clone(), cls, class_map, env)?;
            let mut loop_env = env.clone();
            loop_env.insert(var.clone(), var_ty.clone());
            for s in body.iter_mut() {
                check_stmt(s, cls, class_map, &mut loop_env)?;
            }
        }
        IrStmt::Switch { expr, cases, default } => {
            *expr = check_expr(expr.clone(), cls, class_map, env)?;
            for case in cases.iter_mut() {
                case.value = check_expr(case.value.clone(), cls, class_map, env)?;
                let mut case_env = env.clone();
                for s in case.body.iter_mut() {
                    check_stmt(s, cls, class_map, &mut case_env)?;
                }
            }
            if let Some(def_stmts) = default {
                let mut def_env = env.clone();
                for s in def_stmts.iter_mut() {
                    check_stmt(s, cls, class_map, &mut def_env)?;
                }
            }
        }
        IrStmt::Block(stmts) => {
            let mut block_env = env.clone();
            for s in stmts.iter_mut() {
                check_stmt(s, cls, class_map, &mut block_env)?;
            }
        }
        IrStmt::Throw(e) => {
            *e = check_expr(e.clone(), cls, class_map, env)?;
        }
        IrStmt::TryCatch {
            body,
            catches,
            finally,
        } => {
            let mut try_env = env.clone();
            for s in body.iter_mut() {
                check_stmt(s, cls, class_map, &mut try_env)?;
            }
            for catch in catches.iter_mut() {
                let mut catch_env = env.clone();
                catch_env.insert(
                    catch.var.clone(),
                    IrType::Class(
                        catch.exception_types.first().cloned().unwrap_or_default(),
                    ),
                );
                for s in catch.body.iter_mut() {
                    check_stmt(s, cls, class_map, &mut catch_env)?;
                }
            }
            if let Some(fin) = finally {
                let mut fin_env = env.clone();
                for s in fin.iter_mut() {
                    check_stmt(s, cls, class_map, &mut fin_env)?;
                }
            }
        }
        IrStmt::Break(_) | IrStmt::Continue(_) => {}
    }
    Ok(())
}

fn check_expr(
    expr: IrExpr,
    cls: &IrClass,
    class_map: &HashMap<String, IrClass>,
    env: &HashMap<String, IrType>,
) -> Result<IrExpr, TypeckError> {
    match expr {
        // Literals already have correct types
        IrExpr::LitBool(_)
        | IrExpr::LitInt(_)
        | IrExpr::LitLong(_)
        | IrExpr::LitFloat(_)
        | IrExpr::LitDouble(_)
        | IrExpr::LitChar(_)
        | IrExpr::LitString(_)
        | IrExpr::LitNull => Ok(expr),

        // Variable references
        IrExpr::Var { name, ty: _ } => {
            // Check fields and locals
            if let Some(t) = env.get(&name) {
                Ok(IrExpr::Var {
                    ty: t.clone(),
                    name,
                })
            } else if let Some(field) = cls.fields.iter().find(|f| f.name == name) {
                Ok(IrExpr::Var {
                    ty: field.ty.clone(),
                    name,
                })
            } else {
                // Could be a class name (e.g. System) — leave as Unknown
                Ok(IrExpr::Var {
                    ty: IrType::Unknown,
                    name,
                })
            }
        }

        IrExpr::FieldAccess {
            receiver,
            field_name,
            ..
        } => {
            let receiver = check_expr(*receiver, cls, class_map, env)?;
            // Special-case: System.out is a PrintStream (treat as void-output sink)
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
                .map(|r| check_expr(*r, cls, class_map, env).map(Box::new))
                .transpose()?;
            let args = args
                .into_iter()
                .map(|a| check_expr(a, cls, class_map, env))
                .collect::<Result<Vec<_>, _>>()?;
            let ty = resolve_method_return_type(receiver.as_deref(), &method_name, &args, cls, class_map);
            Ok(IrExpr::MethodCall {
                receiver,
                method_name,
                args,
                ty,
            })
        }

        IrExpr::BinOp { op, lhs, rhs, .. } => {
            let lhs = check_expr(*lhs, cls, class_map, env)?;
            let rhs = check_expr(*rhs, cls, class_map, env)?;
            let ty = binop_type(&op, lhs.ty(), rhs.ty());
            Ok(IrExpr::BinOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                ty,
            })
        }

        IrExpr::UnOp { op, operand, .. } => {
            let operand = check_expr(*operand, cls, class_map, env)?;
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
            let cond = check_expr(*cond, cls, class_map, env)?;
            let then_ = check_expr(*then_, cls, class_map, env)?;
            let else_ = check_expr(*else_, cls, class_map, env)?;
            let ty = then_.ty().clone();
            Ok(IrExpr::Ternary {
                cond: Box::new(cond),
                then_: Box::new(then_),
                else_: Box::new(else_),
                ty,
            })
        }

        IrExpr::Assign { lhs, rhs, .. } => {
            let lhs = check_expr(*lhs, cls, class_map, env)?;
            let rhs = check_expr(*rhs, cls, class_map, env)?;
            let ty = lhs.ty().clone();
            Ok(IrExpr::Assign {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                ty,
            })
        }

        IrExpr::CompoundAssign { op, lhs, rhs, .. } => {
            let lhs = check_expr(*lhs, cls, class_map, env)?;
            let rhs = check_expr(*rhs, cls, class_map, env)?;
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
                .map(|a| check_expr(a, cls, class_map, env))
                .collect::<Result<Vec<_>, _>>()?;
            let ty = IrType::Class(class.clone());
            Ok(IrExpr::New { class, args, ty })
        }

        IrExpr::NewArray { elem_ty, len, .. } => {
            let len = check_expr(*len, cls, class_map, env)?;
            let ty = IrType::Array(Box::new(elem_ty.clone()));
            Ok(IrExpr::NewArray {
                elem_ty,
                len: Box::new(len),
                ty,
            })
        }

        IrExpr::ArrayAccess { array, index, .. } => {
            let array = check_expr(*array, cls, class_map, env)?;
            let index = check_expr(*index, cls, class_map, env)?;
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

        IrExpr::Cast { target, expr } => {
            let expr = check_expr(*expr, cls, class_map, env)?;
            Ok(IrExpr::Cast {
                target,
                expr: Box::new(expr),
            })
        }

        IrExpr::InstanceOf { expr, check_type } => {
            let expr = check_expr(*expr, cls, class_map, env)?;
            Ok(IrExpr::InstanceOf {
                expr: Box::new(expr),
                check_type,
            })
        }
    }
}

fn binop_type(op: &BinOp, lhs: &IrType, rhs: &IrType) -> IrType {
    match op {
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
        | BinOp::And | BinOp::Or => IrType::Bool,
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
            // Look up in class map
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

fn resolve_method_return_type(
    receiver: Option<&IrExpr>,
    method_name: &str,
    _args: &[IrExpr],
    cls: &IrClass,
    class_map: &HashMap<String, IrClass>,
) -> IrType {
    // System.out.println / System.err.println → void
    if method_name == "println" || method_name == "print" || method_name == "printf" {
        return IrType::Void;
    }
    // String methods
    match method_name {
        "length" => return IrType::Int,
        "charAt" => return IrType::Char,
        "substring" | "toString" | "trim" | "toLowerCase" | "toUpperCase" | "concat"
        | "replace" | "valueOf" => return IrType::String,
        "equals" | "equalsIgnoreCase" | "contains" | "startsWith" | "endsWith"
        | "isEmpty" | "matches" => return IrType::Bool,
        "indexOf" | "lastIndexOf" | "compareTo" => return IrType::Int,
        "parseInt" => return IrType::Int,
        "parseLong" => return IrType::Long,
        "parseDouble" => return IrType::Double,
        "parseFloat" => return IrType::Float,
        _ => {}
    }

    // Look up in the same class
    if receiver.is_none() {
        if let Some(m) = cls.methods.iter().find(|m| m.name == method_name) {
            return m.return_ty.clone();
        }
    }

    // Look up on receiver class
    if let Some(recv) = receiver {
        let recv_ty = recv.ty();
        if let IrType::Class(class_name) = recv_ty {
            if let Some(recv_cls) = class_map.get(class_name) {
                if let Some(m) = recv_cls.methods.iter().find(|m| m.name == method_name) {
                    return m.return_ty.clone();
                }
            }
        }
    }

    IrType::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;
    use ir::IrModule;

    #[test]
    fn check_simple_module() {
        let module = IrModule::new("test");
        let result = type_check(module);
        assert!(result.is_ok());
    }
}

