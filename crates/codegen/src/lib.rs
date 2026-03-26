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
        use java_compat::{
            JString, JArray, JObject, JNull, JList, JMap, JSet, JException,
            JAtomicInteger, JAtomicLong, JAtomicBoolean,
            JCountDownLatch, JSemaphore, JThread, JClass,
            JOptional, JStringBuilder, JBigInteger, JPattern, JMatcher,
            JLocalDate, JFile, JStream,
        };
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

    // If any method is `synchronized`, emit a static per-class monitor helper.
    if cls.methods.iter().any(|m| m.is_synchronized) {
        method_tokens.push(quote! {
            fn __sync_monitor() -> &'static (::std::sync::Mutex<()>, ::std::sync::Condvar) {
                static __M: ::std::sync::OnceLock<
                    (::std::sync::Mutex<()>, ::std::sync::Condvar),
                > = ::std::sync::OnceLock::new();
                __M.get_or_init(|| {
                    (::std::sync::Mutex::new(()), ::std::sync::Condvar::new())
                })
            }
        });
    }

    // `getClass()` — returns a compile-time JClass descriptor for this type.
    {
        let class_name_str = &cls.name;
        method_tokens.push(quote! {
            pub fn getClass(&self) -> JClass {
                JClass::new(#class_name_str)
            }
        });
    }

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

    // `impl Display` — auto-generated if the class defines a `toString()` method.
    // This enables use of the class in string-concatenation expressions and
    // `println!("{}", obj)` calls.
    let display_impl = if cls
        .methods
        .iter()
        .any(|m| m.name == "toString" && !m.is_static)
    {
        quote! {
            impl #impl_generics ::std::fmt::Display for #name #struct_generics {
                fn fmt(
                    &self,
                    f: &mut ::std::fmt::Formatter<'_>,
                ) -> ::std::fmt::Result {
                    write!(f, "{}", self.clone().toString())
                }
            }
        }
    } else {
        quote! {}
    };

    Ok(quote! {
        #struct_def
        #(#static_items)*
        #impl_block
        #display_impl
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
        IrType::Atomic(inner) => match inner.as_ref() {
            IrType::Int => quote! {
                ::std::sync::Arc::new(::std::sync::atomic::AtomicI32::new(0))
            },
            IrType::Long => quote! {
                ::std::sync::Arc::new(::std::sync::atomic::AtomicI64::new(0))
            },
            IrType::Bool => quote! {
                ::std::sync::Arc::new(::std::sync::atomic::AtomicBool::new(false))
            },
            _ => quote! { Default::default() },
        },
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

    // Preamble for `synchronized` instance methods: acquire per-class monitor.
    let sync_preamble = if method.is_synchronized && !method.is_static {
        quote! {
            let (__sync_lock, __sync_cond) = Self::__sync_monitor();
            let mut __sync_guard = __sync_lock.lock().unwrap();
            let _ = &__sync_guard;
        }
    } else {
        quote! {}
    };

    let ret_clause = if method.return_ty == IrType::Void {
        quote! {}
    } else {
        quote! { -> #ret_ty }
    };

    if pub_vis {
        Ok(quote! {
            pub fn #name(#self_param #(#params),*) #ret_clause {
                #sync_preamble
                #(#body)*
            }
        })
    } else {
        Ok(quote! {
            fn #name(#self_param #(#params),*) #ret_clause {
                #sync_preamble
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
        IrStmt::Throw(e) => emit_throw(e),
        IrStmt::SuperConstructorCall { .. } => {
            // Already consumed by emit_constructor; should not appear here.
            Ok(quote! {})
        }
        IrStmt::TryCatch {
            body,
            catches,
            finally,
        } => {
            let body_stmts = emit_stmts(body)?;
            let catch_chain = emit_catch_chain(catches)?;
            let finally_block = if let Some(fin) = finally {
                let fs = emit_stmts(fin)?;
                quote! { #(#fs)* }
            } else {
                quote! {}
            };
            Ok(quote! {
                let __try_result =
                    ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                        #(#body_stmts)*
                    }));
                let __rethrow = match __try_result {
                    Ok(_) => None,
                    Err(__panic_val) => {
                        let __ex_opt =
                            JException::from_panic_payload(&__panic_val);
                        if let Some(ref __ex) = __ex_opt {
                            #catch_chain
                        } else {
                            Some(__panic_val)
                        }
                    }
                };
                #finally_block
                if let Some(__ex) = __rethrow {
                    ::std::panic::resume_unwind(__ex);
                }
            })
        }
        IrStmt::Block(stmts) => {
            let ts = emit_stmts(stmts)?;
            Ok(quote! { { #(#ts)* } })
        }
        IrStmt::Synchronized { body, .. } => {
            // Use the process-global monitor for synchronized blocks.
            // The monitor expression is evaluated but not used for locking
            // (single-class files do not need per-object granularity).
            let body_ts = emit_stmts(body)?;
            Ok(quote! {
                {
                    let (__sync_lock, __sync_cond) = java_compat::__sync_block_monitor();
                    let mut __sync_guard = __sync_lock.lock().unwrap();
                    let _ = &__sync_guard;
                    { #(#body_ts)* }
                    drop(__sync_guard);
                }
            })
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

/// Emit a `throw` statement.
///
/// `throw new SomeException("msg")` → `panic!("JException:SomeException:{msg}")`
/// `throw e` (rethrow of caught var) → `panic!("{}", e.to_panic_string())`
fn emit_throw(e: &IrExpr) -> Result<TokenStream, CodegenError> {
    match e {
        IrExpr::New { class, args, .. } => {
            let class_str = class.as_str();
            if let Some(msg_expr) = args.first() {
                let msg_ts = emit_expr(msg_expr)?;
                Ok(quote! { panic!("JException:{}:{}", #class_str, #msg_ts) })
            } else {
                Ok(quote! { panic!("JException:{}:", #class_str) })
            }
        }
        IrExpr::Var { name, .. } => {
            let var_id = ident(name);
            Ok(quote! { panic!("{}", #var_id.to_panic_string()) })
        }
        _ => {
            let ts = emit_expr(e)?;
            Ok(quote! { panic!("{}", #ts) })
        }
    }
}

/// Build a nested if-else chain that matches a `JException` against catch
/// clauses, returning `None` if handled or `Some(__panic_val)` to rethrow.
fn emit_catch_chain(catches: &[ir::stmt::CatchClause]) -> Result<TokenStream, CodegenError> {
    // Start with the "no match → rethrow" base case.
    let mut chain = quote! { Some(__panic_val) };

    for catch in catches.iter().rev() {
        let var = ident(&catch.var);
        let catch_stmts = emit_stmts(&catch.body)?;

        let cond = if catch
            .exception_types
            .iter()
            .any(|t| matches!(t.as_str(), "Throwable" | "Exception"))
        {
            // Catch-all
            quote! { true }
        } else {
            let checks: Vec<TokenStream> = catch
                .exception_types
                .iter()
                .map(|t| quote! { __ex.is_instance_of(#t) })
                .collect();
            quote! { #(#checks)||* }
        };

        chain = quote! {
            if #cond {
                let #var = __ex.clone();
                { #(#catch_stmts)* }
                None
            } else {
                #chain
            }
        };
    }

    Ok(chain)
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
            // Volatile field read: emit .load(SeqCst) instead of plain field access.
            if matches!(ty, IrType::Atomic(_)) {
                let recv = emit_expr(receiver)?;
                let field = ident(field_name);
                return Ok(quote! {
                    (#recv).#field.load(::std::sync::atomic::Ordering::SeqCst)
                });
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

                // Thread.sleep(ms) — static call on the Thread class name.
                if let IrExpr::Var { name, .. } = recv.as_ref() {
                    if name == "Thread" && method_name == "sleep" {
                        return Ok(quote! { JThread::sleep(#(#args_ts),*) });
                    }
                    // Math.x(...) — static Math methods → f64 method calls or std ops.
                    if name == "Math" {
                        let first_ty = args.first().map(|e| e.ty().clone());
                        let is_double = matches!(first_ty.as_ref(), Some(IrType::Double) | Some(IrType::Float));
                        let is_long = matches!(first_ty.as_ref(), Some(IrType::Long));
                        return match method_name.as_str() {
                            "abs" => {
                                let a = &args_ts[0];
                                if is_double {
                                    Ok(quote! { (#a as f64).abs() })
                                } else if is_long {
                                    Ok(quote! { (#a as i64).abs() })
                                } else {
                                    Ok(quote! { (#a as i32).abs() })
                                }
                            }
                            "max" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                if is_double {
                                    Ok(quote! { { let __a = #a as f64; let __b = #b as f64; if __a > __b { __a } else { __b } } })
                                } else {
                                    Ok(quote! { { let __a = #a as i32; let __b = #b as i32; if __a > __b { __a } else { __b } } })
                                }
                            }
                            "min" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                if is_double {
                                    Ok(quote! { { let __a = #a as f64; let __b = #b as f64; if __a < __b { __a } else { __b } } })
                                } else {
                                    Ok(quote! { { let __a = #a as i32; let __b = #b as i32; if __a < __b { __a } else { __b } } })
                                }
                            }
                            "pow" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { (#a as f64).powf(#b as f64) })
                            }
                            "sqrt" => { let a = &args_ts[0]; Ok(quote! { (#a as f64).sqrt() }) }
                            "floor" => { let a = &args_ts[0]; Ok(quote! { (#a as f64).floor() }) }
                            "ceil" => { let a = &args_ts[0]; Ok(quote! { (#a as f64).ceil() }) }
                            "round" => { let a = &args_ts[0]; Ok(quote! { (#a as f64).round() as i64 }) }
                            "log" => { let a = &args_ts[0]; Ok(quote! { (#a as f64).ln() }) }
                            "log10" => { let a = &args_ts[0]; Ok(quote! { (#a as f64).log10() }) }
                            "sin" => { let a = &args_ts[0]; Ok(quote! { (#a as f64).sin() }) }
                            "cos" => { let a = &args_ts[0]; Ok(quote! { (#a as f64).cos() }) }
                            "tan" => { let a = &args_ts[0]; Ok(quote! { (#a as f64).tan() }) }
                            "exp" => { let a = &args_ts[0]; Ok(quote! { (#a as f64).exp() }) }
                            "random" => Ok(quote! { 0.0_f64 }),
                            "PI" => Ok(quote! { std::f64::consts::PI }),
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { (#(#args_ts),* as f64).#m() })
                            }
                        };
                    }
                    // Optional.of(...) / Optional.empty() / Optional.ofNullable(...)
                    if name == "Optional" {
                        return match method_name.as_str() {
                            "of" => Ok(quote! { JOptional::of(#(#args_ts),*) }),
                            "empty" => Ok(quote! { JOptional::empty() }),
                            "ofNullable" => Ok(quote! { JOptional::of_nullable(#(#args_ts),*) }),
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JOptional::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // Pattern.compile(...) / Pattern.matches(...)
                    if name == "Pattern" {
                        return match method_name.as_str() {
                            "compile" => Ok(quote! { JPattern::compile(#(#args_ts),*) }),
                            "matches" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { JPattern::static_matches(#a, #b) })
                            }
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JPattern::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // LocalDate.of(...) / LocalDate.now()
                    if name == "LocalDate" {
                        return match method_name.as_str() {
                            "of" => Ok(quote! { JLocalDate::of(#(#args_ts),*) }),
                            "now" => Ok(quote! { JLocalDate::now() }),
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JLocalDate::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // BigInteger.valueOf(long)
                    if name == "BigInteger" {
                        return match method_name.as_str() {
                            "valueOf" => Ok(quote! { JBigInteger::from_long(#(#args_ts),*) }),
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JBigInteger::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // Integer.parseInt / Integer.valueOf
                    if name == "Integer" {
                        return match method_name.as_str() {
                            "parseInt" | "valueOf" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a).as_str().parse::<i32>().unwrap() })
                            }
                            "toString" => {
                                let a = &args_ts[0];
                                Ok(quote! { JString::from(format!("{}", #a).as_str()) })
                            }
                            _ => Ok(quote! { 0i32 }),
                        };
                    }
                    // String.valueOf
                    if name == "String" {
                        return match method_name.as_str() {
                            "valueOf" => {
                                let a = &args_ts[0];
                                Ok(quote! { JString::from(format!("{}", #a).as_str()) })
                            }
                            _ => Ok(quote! { JString::from("") }),
                        };
                    }
                }

                // `.collect(Collectors.toList())` → `.collect_to_list()`
                if method_name == "collect" {
                    if let Some(arg) = args.first() {
                        if is_collectors_to_list(arg) {
                            let recv_ts = emit_expr(recv)?;
                            return Ok(quote! { (#recv_ts).collect_to_list() });
                        }
                    }
                }

                let recv_ts = emit_expr(recv)?;
                // Rename `await` → `await_` to avoid collision with the Rust keyword.
                // Also rename `mod` → `mod_` (Rust keyword).
                let method = match method_name.as_str() {
                    "await" => ident("await_"),
                    "mod" => ident("mod_"),
                    _ => ident(method_name),
                };
                return Ok(quote! { (#recv_ts).#method(#(#args_ts),*) });
            }

            // No receiver — unqualified call.
            // wait() / notifyAll() / notify() are special inside synchronized methods.
            match method_name.as_str() {
                "wait" => {
                    return Ok(quote! { __sync_guard = __sync_cond.wait(__sync_guard).unwrap() });
                }
                "notifyAll" => {
                    return Ok(quote! { __sync_cond.notify_all() });
                }
                "notify" => {
                    return Ok(quote! { __sync_cond.notify_one() });
                }
                _ => {}
            }

            let method = ident(method_name);
            Ok(quote! { Self::#method(#(#args_ts),*) })
        }

        IrExpr::New { class, args, .. } => {
            let args_ts: Vec<TokenStream> = args.iter().map(emit_expr).collect::<Result<_, _>>()?;
            // Map Java constructors to their runtime equivalents.
            match class.as_str() {
                "ArrayList" | "LinkedList" | "ArrayDeque" => Ok(quote! { JList::new() }),
                "HashMap" | "LinkedHashMap" | "TreeMap" | "Hashtable" => Ok(quote! { JMap::new() }),
                "HashSet" | "LinkedHashSet" | "TreeSet" => Ok(quote! { JSet::new() }),
                "AtomicInteger" => Ok(quote! { JAtomicInteger::new(#(#args_ts),*) }),
                "AtomicLong" => Ok(quote! { JAtomicLong::new(#(#args_ts),*) }),
                "AtomicBoolean" => Ok(quote! { JAtomicBoolean::new(#(#args_ts),*) }),
                "CountDownLatch" => Ok(quote! { JCountDownLatch::new(#(#args_ts),*) }),
                "Semaphore" => Ok(quote! { JSemaphore::new(#(#args_ts),*) }),
                "StringBuilder" => {
                    if args_ts.is_empty() {
                        Ok(quote! { JStringBuilder::new() })
                    } else {
                        Ok(quote! { JStringBuilder::new_from_string(#(#args_ts),*) })
                    }
                }
                "BigInteger" => {
                    Ok(quote! { JBigInteger::from_string(#(#args_ts),*) })
                }
                "File" => {
                    if args_ts.len() == 2 {
                        let a = &args_ts[0];
                        let b = &args_ts[1];
                        Ok(quote! { JFile::new_child(#a, #b) })
                    } else {
                        Ok(quote! { JFile::new(#(#args_ts),*) })
                    }
                }
                "Thread" => {
                    // new Thread(runnable) → JThread wrapping a move-closure that calls run()
                    if let Some(runnable_ts) = args_ts.first() {
                        Ok(quote! {
                            JThread::new({
                                let mut __r = #runnable_ts;
                                move || { (__r).run(); }
                            })
                        })
                    } else {
                        Ok(quote! { JThread::new(|| {}) })
                    }
                }
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
            // Volatile field write: emit .store(val, SeqCst) instead of assignment.
            if let IrExpr::FieldAccess {
                receiver,
                field_name,
                ty,
            } = lhs.as_ref()
            {
                if matches!(ty, IrType::Atomic(_)) {
                    let recv = emit_expr(receiver)?;
                    let field = ident(field_name);
                    let val = emit_expr(rhs)?;
                    return Ok(quote! {
                        (#recv).#field.store(#val, ::std::sync::atomic::Ordering::SeqCst)
                    });
                }
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

        IrExpr::Lambda { params, body, .. } => {
            let param_idents: Vec<Ident> = params.iter().map(|p| ident(p)).collect();
            let body_ts = emit_expr(body)?;
            if param_idents.len() == 1 {
                let p = &param_idents[0];
                Ok(quote! { |#p| { #body_ts } })
            } else {
                Ok(quote! { |#(#param_idents),*| { #body_ts } })
            }
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
                "AtomicInteger" => quote! { JAtomicInteger },
                "AtomicLong" => quote! { JAtomicLong },
                "AtomicBoolean" => quote! { JAtomicBoolean },
                "CountDownLatch" => quote! { JCountDownLatch },
                "Semaphore" => quote! { JSemaphore },
                "Thread" => quote! { JThread },
                "Optional" => quote! { JOptional },
                "StringBuilder" => quote! { JStringBuilder },
                "BigInteger" => quote! { JBigInteger },
                "Pattern" => quote! { JPattern },
                "Matcher" => quote! { JMatcher },
                "LocalDate" => quote! { JLocalDate },
                "File" => quote! { JFile },
                "JStream" => quote! { JStream },
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
                    "Optional" => {
                        let a = emit_type(args.first().unwrap_or(&IrType::Unknown));
                        return quote! { JOptional<#a> };
                    }
                    "Stream" => {
                        let a = emit_type(args.first().unwrap_or(&IrType::Unknown));
                        return quote! { JStream<#a> };
                    }
                    _ => {}
                }
            }
            // Passthrough for user-defined generics, e.g. Wrapper<T>.
            let b = emit_type(base);
            let a: Vec<TokenStream> = args.iter().map(emit_type).collect();
            quote! { #b<#(#a),*> }
        }
        IrType::Atomic(inner) => match inner.as_ref() {
            IrType::Int => {
                quote! { ::std::sync::Arc<::std::sync::atomic::AtomicI32> }
            }
            IrType::Long => {
                quote! { ::std::sync::Arc<::std::sync::atomic::AtomicI64> }
            }
            IrType::Bool => {
                quote! { ::std::sync::Arc<::std::sync::atomic::AtomicBool> }
            }
            other => {
                let t = emit_type(other);
                quote! { ::std::sync::Arc<#t> }
            }
        },
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
        "await" => "await_".to_owned(),
        "async" => "async_".to_owned(),
        other => other.to_owned(),
    };
    Ident::new(&sanitised, Span::call_site())
}

/// Check if an expression is `Collectors.toList()` (a MethodCall on `Collectors` with name `toList`).
fn is_collectors_to_list(expr: &IrExpr) -> bool {
    if let IrExpr::MethodCall {
        receiver: Some(recv),
        method_name,
        ..
    } = expr
    {
        if method_name == "toList" {
            if let IrExpr::Var { name, .. } = recv.as_ref() {
                return name == "Collectors";
            }
        }
    }
    false
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
