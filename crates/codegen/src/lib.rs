//! Code-generation pass: lowers a typed [`ir::IrModule`] to a Rust source
//! string using `proc-macro2` and `quote`.

use ir::{
    decl::{IrClass, IrConstructor, IrInterface, IrMethod, IrParam},
    expr::{BinOp, UnOp},
    stmt::SwitchCase,
    IrDecl, IrExpr, IrModule, IrStmt, IrType,
};
use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::quote;
use thiserror::Error;

/// Errors produced during code-generation.
#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("unsupported IR node: {0}")]
    Unsupported(String),
}

/// Lower an [`IrModule`] to a Rust source string (formatted via prettyplease
/// or just `to_string()` if unavailable).
pub fn generate(module: &IrModule) -> Result<String, CodegenError> {
    let mut items: Vec<TokenStream> = Vec::new();

    // Emit a use for the runtime crate
    items.push(quote! {
        #[allow(unused_imports)]
        use java_compat::{JString, JArray, JObject, JNull, JList, JMap, JSet};
    });

    // Build maps so emit_class can find parent class info and interface methods.
    let class_map: std::collections::HashMap<String, &IrClass> = module
        .decls
        .iter()
        .filter_map(|d| {
            if let IrDecl::Class(c) = d {
                Some((c.name.clone(), c))
            } else {
                None
            }
        })
        .collect();

    let interface_map: std::collections::HashMap<String, &IrInterface> = module
        .decls
        .iter()
        .filter_map(|d| {
            if let IrDecl::Interface(i) = d {
                Some((i.name.clone(), i))
            } else {
                None
            }
        })
        .collect();

    // Collect class names so we know what to emit a main() for
    let mut main_class: Option<String> = None;

    for decl in &module.decls {
        match decl {
            IrDecl::Class(cls) => {
                if cls.methods.iter().any(|m| m.name == "main" && m.is_static) {
                    main_class = Some(cls.name.clone());
                }
                items.push(emit_class(cls, &class_map, &interface_map)?);
            }
            IrDecl::Interface(iface) => {
                items.push(emit_interface(iface)?);
            }
        }
    }

    // Emit fn main() that calls ClassName::main(args)
    if let Some(class_name) = main_class {
        let class_ident = ident(&class_name);
        items.push(quote! {
            fn main() {
                let args: JArray<JString> = JArray::from_vec(
                    std::env::args().skip(1).map(|s| JString::from(s.as_str())).collect()
                );
                #class_ident::main(args);
            }
        });
    }

    let file: TokenStream = items.into_iter().collect();
    // Use prettyplease if available, else just token-string
    Ok(format_tokens(file))
}

fn format_tokens(ts: TokenStream) -> String {
    let code = ts.to_string();
    // Try to parse & format as a syn file for readability
    if let Ok(parsed) = syn::parse_file(&code) {
        prettyplease::unparse(&parsed)
    } else {
        code
    }
}

// ─── interface ──────────────────────────────────────────────────────────────

fn emit_interface(iface: &IrInterface) -> Result<TokenStream, CodegenError> {
    let name = ident(&iface.name);
    let method_sigs: Vec<TokenStream> = iface
        .methods
        .iter()
        .filter(|m| !m.is_static)
        .map(|m| {
            let mname = ident(&m.name);
            let params = emit_params_sig(&m.params);
            let ret_ty = emit_type(&m.return_ty);
            if m.return_ty == IrType::Void {
                quote! { fn #mname(&mut self, #(#params),*); }
            } else {
                quote! { fn #mname(&mut self, #(#params),*) -> #ret_ty; }
            }
        })
        .collect();
    Ok(quote! {
        pub trait #name {
            #(#method_sigs)*
        }
    })
}

// ─── class ──────────────────────────────────────────────────────────────────

fn emit_class(
    cls: &IrClass,
    class_map: &std::collections::HashMap<String, &IrClass>,
    interface_map: &std::collections::HashMap<String, &IrInterface>,
) -> Result<TokenStream, CodegenError> {
    let name = ident(&cls.name);

    // Generic type parameters: `struct Foo<T>` / `impl<T: Clone + Default + Debug> Foo<T>`
    let (struct_generics, impl_generics) = if cls.type_params.is_empty() {
        (quote! {}, quote! {})
    } else {
        let names: Vec<_> = cls.type_params.iter().map(|tp| ident(tp)).collect();
        let bounds: Vec<_> = cls
            .type_params
            .iter()
            .map(|tp| {
                let id = ident(tp);
                quote! { #id: Clone + Default + ::std::fmt::Debug }
            })
            .collect();
        (quote! { <#(#names),*> }, quote! { <#(#bounds),*> })
    };

    // Collect the set of method names that a class implements via interfaces.
    // Those methods will be emitted only inside `impl Interface for Class`
    // blocks, not in `impl Class`.
    let interface_methods: std::collections::HashSet<String> = cls
        .interfaces
        .iter()
        .filter_map(|iname| interface_map.get(iname))
        .flat_map(|iface| iface.methods.iter().map(|m| m.name.clone()))
        .collect();

    // Own instance fields (not static, not in a parent)
    let own_instance_fields: Vec<&ir::decl::IrField> =
        cls.fields.iter().filter(|f| !f.is_static).collect();

    // Struct fields = optional parent struct + own instance fields
    let mut struct_fields: Vec<TokenStream> = Vec::new();
    if let Some(parent_name) = &cls.superclass {
        let parent_ident = ident(parent_name);
        struct_fields.push(quote! { pub _super: #parent_ident, });
    }
    for f in &own_instance_fields {
        let fname = ident(&f.name);
        let fty = emit_type(&f.ty);
        struct_fields.push(quote! { pub #fname: #fty, });
    }

    let struct_def = if struct_fields.is_empty() {
        quote! {
            #[derive(Debug, Clone, Default)]
            pub struct #name #struct_generics;
        }
    } else {
        quote! {
            #[derive(Debug, Clone, Default)]
            pub struct #name #struct_generics {
                #(#struct_fields)*
            }
        }
    };

    // Static fields → const items
    let static_items: Vec<TokenStream> = cls
        .fields
        .iter()
        .filter(|f| f.is_static)
        .filter_map(|f| {
            if f.is_final {
                if let Some(init) = &f.init {
                    if let Some(lit) = as_const_literal(init) {
                        let fname = ident(&f.name.to_uppercase());
                        let fty = emit_type(&f.ty);
                        return Some(quote! {
                            pub const #fname: #fty = #lit;
                        });
                    }
                }
            }
            None
        })
        .collect();

    // Collect non-overriding ancestor methods that need delegation stubs
    let own_method_names: std::collections::HashSet<String> =
        cls.methods.iter().map(|m| m.name.clone()).collect();

    let mut delegation_methods: Vec<TokenStream> = Vec::new();
    if let Some(parent_name) = &cls.superclass {
        let ancestors = collect_all_ancestor_methods(parent_name, class_map);
        for ancestor_method in &ancestors {
            if ancestor_method.is_static {
                continue;
            }
            if own_method_names.contains(&ancestor_method.name) {
                continue;
            }
            if interface_methods.contains(&ancestor_method.name) {
                continue;
            }
            delegation_methods.push(emit_delegation_method(ancestor_method)?);
        }
    }

    // Constructors and own methods (excluding interface-implemented methods)
    let mut method_tokens: Vec<TokenStream> = Vec::new();

    // Always emit a `pub fn new() -> Self` even when no explicit constructor
    // exists, so that `new Foo()` calls in the generated code compile.
    if cls.constructors.is_empty() {
        method_tokens.push(quote! {
            pub fn new() -> Self {
                Self::default()
            }
        });
    }
    for ctor in &cls.constructors {
        method_tokens.push(emit_constructor(ctor, cls)?);
    }
    for method in &cls.methods {
        if !interface_methods.contains(&method.name) {
            method_tokens.push(emit_method(method, &cls.name)?);
        }
    }
    method_tokens.extend(delegation_methods);

    // `_instanceof` — enables the Java `instanceof` operator.
    // Checks own class name, each implemented interface name, then delegates
    // up the `_super` composition chain for inherited types.
    {
        let own_name_str = &cls.name;
        let iface_checks: Vec<TokenStream> = cls
            .interfaces
            .iter()
            .map(|iname| quote! { || type_name == #iname })
            .collect();
        let super_check = if cls.superclass.is_some() {
            quote! { || self._super._instanceof(type_name) }
        } else {
            quote! {}
        };
        method_tokens.push(quote! {
            pub fn _instanceof(&self, type_name: &str) -> bool {
                type_name == #own_name_str #(#iface_checks)* #super_check
            }
        });
    }

    let impl_block = if !method_tokens.is_empty() {
        quote! {
            impl #impl_generics #name #struct_generics {
                #(#method_tokens)*
            }
        }
    } else {
        quote! {}
    };

    // `impl Interface for Class` blocks
    let mut trait_impls: Vec<TokenStream> = Vec::new();
    for iface_name in &cls.interfaces {
        if let Some(iface) = interface_map.get(iface_name) {
            let iface_ident = ident(iface_name);
            let impl_methods: Vec<TokenStream> = iface
                .methods
                .iter()
                .filter(|m| !m.is_static)
                .map(|iface_method| {
                    // Find the matching implementation in cls.methods
                    let body_method = cls.methods.iter().find(|m| m.name == iface_method.name);
                    match body_method {
                        Some(m) => emit_trait_method(m),
                        None => {
                            // Provide a panic stub for unimplemented methods
                            let mname = ident(&iface_method.name);
                            let params = emit_params(&iface_method.params);
                            let ret_ty = emit_type(&iface_method.return_ty);
                            if iface_method.return_ty == IrType::Void {
                                Ok(quote! {
                                    fn #mname(&mut self, #(#params),*) {
                                        unimplemented!()
                                    }
                                })
                            } else {
                                Ok(quote! {
                                    fn #mname(&mut self, #(#params),*) -> #ret_ty {
                                        unimplemented!()
                                    }
                                })
                            }
                        }
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            trait_impls.push(quote! {
                impl #impl_generics #iface_ident for #name #struct_generics {
                    #(#impl_methods)*
                }
            });
        }
    }

    Ok(quote! {
        #struct_def
        #(#static_items)*
        #impl_block
        #(#trait_impls)*
    })
}

/// Collect all non-overridden ancestor instance methods accessible through the
/// `_super` chain.  Returns each method paired with how many `_super` hops are
/// needed to call it (for delegation).
fn collect_all_ancestor_methods<'a>(
    parent_name: &str,
    class_map: &'a std::collections::HashMap<String, &'a IrClass>,
) -> Vec<&'a IrMethod> {
    let Some(parent_cls) = class_map.get(parent_name) else {
        return vec![];
    };
    let mut result: Vec<&IrMethod> = parent_cls.methods.iter().filter(|m| !m.is_static).collect();

    // Recursively collect ancestors and add their methods if not already
    // shadowed by a closer ancestor.
    if let Some(grandparent_name) = &parent_cls.superclass {
        let ancestor_methods = collect_all_ancestor_methods(grandparent_name, class_map);
        let already: std::collections::HashSet<&str> =
            result.iter().map(|m| m.name.as_str()).collect();
        for m in ancestor_methods {
            if !already.contains(m.name.as_str()) {
                result.push(m);
            }
        }
    }
    result
}

/// Emit a delegation stub that forwards a call to `self._super.method(args)`.
fn emit_delegation_method(method: &IrMethod) -> Result<TokenStream, CodegenError> {
    let name = ident(&method.name);
    let params = emit_params(&method.params);
    let param_names: Vec<TokenStream> = method
        .params
        .iter()
        .map(|p| {
            let n = ident(&p.name);
            quote! { #n }
        })
        .collect();
    let ret_ty = emit_type(&method.return_ty);
    if method.return_ty == IrType::Void {
        Ok(quote! {
            pub fn #name(&mut self, #(#params),*) {
                self._super.#name(#(#param_names),*);
            }
        })
    } else {
        Ok(quote! {
            pub fn #name(&mut self, #(#params),*) -> #ret_ty {
                self._super.#name(#(#param_names),*)
            }
        })
    }
}

fn emit_constructor(ctor: &IrConstructor, cls: &IrClass) -> Result<TokenStream, CodegenError> {
    let params = emit_params(&ctor.params);

    // Split body into: optional super() call, then remaining statements.
    let (super_args, rest_stmts): (Option<Vec<IrExpr>>, Vec<IrStmt>) = split_super_call(&ctor.body);

    // Build the struct initializer.
    let super_field_init: Option<TokenStream> = if let Some(parent_name) = &cls.superclass {
        let parent_ident = ident(parent_name);
        let super_arg_ts: Vec<TokenStream> = super_args
            .as_ref()
            .map(|args| args.iter().map(emit_expr).collect::<Result<Vec<_>, _>>())
            .transpose()?
            .unwrap_or_default();
        Some(quote! { _super: #parent_ident::new(#(#super_arg_ts),*), })
    } else {
        None
    };

    // Own field initializers (zero/default values — the constructor body will
    // overwrite them via `__self__.field = ...` assignments below).
    let own_field_inits: Vec<TokenStream> = cls
        .fields
        .iter()
        .filter(|f| !f.is_static)
        .map(|f| {
            let fname = ident(&f.name);
            let default_val = field_default_val(&f.ty);
            quote! { #fname: #default_val, }
        })
        .collect();

    // Use `Self` so this works for both plain and generic structs.
    let struct_init = quote! {
        let mut __self__: Self = Self {
            #super_field_init
            #(#own_field_inits)*
        };
    };

    let body = emit_stmts(&rest_stmts)?;

    Ok(quote! {
        pub fn new(#(#params),*) -> Self {
            #struct_init
            #(#body)*
            __self__
        }
    })
}

/// Return the default / zero value for a type, used in constructor struct init.
fn field_default_val(ty: &IrType) -> TokenStream {
    match ty {
        IrType::Bool => quote! { false },
        IrType::Byte | IrType::Short | IrType::Int | IrType::Long => quote! { 0 },
        IrType::Float | IrType::Double => quote! { 0.0 },
        IrType::Char => quote! { '\0' },
        IrType::String => quote! { JString::from("") },
        _ => quote! { Default::default() },
    }
}

/// Pull the first `SuperConstructorCall` out of a constructor body, returning
/// `(super_args, remaining_stmts)`.
fn split_super_call(stmts: &[IrStmt]) -> (Option<Vec<IrExpr>>, Vec<IrStmt>) {
    if let Some(IrStmt::SuperConstructorCall { args }) = stmts.first() {
        (Some(args.clone()), stmts[1..].to_vec())
    } else {
        (None, stmts.to_vec())
    }
}

fn emit_method(method: &IrMethod, _class_name: &str) -> Result<TokenStream, CodegenError> {
    emit_method_with_pub(method, true)
}

fn emit_trait_method(method: &IrMethod) -> Result<TokenStream, CodegenError> {
    emit_method_with_pub(method, false)
}

fn emit_method_with_pub(method: &IrMethod, pub_vis: bool) -> Result<TokenStream, CodegenError> {
    let name = ident(&method.name);
    let params = emit_params(&method.params);
    let ret_ty = emit_type(&method.return_ty);

    let self_param = if method.is_static {
        quote! {}
    } else {
        quote! { &mut self, }
    };

    let body = match &method.body {
        Some(stmts) => emit_stmts(stmts)?,
        None => return Ok(quote! {}), // abstract
    };

    let ret_clause = if method.return_ty == IrType::Void {
        quote! {}
    } else {
        quote! { -> #ret_ty }
    };

    if pub_vis {
        Ok(quote! {
            pub fn #name(#self_param #(#params),*) #ret_clause {
                #(#body)*
            }
        })
    } else {
        Ok(quote! {
            fn #name(#self_param #(#params),*) #ret_clause {
                #(#body)*
            }
        })
    }
}

fn emit_params(params: &[IrParam]) -> Vec<TokenStream> {
    params
        .iter()
        .map(|p| {
            let name = ident(&p.name);
            let ty = emit_type(&p.ty);
            quote! { mut #name: #ty }
        })
        .collect()
}

/// Like `emit_params` but without `mut` — required for trait method signatures
/// (Rust does not allow patterns in function declarations without bodies).
fn emit_params_sig(params: &[IrParam]) -> Vec<TokenStream> {
    params
        .iter()
        .map(|p| {
            let name = ident(&p.name);
            let ty = emit_type(&p.ty);
            quote! { #name: #ty }
        })
        .collect()
}

// ─── statements ─────────────────────────────────────────────────────────────

fn emit_stmts(stmts: &[IrStmt]) -> Result<Vec<TokenStream>, CodegenError> {
    stmts.iter().map(emit_stmt).collect()
}

/// Returns true if `stmts` (at the outermost level, not inside nested loops)
/// contains a bare `continue` that targets the enclosing for-loop.
fn for_body_has_continue(stmts: &[IrStmt]) -> bool {
    stmts.iter().any(for_stmt_has_continue)
}

fn for_stmt_has_continue(stmt: &IrStmt) -> bool {
    match stmt {
        IrStmt::Continue(_) => true,
        IrStmt::If { then_, else_, .. } => {
            for_body_has_continue(then_)
                || else_.as_deref().map(for_body_has_continue).unwrap_or(false)
        }
        IrStmt::Block(inner) => for_body_has_continue(inner),
        // Nested loops own their own breaks/continues — don't recurse.
        IrStmt::While { .. }
        | IrStmt::For { .. }
        | IrStmt::DoWhile { .. }
        | IrStmt::ForEach { .. } => false,
        _ => false,
    }
}

/// Re-emit `stmts` for use inside a labeled for-body loop, replacing bare
/// `continue` → `break 'for_body` and bare `break` → `break 'for_loop`.
fn transform_for_body(stmts: &[IrStmt]) -> Result<Vec<TokenStream>, CodegenError> {
    stmts.iter().map(transform_for_body_stmt).collect()
}

fn transform_for_body_stmt(stmt: &IrStmt) -> Result<TokenStream, CodegenError> {
    match stmt {
        IrStmt::Continue(_) => Ok(quote! { break 'for_body; }),
        IrStmt::Break(_) => Ok(quote! { break 'for_loop; }),
        IrStmt::If { cond, then_, else_ } => {
            let cond_ts = emit_expr(cond)?;
            let then_ts = transform_for_body(then_)?;
            if let Some(else_stmts) = else_ {
                let else_ts = transform_for_body(else_stmts)?;
                Ok(quote! { if #cond_ts { #(#then_ts)* } else { #(#else_ts)* } })
            } else {
                Ok(quote! { if #cond_ts { #(#then_ts)* } })
            }
        }
        IrStmt::Block(inner) => {
            let inner_ts = transform_for_body(inner)?;
            Ok(quote! { { #(#inner_ts)* } })
        }
        // Nested loops keep their own semantics.
        _ => emit_stmt(stmt),
    }
}

fn emit_stmt(stmt: &IrStmt) -> Result<TokenStream, CodegenError> {
    match stmt {
        IrStmt::LocalVar { name, ty, init } => {
            let n = ident(name);
            let t = emit_type(ty);
            if let Some(init_expr) = init {
                let val = emit_expr(init_expr)?;
                Ok(quote! { let mut #n: #t = #val; })
            } else {
                Ok(quote! { let mut #n: #t; })
            }
        }
        IrStmt::Expr(e) => {
            let ts = emit_expr(e)?;
            Ok(quote! { #ts; })
        }
        IrStmt::Return(Some(e)) => {
            let ts = emit_expr(e)?;
            Ok(quote! { return #ts; })
        }
        IrStmt::Return(None) => Ok(quote! { return; }),
        IrStmt::If { cond, then_, else_ } => {
            let cond_ts = emit_expr(cond)?;
            let then_ts = emit_stmts(then_)?;
            if let Some(else_stmts) = else_ {
                let else_ts = emit_stmts(else_stmts)?;
                Ok(quote! {
                    if #cond_ts { #(#then_ts)* } else { #(#else_ts)* }
                })
            } else {
                Ok(quote! {
                    if #cond_ts { #(#then_ts)* }
                })
            }
        }
        IrStmt::While { cond, body } => {
            let cond_ts = emit_expr(cond)?;
            let body_ts = emit_stmts(body)?;
            Ok(quote! { while #cond_ts { #(#body_ts)* } })
        }
        IrStmt::DoWhile { body, cond } => {
            let body_ts = emit_stmts(body)?;
            let cond_ts = emit_expr(cond)?;
            Ok(quote! {
                loop {
                    #(#body_ts)*
                    if !(#cond_ts) { break; }
                }
            })
        }
        IrStmt::For {
            init,
            cond,
            update,
            body,
        } => {
            let init_ts = init
                .as_ref()
                .map(|s| emit_stmt(s))
                .transpose()?
                .unwrap_or_default();
            let cond_ts = cond
                .as_ref()
                .map(emit_expr)
                .transpose()?
                .unwrap_or_else(|| quote! { true });
            let update_ts: Vec<TokenStream> = update
                .iter()
                .map(|u| emit_expr(u).map(|ts| quote! { #ts; }))
                .collect::<Result<_, _>>()?;
            // If the body contains a bare `continue`, a simple while-loop
            // desugaring is wrong: the update would be skipped. Use labeled
            // loops so that `continue` (→ `break 'for_body`) still runs the
            // update before the next iteration.
            if for_body_has_continue(body) {
                let body_ts = transform_for_body(body)?;
                Ok(quote! {
                    {
                        #init_ts
                        'for_loop: loop {
                            if !(#cond_ts) { break 'for_loop; }
                            'for_body: loop {
                                #(#body_ts)*
                                break 'for_body;
                            }
                            #(#update_ts)*
                        }
                    }
                })
            } else {
                let body_ts = emit_stmts(body)?;
                Ok(quote! {
                    {
                        #init_ts
                        while #cond_ts {
                            #(#body_ts)*
                            #(#update_ts)*
                        }
                    }
                })
            }
        }
        IrStmt::ForEach {
            var,
            var_ty,
            iterable,
            body,
        } => {
            let v = ident(var);
            let ty = emit_type(var_ty);
            let iter_ts = emit_expr(iterable)?;
            let body_ts = emit_stmts(body)?;
            Ok(quote! {
                for #v in #iter_ts.iter() {
                    let #v: #ty = #v.clone();
                    #(#body_ts)*
                }
            })
        }
        IrStmt::Switch {
            expr,
            cases,
            default,
        } => {
            let expr_ts = emit_expr(expr)?;
            let case_arms = cases
                .iter()
                .map(emit_switch_case)
                .collect::<Result<Vec<_>, _>>()?;
            let default_arm = if let Some(def) = default {
                let def_ts = emit_stmts(def)?;
                quote! { _ => { #(#def_ts)* } }
            } else {
                quote! { _ => {} }
            };
            Ok(quote! {
                match #expr_ts {
                    #(#case_arms)*
                    #default_arm
                }
            })
        }
        IrStmt::Break(_) => Ok(quote! { break; }),
        IrStmt::Continue(_) => Ok(quote! { continue; }),
        IrStmt::Throw(e) => {
            let ts = emit_expr(e)?;
            Ok(quote! { panic!("{:?}", #ts); })
        }
        IrStmt::SuperConstructorCall { .. } => {
            // Already consumed by emit_constructor; should not appear here.
            Ok(quote! {})
        }
        IrStmt::TryCatch { body, .. } => {
            // Stage 1: emit body only; full exception handling is Stage 4
            let body_ts = emit_stmts(body)?;
            Ok(quote! {
                {
                    #(#body_ts)*
                }
            })
        }
        IrStmt::Block(stmts) => {
            let ts = emit_stmts(stmts)?;
            Ok(quote! { { #(#ts)* } })
        }
    }
}

fn emit_switch_case(case: &SwitchCase) -> Result<TokenStream, CodegenError> {
    let val = emit_expr(&case.value)?;
    let body = emit_stmts(&case.body)?;
    Ok(quote! {
        #val => { #(#body)* }
    })
}

// ─── expressions ─────────────────────────────────────────────────────────────

/// Emit an expression as a place (lvalue) — never adds `.clone()`.
fn emit_place(expr: &IrExpr) -> Result<TokenStream, CodegenError> {
    match expr {
        IrExpr::FieldAccess {
            receiver,
            field_name,
            ..
        } => {
            if let IrExpr::Var { name, .. } = receiver.as_ref() {
                if name == "System" {
                    let stream = ident(field_name);
                    return Ok(quote! { std::io::#stream });
                }
            }
            let recv = emit_expr(receiver)?;
            let field = ident(field_name);
            Ok(quote! { (#recv).#field })
        }
        _ => emit_expr(expr),
    }
}

fn emit_expr(expr: &IrExpr) -> Result<TokenStream, CodegenError> {
    match expr {
        IrExpr::LitBool(b) => Ok(quote! { #b }),
        IrExpr::LitInt(n) => {
            let lit = Literal::i32_unsuffixed(*n as i32);
            Ok(quote! { #lit })
        }
        IrExpr::LitLong(n) => {
            let lit = Literal::i64_unsuffixed(*n);
            Ok(quote! { #lit })
        }
        IrExpr::LitFloat(f) => {
            let lit = Literal::f64_unsuffixed(*f);
            Ok(quote! { #lit })
        }
        IrExpr::LitDouble(f) => {
            let lit = Literal::f64_unsuffixed(*f);
            Ok(quote! { #lit })
        }
        IrExpr::LitChar(c) => Ok(quote! { #c }),
        IrExpr::LitString(s) => Ok(quote! { JString::from(#s) }),
        IrExpr::LitNull => Ok(quote! { None }),

        IrExpr::Var { name, .. } => {
            let id = ident(name);
            Ok(quote! { #id })
        }

        IrExpr::FieldAccess {
            receiver,
            field_name,
            ty,
        } => {
            // System.out / System.err — emit as the receiver for println calls
            if let IrExpr::Var { name, .. } = receiver.as_ref() {
                if name == "System" && (field_name == "out" || field_name == "err") {
                    // Will be consumed by a MethodCall — return a placeholder
                    let stream = ident(field_name);
                    return Ok(quote! { std::io::#stream });
                }
            }
            let recv = emit_expr(receiver)?;
            // array.length — check receiver is an array type
            if field_name == "length" {
                if let IrType::Array(_) = receiver.ty() {
                    return Ok(quote! { (#recv).length() });
                }
            }
            let field = ident(field_name);
            // Non-Copy types must be cloned when read as a value.
            match ty {
                IrType::String
                | IrType::Class(_)
                | IrType::TypeVar(_)
                | IrType::Generic { .. }
                | IrType::Array(_) => Ok(quote! { (#recv).#field.clone() }),
                _ => Ok(quote! { (#recv).#field }),
            }
        }

        IrExpr::MethodCall {
            receiver,
            method_name,
            args,
            ..
        } => {
            let args_ts: Vec<TokenStream> = args.iter().map(emit_expr).collect::<Result<_, _>>()?;

            // System.out.println(x) → println!("{}", x)  /  System.err.println → eprintln!
            if let Some(recv) = receiver {
                if let IrExpr::FieldAccess {
                    receiver: inner_recv,
                    field_name,
                    ..
                } = recv.as_ref()
                {
                    if let IrExpr::Var { name, .. } = inner_recv.as_ref() {
                        if name == "System" {
                            let macro_name = if field_name == "out" {
                                "println"
                            } else {
                                "eprintln"
                            };
                            return emit_print_call(
                                macro_name,
                                method_name,
                                &args_ts,
                                args.first().map(|e| e.ty()),
                            );
                        }
                    }
                }
            }

            let method = ident(method_name);
            match receiver {
                None => Ok(quote! { Self::#method(#(#args_ts),*) }),
                Some(recv) => {
                    let recv_ts = emit_expr(recv)?;
                    Ok(quote! { (#recv_ts).#method(#(#args_ts),*) })
                }
            }
        }

        IrExpr::New { class, args, .. } => {
            let args_ts: Vec<TokenStream> = args.iter().map(emit_expr).collect::<Result<_, _>>()?;
            // Map Java collection constructors to their JList/JMap/JSet equivalents.
            match class.as_str() {
                "ArrayList" | "LinkedList" | "ArrayDeque" => Ok(quote! { JList::new() }),
                "HashMap" | "LinkedHashMap" | "TreeMap" | "Hashtable" => {
                    Ok(quote! { JMap::new() })
                }
                "HashSet" | "LinkedHashSet" | "TreeSet" => Ok(quote! { JSet::new() }),
                _ => {
                    let cls = ident(class);
                    Ok(quote! { #cls::new(#(#args_ts),*) })
                }
            }
        }

        IrExpr::NewArray { elem_ty, len, .. } => {
            let len_ts = emit_expr(len)?;
            let elem_ts = emit_type(elem_ty);
            Ok(quote! { JArray::<#elem_ts>::new_default(#len_ts) })
        }

        IrExpr::ArrayAccess { array, index, .. } => {
            let arr = emit_expr(array)?;
            let idx = emit_expr(index)?;
            Ok(quote! { (#arr).get(#idx) })
        }

        IrExpr::BinOp { op, lhs, rhs, ty } => {
            let l = emit_expr(lhs)?;
            let r = emit_expr(rhs)?;
            // String concatenation → + operator on JString
            if matches!(op, BinOp::Add | BinOp::Concat)
                && (ty == &IrType::String
                    || lhs.ty() == &IrType::String
                    || rhs.ty() == &IrType::String)
            {
                // coerce non-strings to JString
                let l = coerce_to_jstring(lhs, l)?;
                let r = coerce_to_jstring(rhs, r)?;
                return Ok(quote! { (#l + #r) });
            }
            let op_ts = emit_binop(op);
            Ok(quote! { (#l #op_ts #r) })
        }

        IrExpr::UnOp { op, operand, .. } => {
            let operand_ts = emit_expr(operand)?;
            match op {
                UnOp::Neg => Ok(quote! { (-#operand_ts) }),
                UnOp::Not => Ok(quote! { (!#operand_ts) }),
                UnOp::BitNot => Ok(quote! { (!#operand_ts) }),
                UnOp::PreInc => Ok(quote! { { #operand_ts += 1; #operand_ts } }),
                UnOp::PreDec => Ok(quote! { { #operand_ts -= 1; #operand_ts } }),
                UnOp::PostInc => Ok(quote! { { let _tmp = #operand_ts; #operand_ts += 1; _tmp } }),
                UnOp::PostDec => Ok(quote! { { let _tmp = #operand_ts; #operand_ts -= 1; _tmp } }),
            }
        }

        IrExpr::Ternary {
            cond, then_, else_, ..
        } => {
            let c = emit_expr(cond)?;
            let t = emit_expr(then_)?;
            let e = emit_expr(else_)?;
            Ok(quote! { if #c { #t } else { #e } })
        }

        IrExpr::Assign { lhs, rhs, .. } => {
            if let IrExpr::ArrayAccess { array, index, .. } = lhs.as_ref() {
                let arr = emit_expr(array)?;
                let idx = emit_expr(index)?;
                let val = emit_expr(rhs)?;
                return Ok(quote! { (#arr).set(#idx, #val) });
            }
            let l = emit_place(lhs)?;
            let r = emit_expr(rhs)?;
            Ok(quote! { #l = #r })
        }

        IrExpr::CompoundAssign { op, lhs, rhs, .. } => {
            let l = emit_place(lhs)?;
            let r = emit_expr(rhs)?;
            let compound = match op {
                BinOp::Add | BinOp::Concat => quote! { += },
                BinOp::Sub => quote! { -= },
                BinOp::Mul => quote! { *= },
                BinOp::Div => quote! { /= },
                BinOp::Rem => quote! { %= },
                BinOp::BitAnd => quote! { &= },
                BinOp::BitOr => quote! { |= },
                BinOp::BitXor => quote! { ^= },
                BinOp::Shl => quote! { <<= },
                BinOp::Shr => quote! { >>= },
                _ => {
                    return Err(CodegenError::Unsupported(format!(
                        "{op:?} is not a compound-assignment operator"
                    )))
                }
            };
            Ok(quote! { #l #compound #r })
        }

        IrExpr::Cast { target, expr } => {
            let inner = emit_expr(expr)?;
            let ty = emit_type(target);
            Ok(quote! { (#inner as #ty) })
        }

        IrExpr::InstanceOf { expr, check_type } => {
            let inner = emit_expr(expr)?;
            let type_name_str = match check_type {
                IrType::Class(name) => name.clone(),
                IrType::String => "String".to_string(),
                // instanceof with a primitive type is a Java compile error;
                // emit false as a safe fallback.
                _ => return Ok(quote! { { let _ = &#inner; false } }),
            };
            Ok(quote! { #inner._instanceof(#type_name_str) })
        }
    }
}

fn emit_print_call(
    macro_name: &str,
    method_name: &str,
    args: &[TokenStream],
    first_arg_ty: Option<&IrType>,
) -> Result<TokenStream, CodegenError> {
    let macro_ident = ident(macro_name);
    let is_float = matches!(first_arg_ty, Some(IrType::Double) | Some(IrType::Float));
    match method_name {
        "println" => {
            if args.is_empty() {
                Ok(quote! { #macro_ident!() })
            } else {
                let first = &args[0];
                if is_float {
                    Ok(quote! { #macro_ident!("{:?}", #first) })
                } else {
                    Ok(quote! { #macro_ident!("{}", #first) })
                }
            }
        }
        "print" => {
            if args.is_empty() {
                Ok(quote! { print!("") })
            } else {
                let first = &args[0];
                if is_float {
                    Ok(quote! { print!("{:?}", #first) })
                } else {
                    Ok(quote! { print!("{}", #first) })
                }
            }
        }
        "printf" | "format" => {
            if args.is_empty() {
                Ok(quote! { print!("") })
            } else {
                // Pass through args directly — best-effort
                Ok(quote! { print!(#(#args),*) })
            }
        }
        _ => Ok(quote! {}),
    }
}

fn coerce_to_jstring(expr: &IrExpr, ts: TokenStream) -> Result<TokenStream, CodegenError> {
    if expr.ty() == &IrType::String {
        Ok(ts)
    } else {
        Ok(quote! { JString::from(format!("{}", #ts).as_str()) })
    }
}

fn emit_binop(op: &BinOp) -> TokenStream {
    match op {
        BinOp::Add | BinOp::Concat => quote! { + },
        BinOp::Sub => quote! { - },
        BinOp::Mul => quote! { * },
        BinOp::Div => quote! { / },
        BinOp::Rem => quote! { % },
        BinOp::BitAnd => quote! { & },
        BinOp::BitOr => quote! { | },
        BinOp::BitXor => quote! { ^ },
        BinOp::Shl => quote! { << },
        BinOp::Shr => quote! { >> },
        BinOp::UShr => quote! { >> }, // Rust doesn't have unsigned shift on signed types; best-effort
        BinOp::And => quote! { && },
        BinOp::Or => quote! { || },
        BinOp::Eq => quote! { == },
        BinOp::Ne => quote! { != },
        BinOp::Lt => quote! { < },
        BinOp::Le => quote! { <= },
        BinOp::Gt => quote! { > },
        BinOp::Ge => quote! { >= },
    }
}

// ─── types ───────────────────────────────────────────────────────────────────

fn emit_type(ty: &IrType) -> TokenStream {
    match ty {
        IrType::Bool => quote! { bool },
        IrType::Byte => quote! { i8 },
        IrType::Short => quote! { i16 },
        IrType::Int => quote! { i32 },
        IrType::Long => quote! { i64 },
        IrType::Float => quote! { f32 },
        IrType::Double => quote! { f64 },
        IrType::Char => quote! { char },
        IrType::Void => quote! { () },
        IrType::String => quote! { JString },
        IrType::Null => quote! { Option<()> },
        IrType::Nullable(inner) => {
            let t = emit_type(inner);
            quote! { Option<#t> }
        }
        IrType::Array(elem) => {
            let t = emit_type(elem);
            quote! { JArray<#t> }
        }
        IrType::Class(name) => {
            // Map Java boxed / well-known types to their Rust equivalents.
            match name.as_str() {
                "Integer" => quote! { i32 },
                "Long" => quote! { i64 },
                "Double" => quote! { f64 },
                "Float" => quote! { f32 },
                "Boolean" => quote! { bool },
                "Character" => quote! { char },
                "Byte" => quote! { i8 },
                "Short" => quote! { i16 },
                "String" | "CharSequence" => quote! { JString },
                _ => {
                    let id = ident(name);
                    quote! { #id }
                }
            }
        }
        IrType::TypeVar(name) => {
            let id = ident(name);
            quote! { #id }
        }
        IrType::Generic { base, args } => {
            // Map Java collection generics to JList / JMap / JSet.
            if let IrType::Class(base_name) = base.as_ref() {
                match base_name.as_str() {
                    "List" | "ArrayList" | "LinkedList" | "Collection" | "Iterable"
                    | "ArrayDeque" => {
                        let a = emit_type(args.first().unwrap_or(&IrType::Unknown));
                        return quote! { JList<#a> };
                    }
                    "Map" | "HashMap" | "LinkedHashMap" | "TreeMap" | "Hashtable" => {
                        let k = emit_type(args.first().unwrap_or(&IrType::Unknown));
                        let v = emit_type(args.get(1).unwrap_or(&IrType::Unknown));
                        return quote! { JMap<#k, #v> };
                    }
                    "Set" | "HashSet" | "LinkedHashSet" | "TreeSet" => {
                        let a = emit_type(args.first().unwrap_or(&IrType::Unknown));
                        return quote! { JSet<#a> };
                    }
                    _ => {}
                }
            }
            // Passthrough for user-defined generics, e.g. Wrapper<T>.
            let b = emit_type(base);
            let a: Vec<TokenStream> = args.iter().map(emit_type).collect();
            quote! { #b<#(#a),*> }
        }
        IrType::Unknown => quote! { _ },
    }
}

// ─── helpers ─────────────────────────────────────────────────────────────────

fn ident(name: &str) -> Ident {
    // Sanitise name: replace illegal chars with _
    let sanitised: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    // Avoid clashing with Rust keywords
    let sanitised = match sanitised.as_str() {
        "type" => "type_".to_owned(),
        "move" => "move_".to_owned(),
        "loop" => "loop_".to_owned(),
        "match" => "match_".to_owned(),
        "use" => "use_".to_owned(),
        "ref" => "ref_".to_owned(),
        "struct" => "struct_".to_owned(),
        "trait" => "trait_".to_owned(),
        "impl" => "impl_".to_owned(),
        "fn" => "fn_".to_owned(),
        "in" => "in_".to_owned(),
        other => other.to_owned(),
    };
    Ident::new(&sanitised, Span::call_site())
}

fn as_const_literal(expr: &IrExpr) -> Option<TokenStream> {
    match expr {
        IrExpr::LitInt(n) => {
            let lit = Literal::i32_unsuffixed(*n as i32);
            Some(quote! { #lit })
        }
        IrExpr::LitLong(n) => {
            let lit = Literal::i64_unsuffixed(*n);
            Some(quote! { #lit })
        }
        IrExpr::LitBool(b) => Some(quote! { #b }),
        IrExpr::LitFloat(f) => {
            let lit = Literal::f64_unsuffixed(*f);
            Some(quote! { #lit })
        }
        IrExpr::LitDouble(f) => {
            let lit = Literal::f64_unsuffixed(*f);
            Some(quote! { #lit })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ir::IrModule;

    #[test]
    fn generate_empty_module() {
        let module = IrModule::new("");
        let result = generate(&module);
        assert!(result.is_ok(), "empty module should generate without error");
    }
}
