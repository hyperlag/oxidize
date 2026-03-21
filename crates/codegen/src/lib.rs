//! Code-generation pass: lowers a typed [`ir::IrModule`] to a Rust token
//! stream using `proc-macro2` and `quote`.
//!
//! Stage 1 scope:
//! - Single-class programs with `main(String[] args)`
//! - Primitive types and `String` (mapped to `java_compat::JString`)
//! - All arithmetic, comparison, logical, and bitwise operators
//! - `if/else`, `while`, `do-while`, traditional `for`, `switch`
//! - `return`, `break`, `continue`, `throw`
//! - `System.out.println` → `println!` macro
//! - Static and instance methods

use ir::decl::{IrClass, IrConstructor, IrMethod, IrParam};
use ir::expr::{BinOp, UnOp};
use ir::stmt::SwitchCase;
use ir::{IrDecl, IrExpr, IrModule, IrStmt, IrType};
use proc_macro2::{Ident, Span, TokenStream};
use proc_macro2::Literal;
use quote::{format_ident, quote};
use thiserror::Error;

/// Errors produced during code-generation.
#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("unsupported IR node: {0}")]
    Unsupported(String),
}

/// Lower `module` to a complete Rust source file as a [`TokenStream`].
///
/// The result can be formatted with `rustfmt` and written to disk.
pub fn generate(module: &IrModule) -> Result<TokenStream, CodegenError> {
    let mut items: Vec<TokenStream> = Vec::new();

    // Standard imports for every generated file
    items.push(quote! {
        #[allow(unused_imports)]
        use java_compat::{JArray, JString};
    });

    for decl in &module.decls {
        match decl {
            IrDecl::Class(cls) => items.push(gen_class(cls)?),
            IrDecl::Interface(_) => {} // interfaces not emitted in Stage 1
        }
    }

    Ok(quote! { #(#items)* })
}

// ─── Types ────────────────────────────────────────────────────────────────────

fn gen_type(ty: &IrType) -> TokenStream {
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
        IrType::Array(elem) => {
            let elem_ts = gen_type(elem);
            quote! { JArray<#elem_ts> }
        }
        IrType::Nullable(inner) => {
            let inner_ts = gen_type(inner);
            quote! { Option<#inner_ts> }
        }
        IrType::Class(name) | IrType::TypeVar(name) => {
            let ident = format_ident!("{}", name.replace('.', "_"));
            quote! { #ident }
        }
        IrType::Generic { base, .. } => gen_type(base),
        IrType::Unknown | IrType::Null => quote! { () },
    }
}

// ─── Class ────────────────────────────────────────────────────────────────────

fn gen_class(cls: &IrClass) -> Result<TokenStream, CodegenError> {
    let struct_name = format_ident!("{}", cls.name);

    // Fields
    let field_defs: Vec<TokenStream> = cls
        .fields
        .iter()
        .filter(|f| !f.is_static)
        .map(|f| {
            let fname = format_ident!("{}", f.name);
            let fty = gen_type(&f.ty);
            quote! { pub #fname: #fty, }
        })
        .collect();

    // Always emit a struct — unit struct when there are no fields
    let struct_def = if field_defs.is_empty() {
        quote! {
            #[derive(Debug, Clone)]
            pub struct #struct_name;
        }
    } else {
        quote! {
            #[derive(Debug, Clone)]
            pub struct #struct_name {
                #(#field_defs)*
            }
        }
    };

    // Gather static field initializers into module-level constants
    let static_fields: Vec<TokenStream> = cls
        .fields
        .iter()
        .filter(|f| f.is_static && f.is_final)
        .map(|f| {
            let fname = Ident::new(&f.name.to_uppercase(), Span::call_site());
            let fty = gen_type(&f.ty);
            let init = f.init.as_ref().map(gen_expr).unwrap_or(quote! { Default::default() });
            quote! { static #fname: #fty = #init; }
        })
        .collect();

    // Methods
    let methods: Vec<TokenStream> = cls
        .methods
        .iter()
        .map(|m| gen_method(m, &cls.name))
        .collect::<Result<_, _>>()?;

    // Constructors
    let ctors: Vec<TokenStream> = cls
        .constructors
        .iter()
        .map(|c| gen_constructor(c, &cls.name))
        .collect::<Result<_, _>>()?;

    let impl_block = if methods.is_empty() && ctors.is_empty() {
        quote! {}
    } else {
        quote! {
            impl #struct_name {
                #(#ctors)*
                #(#methods)*
            }
        }
    };

    // `main` entry point: if class has a static `main(String[] args)` method,
    // emit a Rust `fn main()` that calls it.
    let main_fn = if cls.methods.iter().any(|m| m.name == "main" && m.is_static) {
        quote! {
            fn main() {
                let args: JArray<JString> = JArray::from_vec(vec![]);
                #struct_name::main(args);
            }
        }
    } else {
        quote! {}
    };

    Ok(quote! {
        #struct_def
        #(#static_fields)*
        #impl_block
        #main_fn
    })
}

fn gen_method(method: &IrMethod, _class_name: &str) -> Result<TokenStream, CodegenError> {
    let mname = format_ident!("{}", method.name);
    let ret_ty = gen_type(&method.return_ty);
    let params = gen_params(&method.params, method.is_static);

    // Special case: main(String[] args) param becomes JArray<JString>
    let body = match &method.body {
        Some(stmts) => {
            let stmts_ts = gen_stmts(stmts)?;
            quote! { { #(#stmts_ts)* } }
        }
        None => quote! { ; },
    };

    let pub_kw = quote! { pub };
    let static_kw = if method.is_static { quote! { } } else { quote! { } };
    let _ = static_kw;

    if method.is_static {
        Ok(quote! {
            #pub_kw fn #mname(#params) -> #ret_ty #body
        })
    } else {
        Ok(quote! {
            #pub_kw fn #mname(&mut self, #params) -> #ret_ty #body
        })
    }
}

fn gen_constructor(ctor: &IrConstructor, class_name: &str) -> Result<TokenStream, CodegenError> {
    let params = gen_params(&ctor.params, false);
    let stmts_ts = gen_stmts(&ctor.body)?;
    let struct_name = format_ident!("{}", class_name);

    // Build `Self { field: param, ... }` return — simple heuristic: match param names to fields
    let param_inits: Vec<TokenStream> = ctor
        .params
        .iter()
        .map(|p| {
            let n = format_ident!("{}", p.name);
            quote! { #n, }
        })
        .collect();

    Ok(quote! {
        pub fn new(#params) -> #struct_name {
            #(#stmts_ts)*
            #struct_name { #(#param_inits)* }
        }
    })
}

fn gen_params(params: &[IrParam], _is_static: bool) -> TokenStream {
    let ps: Vec<TokenStream> = params
        .iter()
        .map(|p| {
            let pname = format_ident!("{}", p.name);
            let pty = gen_type(&p.ty);
            quote! { #pname: #pty }
        })
        .collect();
    quote! { #(#ps),* }
}

// ─── Statements ───────────────────────────────────────────────────────────────

fn gen_stmts(stmts: &[IrStmt]) -> Result<Vec<TokenStream>, CodegenError> {
    stmts.iter().map(gen_stmt).collect()
}

fn gen_stmt(stmt: &IrStmt) -> Result<TokenStream, CodegenError> {
    match stmt {
        IrStmt::LocalVar { name, ty, init } => {
            let vname = format_ident!("{}", name);
            let vty = gen_type(ty);
            match init {
                Some(e) => {
                    let init_ts = gen_expr(e);
                    Ok(quote! { let mut #vname: #vty = #init_ts; })
                }
                None => Ok(quote! { let mut #vname: #vty; }),
            }
        }

        IrStmt::Expr(e) => {
            let e_ts = gen_expr_stmt(e);
            Ok(quote! { #e_ts; })
        }

        IrStmt::Return(Some(e)) => {
            let e_ts = gen_expr(e);
            Ok(quote! { return #e_ts; })
        }
        IrStmt::Return(None) => Ok(quote! { return; }),

        IrStmt::Break(None) => Ok(quote! { break; }),
        IrStmt::Break(Some(label)) => {
            // labeled breaks are rare in Stage 1; emit as plain break
            let _ = label;
            Ok(quote! { break; })
        }
        IrStmt::Continue(None) => Ok(quote! { continue; }),
        IrStmt::Continue(Some(label)) => {
            let _ = label;
            Ok(quote! { continue; })
        }

        IrStmt::Throw(e) => {
            let e_ts = gen_expr(e);
            Ok(quote! { panic!("{}", #e_ts); })
        }

        IrStmt::If { cond, then_, else_ } => {
            let cond_ts = gen_expr(cond);
            let then_ts = gen_stmts(then_)?;
            match else_ {
                None => Ok(quote! { if #cond_ts { #(#then_ts)* } }),
                Some(else_stmts) => {
                    let else_ts = gen_stmts(else_stmts)?;
                    Ok(quote! { if #cond_ts { #(#then_ts)* } else { #(#else_ts)* } })
                }
            }
        }

        IrStmt::While { cond, body } => {
            let cond_ts = gen_expr(cond);
            let body_ts = gen_stmts(body)?;
            Ok(quote! { while #cond_ts { #(#body_ts)* } })
        }

        IrStmt::DoWhile { body, cond } => {
            let cond_ts = gen_expr(cond);
            let body_ts = gen_stmts(body)?;
            // Rust has no do-while; emulate with loop + break
            Ok(quote! {
                loop {
                    #(#body_ts)*
                    if !(#cond_ts) { break; }
                }
            })
        }

        IrStmt::For { init, cond, update, body } => {
            let init_ts = init
                .as_ref()
                .map(|s| gen_stmt(s))
                .transpose()?
                .unwrap_or_default();
            let cond_ts = cond
                .as_ref()
                .map(gen_expr)
                .unwrap_or(quote! { true });
            let update_ts: Vec<TokenStream> = update.iter().map(|e| {
                let e_ts = gen_expr_stmt(e);
                quote! { #e_ts; }
            }).collect();
            let body_ts = gen_stmts(body)?;
            Ok(quote! {
                #init_ts
                while #cond_ts {
                    #(#body_ts)*
                    #(#update_ts)*
                }
            })
        }

        IrStmt::ForEach { var, var_ty, iterable, body } => {
            let vname = format_ident!("{}", var);
            let vty = gen_type(var_ty);
            let iter_ts = gen_expr(iterable);
            let body_ts = gen_stmts(body)?;
            Ok(quote! {
                for #vname in #iter_ts.iter() as #vty {
                    #(#body_ts)*
                }
            })
        }

        IrStmt::Switch { expr, cases, default } => gen_switch(expr, cases, default),

        IrStmt::Block(stmts) => {
            let ts = gen_stmts(stmts)?;
            Ok(quote! { { #(#ts)* } })
        }

        IrStmt::TryCatch { body, catches, finally } => {
            // Rust has no exceptions; inline body and note unhandled catches
            let body_ts = gen_stmts(body)?;
            let catches_ts: Vec<TokenStream> = catches
                .iter()
                .map(|c| {
                    let stmts = gen_stmts(&c.body).unwrap_or_default();
                    quote! { { let _ = (); #(#stmts)* } }
                })
                .collect();
            let finally_ts = finally
                .as_ref()
                .map(|f| gen_stmts(f))
                .transpose()?
                .map(|ts| quote! { #(#ts)* })
                .unwrap_or_default();
            Ok(quote! {
                {
                    #(#body_ts)*
                    #(#catches_ts)*
                    #finally_ts
                }
            })
        }
    }
}

fn gen_switch(
    expr: &IrExpr,
    cases: &[SwitchCase],
    default: &Option<Vec<IrStmt>>,
) -> Result<TokenStream, CodegenError> {
    let expr_ts = gen_expr(expr);
    let mut arms: Vec<TokenStream> = cases
        .iter()
        .map(|c| {
            let val = gen_expr(&c.value);
            let body = gen_stmts(&c.body).unwrap_or_default();
            quote! { #val => { #(#body)* } }
        })
        .collect();

    let default_arm = match default {
        Some(d) => {
            let body = gen_stmts(d).unwrap_or_default();
            quote! { _ => { #(#body)* } }
        }
        None => quote! { _ => {} },
    };
    arms.push(default_arm);

    Ok(quote! {
        match #expr_ts {
            #(#arms)*
        }
    })
}

// ─── Expressions ──────────────────────────────────────────────────────────────

/// Generate an expression as a statement (handles `System.out.println` specially).
fn gen_expr_stmt(expr: &IrExpr) -> TokenStream {
    if let IrExpr::MethodCall { receiver, method_name, args, .. } = expr {
        if is_system_out(receiver.as_deref(), method_name) {
            let args_ts: Vec<TokenStream> = args.iter().map(gen_expr).collect();
            return match method_name.as_str() {
                "println" => {
                    if args_ts.is_empty() {
                        quote! { println!() }
                    } else {
                        quote! { println!("{}", #(#args_ts),*) }
                    }
                }
                "print" => quote! { print!("{}", #(#args_ts),*) },
                "printf" if !args_ts.is_empty() => {
                    // First arg is format string, rest are values
                    let fmt = &args_ts[0];
                    let rest = &args_ts[1..];
                    quote! { print!(#fmt #(, #rest)*) }
                }
                _ => quote! { print!("{}", #(#args_ts),*) },
            };
        }
    }
    gen_expr(expr)
}

fn gen_expr(expr: &IrExpr) -> TokenStream {
    match expr {
        IrExpr::LitBool(b) => quote! { #b },
        IrExpr::LitInt(n) => {
            let n = *n as i32;
            quote! { #n }
        }
        IrExpr::LitLong(n) => {
            let lit = Literal::i64_suffixed(*n);
            quote! { #lit }
        }
        IrExpr::LitFloat(f) => {
            let lit = Literal::f32_suffixed(*f as f32);
            quote! { #lit }
        }
        IrExpr::LitDouble(f) => {
            let lit = Literal::f64_suffixed(*f);
            quote! { #lit }
        }
        IrExpr::LitChar(c) => quote! { #c },
        IrExpr::LitString(s) => {
            quote! { JString::from(#s) }
        }
        IrExpr::LitNull => quote! { None },

        IrExpr::Var { name, .. } => {
            let ident = format_ident!("{}", name);
            quote! { #ident }
        }

        IrExpr::FieldAccess { receiver, field_name, .. } => {
            let recv_ts = gen_expr(receiver);
            let fname = format_ident!("{}", field_name);
            quote! { #recv_ts.#fname }
        }

        IrExpr::BinOp { op, lhs, rhs, .. } => {
            // Java string concatenation: `+` with any String operand, or Concat op
            if (matches!(op, BinOp::Add | BinOp::Concat))
                && (lhs.ty() == &IrType::String
                    || rhs.ty() == &IrType::String
                    || *op == BinOp::Concat)
            {
                let l = gen_expr(lhs);
                let r = gen_expr(rhs);
                return quote! { JString::from(format!("{}{}", #l, #r)) };
            }
            let l = gen_expr(lhs);
            let r = gen_expr(rhs);
            match op {
                BinOp::Add => quote! { (#l + #r) },
                BinOp::Sub => quote! { (#l - #r) },
                BinOp::Mul => quote! { (#l * #r) },
                BinOp::Div => quote! { (#l / #r) },
                BinOp::Rem => quote! { (#l % #r) },
                BinOp::BitAnd => quote! { (#l & #r) },
                BinOp::BitOr => quote! { (#l | #r) },
                BinOp::BitXor => quote! { (#l ^ #r) },
                BinOp::Shl => quote! { (#l << #r) },
                BinOp::Shr => quote! { (#l >> #r) },
                BinOp::UShr => quote! { (((#l as u64) >> #r) as i64) },
                BinOp::And => quote! { (#l && #r) },
                BinOp::Or => quote! { (#l || #r) },
                BinOp::Eq => quote! { (#l == #r) },
                BinOp::Ne => quote! { (#l != #r) },
                BinOp::Lt => quote! { (#l < #r) },
                BinOp::Le => quote! { (#l <= #r) },
                BinOp::Gt => quote! { (#l > #r) },
                BinOp::Ge => quote! { (#l >= #r) },
                BinOp::Concat => quote! { JString::from(format!("{}{}", #l, #r)) },
            }
        }

        IrExpr::UnOp { op, operand, .. } => {
            let o = gen_expr(operand);
            match op {
                UnOp::Neg => quote! { (-#o) },
                UnOp::Not => quote! { (!#o) },
                UnOp::BitNot => quote! { (!#o) },
                UnOp::PreInc => quote! { { #o += 1; #o } },
                UnOp::PreDec => quote! { { #o -= 1; #o } },
                UnOp::PostInc => quote! { { let _tmp = #o; #o += 1; _tmp } },
                UnOp::PostDec => quote! { { let _tmp = #o; #o -= 1; _tmp } },
            }
        }

        IrExpr::Ternary { cond, then_, else_, .. } => {
            let c = gen_expr(cond);
            let t = gen_expr(then_);
            let e = gen_expr(else_);
            quote! { (if #c { #t } else { #e }) }
        }

        IrExpr::Assign { lhs, rhs, .. } => {
            let l = gen_expr(lhs);
            let r = gen_expr(rhs);
            quote! { (#l = #r) }
        }

        IrExpr::CompoundAssign { op, lhs, rhs, .. } => {
            let l = gen_expr(lhs);
            let r = gen_expr(rhs);
            match op {
                BinOp::Add => quote! { (#l += #r) },
                BinOp::Sub => quote! { (#l -= #r) },
                BinOp::Mul => quote! { (#l *= #r) },
                BinOp::Div => quote! { (#l /= #r) },
                BinOp::Rem => quote! { (#l %= #r) },
                BinOp::BitAnd => quote! { (#l &= #r) },
                BinOp::BitOr => quote! { (#l |= #r) },
                BinOp::BitXor => quote! { (#l ^= #r) },
                BinOp::Shl => quote! { (#l <<= #r) },
                BinOp::Shr => quote! { (#l >>= #r) },
                _ => quote! { (#l = #r) }, // fallback
            }
        }

        IrExpr::MethodCall { receiver, method_name, args, .. } => {
            // System.out.println handled as expression (though it should be a stmt)
            if is_system_out(receiver.as_deref(), method_name) {
                return gen_expr_stmt(expr);
            }
            let args_ts: Vec<TokenStream> = args.iter().map(gen_expr).collect();
            let mname = format_ident!("{}", method_name);
            match receiver {
                Some(recv) => {
                    let r = gen_expr(recv);
                    quote! { #r.#mname(#(#args_ts),*) }
                }
                None => quote! { Self::#mname(#(#args_ts),*) },
            }
        }

        IrExpr::New { class, args, .. } => {
            let cname = format_ident!("{}", class);
            let args_ts: Vec<TokenStream> = args.iter().map(gen_expr).collect();
            quote! { #cname::new(#(#args_ts),*) }
        }

        IrExpr::NewArray { elem_ty, len, .. } => {
            let ety = gen_type(elem_ty);
            let len_ts = gen_expr(len);
            quote! { JArray::<#ety>::new_default(#len_ts as i32) }
        }

        IrExpr::ArrayAccess { array, index, .. } => {
            let arr = gen_expr(array);
            let idx = gen_expr(index);
            quote! { #arr.get(#idx as usize) }
        }

        IrExpr::Cast { target, expr } => {
            let target_ts = gen_type(target);
            let e = gen_expr(expr);
            quote! { (#e as #target_ts) }
        }

        IrExpr::InstanceOf { expr, check_type } => {
            // No runtime reflection in Stage 1; always emit false for unknown types
            let _ = expr;
            let _ = check_type;
            quote! { false }
        }
    }
}

fn is_system_out(receiver: Option<&IrExpr>, method_name: &str) -> bool {
    matches!(method_name, "println" | "print" | "printf")
        && matches!(
            receiver,
            Some(IrExpr::FieldAccess { receiver: outer, field_name, .. })
                if field_name == "out"
                && matches!(outer.as_ref(), IrExpr::Var { name, .. } if name == "System")
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ir::decl::{IrClass, IrMethod, IrParam, Visibility};

    fn hello_world_module() -> IrModule {
        let mut m = IrModule::new("");
        m.decls.push(IrDecl::Class(IrClass {
            name: "HelloWorld".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            type_params: vec![],
            superclass: None,
            interfaces: vec![],
            fields: vec![],
            constructors: vec![],
            methods: vec![IrMethod {
                name: "main".into(),
                visibility: Visibility::Public,
                is_static: true,
                is_abstract: false,
                is_final: false,
                type_params: vec![],
                params: vec![IrParam {
                    name: "args".into(),
                    ty: IrType::Array(Box::new(IrType::String)),
                    is_varargs: false,
                }],
                return_ty: IrType::Void,
                body: Some(vec![IrStmt::Expr(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::FieldAccess {
                        receiver: Box::new(IrExpr::Var {
                            name: "System".into(),
                            ty: IrType::Unknown,
                        }),
                        field_name: "out".into(),
                        ty: IrType::Unknown,
                    })),
                    method_name: "println".into(),
                    args: vec![IrExpr::LitString("Hello, World!".into())],
                    ty: IrType::Void,
                })]),
                throws: vec![],
            }],
        }));
        m
    }

    #[test]
    fn generates_hello_world() {
        let module = hello_world_module();
        let ts = generate(&module).expect("codegen must succeed");
        let src = ts.to_string();
        assert!(src.contains("fn main"), "must emit a main fn");
        assert!(src.contains("println"), "must emit println");
    }

    #[test]
    fn generates_arithmetic_method() {
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
                    op: BinOp::Add,
                    lhs: Box::new(IrExpr::Var { name: "a".into(), ty: IrType::Int }),
                    rhs: Box::new(IrExpr::Var { name: "b".into(), ty: IrType::Int }),
                    ty: IrType::Int,
                }))]),
                throws: vec![],
            }],
        }));
        let ts = generate(&m).expect("codegen must succeed");
        let src = ts.to_string();
        assert!(src.contains("fn add"), "must emit add fn");
        assert!(src.contains("i32"), "params must be i32");
    }
}
