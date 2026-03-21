//! Type-checking pass: walks an [`ir::IrModule`] and annotates every
//! [`ir::IrExpr`] with its resolved [`ir::IrType`].
//!
//! The pass maintains a scoped symbol table mapping variable names to their
//! declared types. It resolves `IrType::Unknown` on `Var`, `BinOp`, `UnOp`,
//! `MethodCall`, `FieldAccess`, `Assign`, and `CompoundAssign` expressions.
//!
//! Stage 1 scope:
//! - Primitive types and `String`
//! - Local variables, method parameters, and static fields
//! - `System.out.println` / `System.out.print` → `void`
//! - Arithmetic and comparison operators
//! - String concatenation via `+`

use std::collections::HashMap;

use ir::decl::{IrClass, IrMethod};
use ir::expr::{BinOp, UnOp};
use ir::{IrDecl, IrExpr, IrModule, IrStmt, IrType};
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

/// Run the type-checking pass over `module`, mutating all `IrType::Unknown`
/// nodes to their resolved types. Errors are collected and returned together;
/// the walk continues even after errors so callers see the full set.
pub fn check(module: &mut IrModule) -> Vec<TypeckError> {
    let mut errors = Vec::new();
    // Build a module-level class map first (needed to resolve `this` / field access)
    let class_map = build_class_map(module);
    for decl in &mut module.decls {
        match decl {
            IrDecl::Class(cls) => {
                check_class(cls, &class_map, &mut errors);
            }
            IrDecl::Interface(_) => {}
        }
    }
    errors
}

// ─── Class map ────────────────────────────────────────────────────────────────

/// A lightweight view of a class used during type resolution.
#[derive(Debug)]
struct ClassInfo {
    /// Static and instance fields: name → type
    fields: HashMap<String, IrType>,
    /// Methods: name → return type (simplified: last definition wins)
    methods: HashMap<String, IrType>,
}

fn build_class_map(module: &IrModule) -> HashMap<String, ClassInfo> {
    let mut map = HashMap::new();
    for decl in &module.decls {
        if let IrDecl::Class(cls) = decl {
            let mut info = ClassInfo {
                fields: HashMap::new(),
                methods: HashMap::new(),
            };
            for f in &cls.fields {
                info.fields.insert(f.name.clone(), f.ty.clone());
            }
            for m in &cls.methods {
                info.methods.insert(m.name.clone(), m.return_ty.clone());
            }
            map.insert(cls.name.clone(), info);
        }
    }
    map
}

// ─── Class / method checking ──────────────────────────────────────────────────

fn check_class(cls: &mut IrClass, class_map: &HashMap<String, ClassInfo>, errors: &mut Vec<TypeckError>) {
    // Collect static fields into a scope available to all methods
    let mut class_scope: HashMap<String, IrType> = HashMap::new();
    for f in &cls.fields {
        class_scope.insert(f.name.clone(), f.ty.clone());
    }

    for method in &mut cls.methods {
        check_method(method, &class_scope, class_map, errors);
    }
    for ctor in &mut cls.constructors {
        let mut scope = class_scope.clone();
        inject_builtins(&mut scope);
        for p in &ctor.params {
            scope.insert(p.name.clone(), p.ty.clone());
        }
        let mut cx = Checker { scope, class_map, errors };
        let mut body = std::mem::take(&mut ctor.body);
        cx.check_stmts(&mut body);
        ctor.body = body;
    }
}

fn check_method(
    method: &mut IrMethod,
    class_scope: &HashMap<String, IrType>,
    class_map: &HashMap<String, ClassInfo>,
    errors: &mut Vec<TypeckError>,
) {
    let mut scope = class_scope.clone();
    // Pre-populate well-known Java built-ins so they don't trigger undefined errors
    inject_builtins(&mut scope);
    for p in &method.params {
        scope.insert(p.name.clone(), p.ty.clone());
    }
    if let Some(body) = &mut method.body {
        let mut cx = Checker { scope, class_map, errors };
        cx.check_stmts(body);
    }
}

/// Insert well-known Java built-in names into `scope` so the type checker
/// does not report them as undefined.
fn inject_builtins(scope: &mut HashMap<String, IrType>) {
    scope.insert("System".into(), IrType::Class("System".into()));
    scope.insert("Math".into(), IrType::Class("Math".into()));
    scope.insert("Integer".into(), IrType::Class("Integer".into()));
    scope.insert("Long".into(), IrType::Class("Long".into()));
    scope.insert("Double".into(), IrType::Class("Double".into()));
    scope.insert("Boolean".into(), IrType::Class("Boolean".into()));
    scope.insert("String".into(), IrType::Class("String".into()));
    scope.insert("Object".into(), IrType::Class("Object".into()));
}

// ─── Checker ──────────────────────────────────────────────────────────────────

struct Checker<'a> {
    /// Current lexical scope: variable name → type.
    scope: HashMap<String, IrType>,
    class_map: &'a HashMap<String, ClassInfo>,
    errors: &'a mut Vec<TypeckError>,
}

impl<'a> Checker<'a> {
    fn child(&mut self) -> Checker<'_> {
        Checker {
            scope: self.scope.clone(),
            class_map: self.class_map,
            errors: self.errors,
        }
    }

    // ── Statements ────────────────────────────────────────────────────────────

    fn check_stmts(&mut self, stmts: &mut Vec<IrStmt>) {
        for stmt in stmts {
            self.check_stmt(stmt);
        }
    }

    fn check_stmt(&mut self, stmt: &mut IrStmt) {
        match stmt {
            IrStmt::LocalVar { name, ty, init } => {
                if let Some(init_expr) = init {
                    self.check_expr(init_expr);
                    // If the declared type is Unknown, infer from init
                    if *ty == IrType::Unknown {
                        *ty = init_expr.ty().clone();
                    }
                }
                self.scope.insert(name.clone(), ty.clone());
            }
            IrStmt::Expr(e) => {
                self.check_expr(e);
            }
            IrStmt::Return(Some(e)) => {
                self.check_expr(e);
            }
            IrStmt::Return(None) => {}
            IrStmt::Throw(e) => {
                self.check_expr(e);
            }
            IrStmt::If { cond, then_, else_ } => {
                self.check_expr(cond);
                self.child().check_stmts(then_);
                if let Some(else_stmts) = else_ {
                    self.child().check_stmts(else_stmts);
                }
            }
            IrStmt::While { cond, body } => {
                self.check_expr(cond);
                self.child().check_stmts(body);
            }
            IrStmt::DoWhile { body, cond } => {
                self.child().check_stmts(body);
                self.check_expr(cond);
            }
            IrStmt::For { init, cond, update, body } => {
                let mut child = self.child();
                if let Some(init_stmt) = init {
                    child.check_stmt(init_stmt);
                }
                if let Some(cond_expr) = cond {
                    child.check_expr(cond_expr);
                }
                for u in update.iter_mut() {
                    child.check_expr(u);
                }
                child.check_stmts(body);
            }
            IrStmt::ForEach { var, var_ty, iterable, body } => {
                self.check_expr(iterable);
                let mut child = self.child();
                child.scope.insert(var.clone(), var_ty.clone());
                child.check_stmts(body);
            }
            IrStmt::Switch { expr, cases, default } => {
                self.check_expr(expr);
                for case in cases.iter_mut() {
                    self.check_expr(&mut case.value);
                    self.child().check_stmts(&mut case.body);
                }
                if let Some(d) = default {
                    self.child().check_stmts(d);
                }
            }
            IrStmt::Block(stmts) => {
                self.child().check_stmts(stmts);
            }
            IrStmt::TryCatch { body, catches, finally } => {
                self.child().check_stmts(body);
                for catch in catches.iter_mut() {
                    let mut child = self.child();
                    child.scope.insert(catch.var.clone(), IrType::Class("Exception".into()));
                    child.check_stmts(&mut catch.body);
                }
                if let Some(f) = finally {
                    self.child().check_stmts(f);
                }
            }
            IrStmt::Break(_) | IrStmt::Continue(_) => {}
        }
    }

    // ── Expressions ───────────────────────────────────────────────────────────

    fn check_expr(&mut self, expr: &mut IrExpr) {
        match expr {
            // Literals already have concrete types
            IrExpr::LitBool(_)
            | IrExpr::LitInt(_)
            | IrExpr::LitLong(_)
            | IrExpr::LitFloat(_)
            | IrExpr::LitDouble(_)
            | IrExpr::LitChar(_)
            | IrExpr::LitString(_)
            | IrExpr::LitNull => {}

            IrExpr::Var { name, ty } => {
                if *ty == IrType::Unknown {
                    if let Some(resolved) = self.scope.get(name.as_str()) {
                        *ty = resolved.clone();
                    } else if name != "this" && name != "super" {
                        self.errors.push(TypeckError::UndefinedVariable(name.clone()));
                    }
                }
            }

            IrExpr::BinOp { op, lhs, rhs, ty } => {
                self.check_expr(lhs);
                self.check_expr(rhs);
                if *ty == IrType::Unknown {
                    *ty = resolve_binop_type(op, lhs.ty(), rhs.ty());
                }
            }

            IrExpr::UnOp { op, operand, ty } => {
                self.check_expr(operand);
                if *ty == IrType::Unknown {
                    *ty = resolve_unop_type(op, operand.ty());
                }
            }

            IrExpr::Ternary { cond, then_, else_, ty } => {
                self.check_expr(cond);
                self.check_expr(then_);
                self.check_expr(else_);
                if *ty == IrType::Unknown {
                    *ty = then_.ty().clone();
                }
            }

            IrExpr::Assign { lhs, rhs, ty } => {
                self.check_expr(lhs);
                self.check_expr(rhs);
                if *ty == IrType::Unknown {
                    *ty = lhs.ty().clone();
                }
                // Update scope if assigning to a Var
                if let IrExpr::Var { name, ty: var_ty } = lhs.as_ref() {
                    self.scope.insert(name.clone(), var_ty.clone());
                }
            }

            IrExpr::CompoundAssign { op, lhs, rhs, ty } => {
                self.check_expr(lhs);
                self.check_expr(rhs);
                if *ty == IrType::Unknown {
                    *ty = resolve_binop_type(op, lhs.ty(), rhs.ty());
                }
            }

            IrExpr::MethodCall { receiver, method_name, args, ty } => {
                if let Some(recv) = receiver {
                    self.check_expr(recv);
                }
                for arg in args.iter_mut() {
                    self.check_expr(arg);
                }
                if *ty == IrType::Unknown {
                    *ty = self.resolve_method_call_type(receiver.as_deref(), method_name);
                }
            }

            IrExpr::FieldAccess { receiver, field_name, ty } => {
                self.check_expr(receiver);
                if *ty == IrType::Unknown {
                    *ty = self.resolve_field_access_type(receiver, field_name);
                }
            }

            IrExpr::New { args, ty, .. } => {
                for arg in args.iter_mut() {
                    self.check_expr(arg);
                }
                // ty is already set to Class(name) by the parser
                let _ = ty;
            }

            IrExpr::NewArray { len, .. } => {
                self.check_expr(len);
            }

            IrExpr::ArrayAccess { array, index, ty } => {
                self.check_expr(array);
                self.check_expr(index);
                if *ty == IrType::Unknown {
                    if let IrType::Array(elem) = array.ty() {
                        *ty = *elem.clone();
                    }
                }
            }

            IrExpr::Cast { expr, .. } => {
                self.check_expr(expr);
            }

            IrExpr::InstanceOf { expr, .. } => {
                self.check_expr(expr);
            }
        }
    }

    // ── Resolution helpers ────────────────────────────────────────────────────

    fn resolve_method_call_type(
        &self,
        receiver: Option<&IrExpr>,
        method_name: &str,
    ) -> IrType {
        // System.out.println / System.out.print → void
        if let Some(recv) = receiver {
            if let IrExpr::FieldAccess { receiver: outer, field_name, .. } = recv {
                if field_name == "out" {
                    if let IrExpr::Var { name, .. } = outer.as_ref() {
                        if name == "System"
                            && (method_name == "println"
                                || method_name == "print"
                                || method_name == "printf")
                        {
                            return IrType::Void;
                        }
                    }
                }
            }
            // Look up method on the receiver's class type
            if let IrType::Class(class_name) = recv.ty() {
                if let Some(info) = self.class_map.get(class_name.as_str()) {
                    if let Some(ty) = info.methods.get(method_name) {
                        return ty.clone();
                    }
                }
            }
        } else {
            // Unqualified call — look in class_map methods
            for info in self.class_map.values() {
                if let Some(ty) = info.methods.get(method_name) {
                    return ty.clone();
                }
            }
        }
        IrType::Unknown
    }

    fn resolve_field_access_type(&self, receiver: &IrExpr, field_name: &str) -> IrType {
        // System.out → special Object (we model as Class("PrintStream"))
        if let IrExpr::Var { name, .. } = receiver {
            if name == "System" && field_name == "out" {
                return IrType::Class("PrintStream".into());
            }
        }
        if let IrType::Class(class_name) = receiver.ty() {
            if let Some(info) = self.class_map.get(class_name.as_str()) {
                if let Some(ty) = info.fields.get(field_name) {
                    return ty.clone();
                }
            }
        }
        IrType::Unknown
    }
}

// ─── Operator type rules ──────────────────────────────────────────────────────

fn resolve_binop_type(op: &BinOp, lhs: &IrType, rhs: &IrType) -> IrType {
    match op {
        // Comparison and logical → bool
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
        | BinOp::And | BinOp::Or => IrType::Bool,
        // String concatenation
        BinOp::Concat => IrType::String,
        // Arithmetic: if either operand is String (+ was mis-categorised), → String
        BinOp::Add if lhs == &IrType::String || rhs == &IrType::String => IrType::String,
        // Arithmetic: promote to the wider numeric type
        _ => numeric_promotion(lhs, rhs),
    }
}

fn resolve_unop_type(op: &UnOp, operand: &IrType) -> IrType {
    match op {
        UnOp::Not => IrType::Bool,
        UnOp::PreInc | UnOp::PreDec | UnOp::PostInc | UnOp::PostDec
        | UnOp::Neg | UnOp::BitNot => operand.clone(),
    }
}

fn numeric_promotion(lhs: &IrType, rhs: &IrType) -> IrType {
    fn rank(t: &IrType) -> u8 {
        match t {
            IrType::Double => 6,
            IrType::Float => 5,
            IrType::Long => 4,
            IrType::Int => 3,
            IrType::Short => 2,
            IrType::Byte => 1,
            IrType::Char => 1,
            _ => 0,
        }
    }
    if rank(lhs) >= rank(rhs) {
        lhs.clone()
    } else {
        rhs.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ir::decl::{IrClass, IrMethod, IrParam, Visibility};
    use ir::IrModule;

    fn make_simple_module() -> IrModule {
        let mut m = IrModule::new("");
        m.decls.push(IrDecl::Class(IrClass {
            name: "Calc".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            type_params: vec![],
            superclass: None,
            interfaces: vec![],
            fields: vec![],
            constructors: vec![],
            methods: vec![IrMethod {
                name: "add".into(),
                visibility: Visibility::Public,
                is_static: true,
                is_abstract: false,
                is_final: false,
                type_params: vec![],
                params: vec![
                    IrParam { name: "a".into(), ty: IrType::Int, is_varargs: false },
                    IrParam { name: "b".into(), ty: IrType::Int, is_varargs: false },
                ],
                return_ty: IrType::Int,
                body: Some(vec![IrStmt::Return(Some(IrExpr::BinOp {
                    op: ir::expr::BinOp::Add,
                    lhs: Box::new(IrExpr::Var { name: "a".into(), ty: IrType::Unknown }),
                    rhs: Box::new(IrExpr::Var { name: "b".into(), ty: IrType::Unknown }),
                    ty: IrType::Unknown,
                }))]),
                throws: vec![],
            }],
        }));
        m
    }

    #[test]
    fn resolves_variable_types() {
        let mut module = make_simple_module();
        let errs = check(&mut module);
        assert!(errs.is_empty(), "expected no errors, got: {errs:?}");

        if let IrDecl::Class(cls) = &module.decls[0] {
            if let Some(body) = &cls.methods[0].body {
                if let IrStmt::Return(Some(IrExpr::BinOp { lhs, rhs, ty, .. })) = &body[0] {
                    assert_eq!(lhs.ty(), &IrType::Int);
                    assert_eq!(rhs.ty(), &IrType::Int);
                    assert_eq!(ty, &IrType::Int);
                } else {
                    panic!("unexpected body");
                }
            }
        }
    }

    #[test]
    fn reports_undefined_variable() {
        let mut module = IrModule::new("");
        module.decls.push(IrDecl::Class(IrClass {
            name: "Bad".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            type_params: vec![],
            superclass: None,
            interfaces: vec![],
            fields: vec![],
            constructors: vec![],
            methods: vec![IrMethod {
                name: "f".into(),
                visibility: Visibility::Public,
                is_static: true,
                is_abstract: false,
                is_final: false,
                type_params: vec![],
                params: vec![],
                return_ty: IrType::Void,
                body: Some(vec![IrStmt::Expr(IrExpr::Var {
                    name: "undeclared".into(),
                    ty: IrType::Unknown,
                })]),
                throws: vec![],
            }],
        }));
        let errs = check(&mut module);
        assert!(!errs.is_empty());
        assert!(matches!(&errs[0], TypeckError::UndefinedVariable(n) if n == "undeclared"));
    }
}
