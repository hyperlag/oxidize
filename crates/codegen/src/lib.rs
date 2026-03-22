//! Code-generation pass: lowers a typed [`ir::IrModule`] to a Rust source
//! string using `proc-macro2` and `quote`.

use ir::{
    decl::{IrClass, IrConstructor, IrMethod, IrParam},
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
        use java_compat::{JString, JArray, JObject, JNull};
    });

    // Collect class names so we know what to emit a main() for
    let mut main_class: Option<String> = None;

    for decl in &module.decls {
        match decl {
            IrDecl::Class(cls) => {
                if cls.methods.iter().any(|m| m.name == "main" && m.is_static) {
                    main_class = Some(cls.name.clone());
                }
                items.push(emit_class(cls)?);
            }
            IrDecl::Interface(_) => {} // Stage 2
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

// ─── class ──────────────────────────────────────────────────────────────────

fn emit_class(cls: &IrClass) -> Result<TokenStream, CodegenError> {
    let name = ident(&cls.name);

    // Fields → struct fields
    let struct_fields: Vec<TokenStream> = cls
        .fields
        .iter()
        .filter(|f| !f.is_static)
        .map(|f| {
            let fname = ident(&f.name);
            let fty = emit_type(&f.ty);
            quote! { pub #fname: #fty, }
        })
        .collect();

    let struct_def = if struct_fields.is_empty() {
        quote! {
            #[derive(Debug, Clone)]
            pub struct #name;
        }
    } else {
        quote! {
            #[derive(Debug, Clone)]
            pub struct #name {
                #(#struct_fields)*
            }
        }
    };

    // Static fields → const / static items
    let static_items: Vec<TokenStream> = cls
        .fields
        .iter()
        .filter(|f| f.is_static)
        .filter_map(|f| {
            if f.is_final {
                // emit as a const — only for literal initialisers
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

    // Methods
    let mut method_tokens: Vec<TokenStream> = Vec::new();

    // Default constructor if no constructors defined and there are fields
    if cls.constructors.is_empty() && !struct_fields.is_empty() {
        // nothing needed — users must call struct literal
    }

    // Constructors → `new` associated functions
    for ctor in &cls.constructors {
        method_tokens.push(emit_constructor(ctor, &cls.name)?);
    }

    // Methods
    for method in &cls.methods {
        method_tokens.push(emit_method(method, &cls.name)?);
    }

    let impl_block = if !method_tokens.is_empty() || !cls.constructors.is_empty() {
        quote! {
            impl #name {
                #(#method_tokens)*
            }
        }
    } else {
        quote! {}
    };

    Ok(quote! {
        #struct_def
        #(#static_items)*
        #impl_block
    })
}

fn emit_constructor(ctor: &IrConstructor, _class_name: &str) -> Result<TokenStream, CodegenError> {
    let params = emit_params(&ctor.params);
    let body = emit_stmts(&ctor.body)?;
    Ok(quote! {
        pub fn new(#(#params),*) -> Self {
            #(#body)*
        }
    })
}

fn emit_method(method: &IrMethod, _class_name: &str) -> Result<TokenStream, CodegenError> {
    let name = ident(&method.name);
    let params = emit_params(&method.params);
    let ret_ty = emit_type(&method.return_ty);

    let self_param = if method.is_static {
        quote! {}
    } else {
        quote! { &self, }
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

    Ok(quote! {
        pub fn #name(#self_param #(#params),*) #ret_clause {
            #(#body)*
        }
    })
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

// ─── statements ─────────────────────────────────────────────────────────────

fn emit_stmts(stmts: &[IrStmt]) -> Result<Vec<TokenStream>, CodegenError> {
    stmts.iter().map(emit_stmt).collect()
}

/// Returns true if `stmts` (at the outermost level, not inside nested loops)
/// contains a bare `continue` that targets the enclosing for-loop.
fn for_body_has_continue(stmts: &[IrStmt]) -> bool {
    stmts.iter().any(|s| for_stmt_has_continue(s))
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
        IrStmt::Switch { expr, cases, default } => {
            let expr_ts = emit_expr(expr)?;
            let case_arms = cases
                .iter()
                .map(|c| emit_switch_case(c))
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
            Ok(quote! { (#recv).#field })
        }

        IrExpr::MethodCall {
            receiver,
            method_name,
            args,
            ..
        } => {
            let args_ts: Vec<TokenStream> = args
                .iter()
                .map(emit_expr)
                .collect::<Result<_, _>>()?;

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
                            return emit_print_call(macro_name, method_name, &args_ts, args.first().map(|e| e.ty()));
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
            let cls = ident(class);
            let args_ts: Vec<TokenStream> = args.iter().map(emit_expr).collect::<Result<_, _>>()?;
            Ok(quote! { #cls::new(#(#args_ts),*) })
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
            let l = emit_expr(lhs)?;
            let r = emit_expr(rhs)?;
            Ok(quote! { #l = #r })
        }

        IrExpr::CompoundAssign { op, lhs, rhs, .. } => {
            let l = emit_expr(lhs)?;
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
                _ => return Err(CodegenError::Unsupported(format!("{op:?} is not a compound-assignment operator"))),
            };
            Ok(quote! { #l #compound #r })
        }

        IrExpr::Cast { target, expr } => {
            let inner = emit_expr(expr)?;
            let ty = emit_type(target);
            Ok(quote! { (#inner as #ty) })
        }

        IrExpr::InstanceOf { expr, check_type } => {
            // No real downcasting at Stage 1 — emit a type-id check stub
            let _ = check_type;
            let inner = emit_expr(expr)?;
            Ok(quote! { { let _ = &#inner; true } })
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
            let id = ident(name);
            quote! { #id }
        }
        IrType::TypeVar(name) => {
            let id = ident(name);
            quote! { #id }
        }
        IrType::Generic { base, args } => {
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
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
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

