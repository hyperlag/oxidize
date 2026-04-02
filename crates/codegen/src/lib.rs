//! Code-generation pass: lowers a typed [`ir::IrModule`] to a Rust source
//! string using `proc-macro2` and `quote`.

use ir::{
    decl::{IrClass, IrConstructor, IrEnum, IrInterface, IrMethod, IrParam},
    expr::{BinOp, UnOp},
    stmt::SwitchCase,
    IrDecl, IrExpr, IrModule, IrStmt, IrType,
};
use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::quote;
use std::cell::Cell;
use thiserror::Error;

thread_local! {
    /// Whether the currently-emitted method is static (no `self` receiver).
    static IN_STATIC_METHOD: Cell<bool> = const { Cell::new(false) };
    /// Maps every known enum name to its canonical (potentially mangled) Rust
    /// identifier.  For a top-level enum `Color` the entry is `"Color" →
    /// "Color"`.  For an inner enum that was promoted and mangled (e.g.
    /// `"Outer$Day"`) there are *two* entries:
    ///   `"Outer$Day"  → "Outer$Day"`   (full mangled key)
    ///   `"Day"        → "Outer$Day"`   (simple-name alias)
    /// This lets the codegen resolve `Season` → `EnumCompare$Season` even
    /// though the IR still carries the pre-mangling simple name.
    static ENUM_NAMES: std::cell::RefCell<std::collections::HashMap<String, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Field names of the enum currently being emitted (empty when not in an enum method).
    static ENUM_FIELD_NAMES: std::cell::RefCell<std::collections::HashSet<String>> =
        std::cell::RefCell::new(std::collections::HashSet::new());
}

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
            JReentrantLock, JCondition, JReentrantReadWriteLock, JReadLock, JWriteLock,
            JConcurrentHashMap, JCopyOnWriteArrayList, JThreadLocal,
            JExecutorService, JExecutors, JFuture, JCompletableFuture, JTimeUnit,
            JOptional, JStringBuilder, JBigInteger, JPattern, JMatcher,
            JLocalDate, JFile, JStream,
            JLocalTime, JLocalDateTime, JInstant, JDuration, JPeriod, JDateTimeFormatter,
            JLinkedList, JPriorityQueue, JTreeMap, JTreeSet,
            JLinkedHashMap, JLinkedHashSet, JIterator, JMapEntry,
            JEnumMap, JEnumSet,
            JBufferedReader, JBufferedWriter, JPrintWriter,
            JFileReader, JFileWriter, JFileInputStream, JFileOutputStream,
            JScanner, JPath, JPaths, JFiles,
            JBigDecimal, JMathContext, JRoundingMode,
            JURL, JSocket, JServerSocket, JHttpURLConnection,
            JSpliterator, JavaObject,
            JProcessBuilder, JProcess,
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

    let enum_map: std::collections::HashMap<String, &IrEnum> = module
        .decls
        .iter()
        .filter_map(|d| {
            if let IrDecl::Enum(e) = d {
                Some((e.name.clone(), e))
            } else {
                None
            }
        })
        .collect();

    // Populate thread-local enum names for expression emission.
    // For mangled names like "Outer$Day" we also register the simple name
    // "Day" as an alias so that IR references using the short form still
    // resolve to the correct Rust identifier.
    // Note: if two outer classes each define a nested enum with the same
    // simple name (e.g. A$Day and B$Day), the alias maps to whichever is
    // encountered first.  This is a best-effort fallback; the mangled
    // names themselves are always unambiguous and can be used directly.
    ENUM_NAMES.with(|names| {
        let mut names = names.borrow_mut();
        names.clear();
        for name in enum_map.keys() {
            // canonical entry: full name → full name
            names.insert(name.clone(), name.clone());
            // alias entry: simple name → full name (for mangled names like "A$B")
            if let Some(simple) = name.rfind('$').map(|i| &name[i + 1..]) {
                names
                    .entry(simple.to_owned())
                    .or_insert_with(|| name.clone());
            }
        }
    });

    // Collect class names so we know what to emit a main() for
    let mut main_class: Option<String> = None;

    for decl in &module.decls {
        match decl {
            IrDecl::Class(cls) => {
                if cls.methods.iter().any(|m| m.name == "main" && m.is_static) {
                    main_class = Some(cls.name.clone());
                }
                items.push(emit_class(cls, &class_map, &interface_map, &enum_map)?);
            }
            IrDecl::Interface(iface) => {
                items.push(emit_interface(iface)?);
            }
            IrDecl::Enum(enm) => {
                if enm.methods.iter().any(|m| m.name == "main" && m.is_static) {
                    main_class = Some(enm.name.clone());
                }
                items.push(emit_enum(enm, &enum_map)?);
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

// ─── enum ───────────────────────────────────────────────────────────────────

fn emit_enum(
    enm: &IrEnum,
    _enum_map: &std::collections::HashMap<String, &IrEnum>,
) -> Result<TokenStream, CodegenError> {
    let name = ident(&enm.name);

    // Variant identifiers
    let variant_idents: Vec<Ident> = enm.constants.iter().map(|c| ident(&c.name)).collect();

    // Enum definition with derives
    let enum_def = quote! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum #name {
            #(#variant_idents),*
        }
    };

    let mut impl_methods: Vec<TokenStream> = Vec::new();

    // __data() method for enums with constructor args
    let has_fields = enm
        .constructor
        .as_ref()
        .is_some_and(|c| !c.params.is_empty());
    if has_fields {
        let ctor = enm.constructor.as_ref().unwrap();
        let field_types: Vec<TokenStream> = ctor.params.iter().map(|p| emit_type(&p.ty)).collect();
        let data_arms: Vec<TokenStream> = enm
            .constants
            .iter()
            .map(|c| {
                let cname = ident(&c.name);
                let arg_exprs = c
                    .args
                    .iter()
                    .map(emit_expr)
                    .collect::<Result<Vec<_>, CodegenError>>()?;
                Ok(quote! { Self::#cname => (#(#arg_exprs),*,) })
            })
            .collect::<Result<Vec<_>, CodegenError>>()?;
        impl_methods.push(quote! {
            fn __data(&self) -> (#(#field_types),*,) {
                match self {
                    #(#data_arms),*
                }
            }
        });

        // Field accessor methods (one per field, indexed into __data() tuple)
        for (i, field) in enm.fields.iter().enumerate() {
            let fname = ident(&field.name);
            let fty = emit_type(&field.ty);
            let idx = syn::Index::from(i);
            impl_methods.push(quote! {
                pub fn #fname(&self) -> #fty {
                    self.__data().#idx
                }
            });
        }
    }

    // name() method
    let name_arms: Vec<TokenStream> = enm
        .constants
        .iter()
        .map(|c| {
            let cname = ident(&c.name);
            let cname_str = &c.name;
            quote! { Self::#cname => #cname_str }
        })
        .collect();
    impl_methods.push(quote! {
        pub fn name(&self) -> JString {
            JString::from(match self {
                #(#name_arms),*
            })
        }
    });

    // ordinal() method
    let ordinal_arms: Vec<TokenStream> = enm
        .constants
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let cname = ident(&c.name);
            let idx = i as i32;
            let idx_lit = Literal::i32_unsuffixed(idx);
            quote! { Self::#cname => #idx_lit }
        })
        .collect();
    impl_methods.push(quote! {
        pub fn ordinal(&self) -> i32 {
            match self {
                #(#ordinal_arms),*
            }
        }
    });

    // values() static method
    let values_items: Vec<TokenStream> = enm
        .constants
        .iter()
        .map(|c| {
            let cname = ident(&c.name);
            quote! { #name::#cname }
        })
        .collect();
    impl_methods.push(quote! {
        pub fn values() -> Vec<#name> {
            vec![#(#values_items),*]
        }
    });

    // valueOf(name) static method
    let valueof_arms: Vec<TokenStream> = enm
        .constants
        .iter()
        .map(|c| {
            let cname = ident(&c.name);
            let cname_str = &c.name;
            quote! { #cname_str => #name::#cname }
        })
        .collect();
    impl_methods.push(quote! {
        pub fn valueOf(s: JString) -> #name {
            match s.as_str() {
                #(#valueof_arms,)*
                _ => panic!("No enum constant {}", s),
            }
        }
    });

    // equals() for compatibility with Java's Object.equals()
    impl_methods.push(quote! {
        pub fn equals(&self, other: #name) -> bool {
            *self == other
        }
    });

    // User-defined methods
    // Set enum field names so that field accesses on `self` become method calls.
    ENUM_FIELD_NAMES.with(|names| {
        let mut set = names.borrow_mut();
        set.clear();
        for f in &enm.fields {
            set.insert(f.name.clone());
        }
    });
    for method in &enm.methods {
        let m = emit_enum_method(method)?;
        impl_methods.push(m);
    }
    ENUM_FIELD_NAMES.with(|names| names.borrow_mut().clear());

    let impl_block = quote! {
        impl #name {
            #(#impl_methods)*
        }
    };

    // Display impl (calls name())
    let display_impl = quote! {
        impl std::fmt::Display for #name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.name())
            }
        }
    };

    Ok(quote! {
        #enum_def
        #impl_block
        #display_impl
    })
}

fn emit_enum_method(method: &IrMethod) -> Result<TokenStream, CodegenError> {
    let mname = ident(&method.name);
    let params = emit_params_sig(&method.params);
    let ret_ty = emit_type(&method.return_ty);
    let body_stmts = if let Some(body) = &method.body {
        emit_stmts(body)?
    } else {
        vec![]
    };

    if method.is_static {
        IN_STATIC_METHOD.with(|b| b.set(true));
        let result = if method.return_ty == IrType::Void {
            quote! {
                pub fn #mname(#(#params),*) {
                    #(#body_stmts)*
                }
            }
        } else {
            quote! {
                pub fn #mname(#(#params),*) -> #ret_ty {
                    #(#body_stmts)*
                }
            }
        };
        IN_STATIC_METHOD.with(|b| b.set(false));
        Ok(result)
    } else if method.return_ty == IrType::Void {
        Ok(quote! {
            pub fn #mname(&self, #(#params),*) {
                #(#body_stmts)*
            }
        })
    } else {
        Ok(quote! {
            pub fn #mname(&self, #(#params),*) -> #ret_ty {
                #(#body_stmts)*
            }
        })
    }
}

// ─── class ──────────────────────────────────────────────────────────────────

fn emit_class(
    cls: &IrClass,
    class_map: &std::collections::HashMap<String, &IrClass>,
    interface_map: &std::collections::HashMap<String, &IrInterface>,
    _enum_map: &std::collections::HashMap<String, &IrEnum>,
) -> Result<TokenStream, CodegenError> {
    let name = ident(&cls.name);

    // Generic type parameters: `struct Foo<T>` / `impl<T: Clone + Default + Debug> Foo<T>`
    let (struct_generics, impl_generics) = if cls.type_params.is_empty() {
        (quote! {}, quote! {})
    } else {
        let names: Vec<_> = cls.type_params.iter().map(|tp| ident(&tp.name)).collect();
        let bounds: Vec<_> = cls
            .type_params
            .iter()
            .map(|tp| {
                let id = ident(&tp.name);
                let extra = extra_bounds_for_type_param(tp);
                quote! { #id: Clone + Default + ::std::fmt::Debug #extra }
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

    // Set the static-context flag so emit_expr can distinguish Self:: vs self.
    let prev = IN_STATIC_METHOD.with(|c| c.replace(method.is_static));
    let body = match &method.body {
        Some(stmts) => emit_stmts(stmts),
        None => {
            IN_STATIC_METHOD.with(|c| c.set(prev));
            return Ok(quote! {}); // abstract
        }
    };
    IN_STATIC_METHOD.with(|c| c.set(prev));
    let body = body?;

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

            // Check if we're switching on an enum type; resolve through alias
            // map so that mangled names like `EnumCompare$Season` are found
            // even when the IR carries the simple name `Season`.
            let enum_type_name = if let IrType::Class(ref name) = expr.ty() {
                ENUM_NAMES.with(|names| names.borrow().get(name.as_str()).cloned())
            } else {
                None
            };

            let case_arms = if let Some(ref ename) = enum_type_name {
                cases
                    .iter()
                    .map(|c| emit_enum_switch_case(c, ename))
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                cases
                    .iter()
                    .map(emit_switch_case)
                    .collect::<Result<Vec<_>, _>>()?
            };
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

/// Strip trailing `break;` from a switch-case body (Java needs it, Rust match doesn't).
fn strip_switch_breaks(stmts: &[IrStmt]) -> Vec<IrStmt> {
    let mut out: Vec<IrStmt> = stmts.to_vec();
    while matches!(out.last(), Some(IrStmt::Break(None))) {
        out.pop();
    }
    out
}

fn emit_switch_case(case: &SwitchCase) -> Result<TokenStream, CodegenError> {
    let val = emit_expr(&case.value)?;
    let body = emit_stmts(&strip_switch_breaks(&case.body))?;
    Ok(quote! {
        #val => { #(#body)* }
    })
}

fn emit_enum_switch_case(case: &SwitchCase, enum_name: &str) -> Result<TokenStream, CodegenError> {
    let enum_ident = ident(enum_name);
    // Case value is a bare Var with the constant name (e.g., RED)
    let const_name = match &case.value {
        IrExpr::Var { name, .. } => name.clone(),
        IrExpr::FieldAccess { field_name, .. } => field_name.clone(),
        _ => return emit_switch_case(case),
    };
    let const_ident = ident(&const_name);
    let body = emit_stmts(&strip_switch_breaks(&case.body))?;
    Ok(quote! {
        #enum_ident::#const_ident => { #(#body)* }
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
        IrExpr::Unit => Ok(quote! { () }),
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
            // Enum constant access: Color.RED → Color::RED
            // Resolve through the alias map so mangled names work too.
            if let IrExpr::Var { name, .. } = receiver.as_ref() {
                let canonical = ENUM_NAMES.with(|names| names.borrow().get(name.as_str()).cloned());
                if let Some(canonical_name) = canonical {
                    let enum_ident = ident(&canonical_name);
                    let const_ident = ident(field_name);
                    return Ok(quote! { #enum_ident::#const_ident });
                }
            }
            // System.out / System.err — emit as the receiver for println calls
            if let IrExpr::Var { name, .. } = receiver.as_ref() {
                if name == "System" && (field_name == "out" || field_name == "err") {
                    // Will be consumed by a MethodCall — return a placeholder
                    let stream = ident(field_name);
                    return Ok(quote! { std::io::#stream });
                }
            }
            // BigDecimal / MathContext / RoundingMode static field constants
            if let IrExpr::Var { name, .. } = receiver.as_ref() {
                match name.as_str() {
                    "BigDecimal" => {
                        return match field_name.as_str() {
                            "ZERO" => Ok(quote! { JBigDecimal::zero() }),
                            "ONE" => Ok(quote! { JBigDecimal::one() }),
                            "TEN" => Ok(quote! { JBigDecimal::ten() }),
                            _ => {
                                let field_lit = proc_macro2::Literal::string(field_name);
                                Ok(quote! {
                                    panic!(concat!(
                                        "Unsupported BigDecimal static field: ",
                                        #field_lit
                                    ))
                                })
                            }
                        };
                    }
                    "MathContext" => {
                        return match field_name.as_str() {
                            "DECIMAL32" => Ok(quote! { JMathContext::decimal32() }),
                            "DECIMAL64" => Ok(quote! { JMathContext::decimal64() }),
                            "DECIMAL128" => Ok(quote! { JMathContext::decimal128() }),
                            "UNLIMITED" => Ok(quote! { JMathContext::unlimited() }),
                            _ => {
                                let f = ident(field_name);
                                Ok(quote! { JMathContext::#f() })
                            }
                        };
                    }
                    "RoundingMode" => {
                        return match field_name.as_str() {
                            "UP" => Ok(quote! { JRoundingMode::Up }),
                            "DOWN" => Ok(quote! { JRoundingMode::Down }),
                            "CEILING" => Ok(quote! { JRoundingMode::Ceiling }),
                            "FLOOR" => Ok(quote! { JRoundingMode::Floor }),
                            "HALF_UP" => Ok(quote! { JRoundingMode::HalfUp }),
                            "HALF_DOWN" => Ok(quote! { JRoundingMode::HalfDown }),
                            "HALF_EVEN" => Ok(quote! { JRoundingMode::HalfEven }),
                            "UNNECESSARY" => Ok(quote! { JRoundingMode::Unnecessary }),
                            _ => {
                                let f = ident(field_name);
                                Ok(quote! { JRoundingMode::#f })
                            }
                        };
                    }
                    "TimeUnit" => {
                        return match field_name.as_str() {
                            "NANOSECONDS" => Ok(quote! { JTimeUnit::NANOSECONDS }),
                            "MICROSECONDS" => Ok(quote! { JTimeUnit::MICROSECONDS }),
                            "MILLISECONDS" => Ok(quote! { JTimeUnit::MILLISECONDS }),
                            "SECONDS" => Ok(quote! { JTimeUnit::SECONDS }),
                            "MINUTES" => Ok(quote! { JTimeUnit::MINUTES }),
                            "HOURS" => Ok(quote! { JTimeUnit::HOURS }),
                            "DAYS" => Ok(quote! { JTimeUnit::DAYS }),
                            _ => {
                                let f = ident(field_name);
                                Ok(quote! { JTimeUnit::#f })
                            }
                        };
                    }
                    _ => {}
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
                // fallback for Vec (e.g. Enum.values().length)
                return Ok(quote! { ((#recv).len() as i32) });
            }
            // Enum field access on self → call accessor method
            let is_enum_field =
                ENUM_FIELD_NAMES.with(|names| names.borrow().contains(field_name.as_str()));
            if is_enum_field {
                let field = ident(field_name);
                return Ok(quote! { (#recv).#field() });
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

                // Runtime.getRuntime().exec(cmd) — subprocess convenience wrapper.
                // Detect the two-level chain: MethodCall(Var("Runtime"), "getRuntime").exec(...)
                if let IrExpr::MethodCall {
                    receiver: Some(rt_recv),
                    method_name: inner_method,
                    ..
                } = recv.as_ref()
                {
                    if let IrExpr::Var { name: rt_name, .. } = rt_recv.as_ref() {
                        if rt_name == "Runtime" && inner_method == "getRuntime" {
                            return match method_name.as_str() {
                                "exec" if !args_ts.is_empty() => {
                                    let cmd = &args_ts[0];
                                    // Dispatch to exec_array for String[] arg, exec_string otherwise.
                                    if matches!(
                                        args.first().map(|e| e.ty()),
                                        Some(IrType::Array(_))
                                    ) && args_ts.len() == 1
                                    {
                                        Ok(quote! { JProcessBuilder::exec_array(#cmd) })
                                    } else {
                                        Ok(quote! { JProcessBuilder::exec_string(#cmd) })
                                    }
                                }
                                _ => Err(CodegenError::Unsupported(
                                    "Runtime.getRuntime() methods other than exec are unsupported"
                                        .into(),
                                )),
                            };
                        }
                    }
                }

                // Thread.sleep(ms) — static call on the Thread class name.
                if let IrExpr::Var { name, .. } = recv.as_ref() {
                    if name == "Thread" && method_name == "sleep" {
                        return Ok(quote! { JThread::sleep(#(#args_ts),*) });
                    }

                    // Executors.newFixedThreadPool / newSingleThreadExecutor / newCachedThreadPool
                    if name == "Executors" {
                        return match method_name.as_str() {
                            "newFixedThreadPool" => {
                                Ok(quote! { JExecutors::newFixedThreadPool(#(#args_ts),*) })
                            }
                            "newSingleThreadExecutor" => {
                                Ok(quote! { JExecutors::newSingleThreadExecutor() })
                            }
                            "newCachedThreadPool" => {
                                Ok(quote! { JExecutors::newCachedThreadPool() })
                            }
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JExecutors::#m(#(#args_ts),*) })
                            }
                        };
                    }

                    // CompletableFuture.supplyAsync / runAsync / completedFuture
                    if name == "CompletableFuture" {
                        return match method_name.as_str() {
                            "supplyAsync" | "runAsync" => {
                                // The arg is a lambda / closure
                                let f = &args_ts[0];
                                let m = ident(method_name);
                                Ok(quote! { JCompletableFuture::#m(#f) })
                            }
                            "completedFuture" => {
                                let v = &args_ts[0];
                                Ok(quote! { JCompletableFuture::completedFuture(#v) })
                            }
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JCompletableFuture::#m(#(#args_ts),*) })
                            }
                        };
                    }

                    // ThreadLocal.withInitial(supplier)
                    if name == "ThreadLocal" {
                        return match method_name.as_str() {
                            "withInitial" => {
                                let f = &args_ts[0];
                                Ok(quote! { JThreadLocal::withInitial(#f) })
                            }
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JThreadLocal::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // Runtime.getRuntime() — returns a unit sentinel; exec handled via chain detection.
                    if name == "Runtime" && method_name == "getRuntime" {
                        // Emit unit; the subsequent .exec() call is handled by the chained
                        // MethodCall detection above which matches on the Var("Runtime") receiver
                        // directly (not the result of getRuntime()).
                        return Ok(quote! { () });
                    }
                    // System.exit / currentTimeMillis / nanoTime / getenv / getProperty / arraycopy
                    if name == "System" {
                        return match method_name.as_str() {
                            "exit" => {
                                let code = &args_ts[0];
                                Ok(quote! { std::process::exit(#code) })
                            }
                            "currentTimeMillis" => Ok(quote! { {
                                std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis() as i64
                            } }),
                            "nanoTime" => Ok(quote! { {
                                std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_nanos() as i64
                            } }),
                            "getenv" => {
                                let key = &args_ts[0];
                                Ok(quote! { JString::from(
                                    std::env::var(#key.as_str()).unwrap_or_default().as_str()
                                ) })
                            }
                            "getProperty" => {
                                if args_ts.len() >= 2 {
                                    let key = &args_ts[0];
                                    let def = &args_ts[1];
                                    Ok(quote! { {
                                        let __k = #key;
                                        match __k.as_str() {
                                            "line.separator" => JString::from("\n"),
                                            "file.separator" | "path.separator" => {
                                                let mut __buf = [0u8; 4];
                                                let __s = std::path::MAIN_SEPARATOR.encode_utf8(&mut __buf);
                                                JString::from(&*__s)
                                            },
                                            "os.name" => JString::from(std::env::consts::OS),
                                            "os.arch" => JString::from(std::env::consts::ARCH),
                                            "user.dir" => {
                                                let __p = std::env::current_dir()
                                                    .map(|p| p.to_string_lossy().into_owned())
                                                    .unwrap_or_default();
                                                JString::from(__p.as_str())
                                            },
                                            "user.home" => {
                                                let __h = std::env::var("HOME").unwrap_or_default();
                                                JString::from(__h.as_str())
                                            },
                                            _ => #def,
                                        }
                                    } })
                                } else {
                                    let key = &args_ts[0];
                                    Ok(quote! { {
                                        let __k = #key;
                                        match __k.as_str() {
                                            "line.separator" => JString::from("\n"),
                                            "file.separator" | "path.separator" => {
                                                let mut __buf = [0u8; 4];
                                                let __s = std::path::MAIN_SEPARATOR.encode_utf8(&mut __buf);
                                                JString::from(&*__s)
                                            },
                                            "os.name" => JString::from(std::env::consts::OS),
                                            "os.arch" => JString::from(std::env::consts::ARCH),
                                            "user.dir" => {
                                                let __p = std::env::current_dir()
                                                    .map(|p| p.to_string_lossy().into_owned())
                                                    .unwrap_or_default();
                                                JString::from(__p.as_str())
                                            },
                                            "user.home" => {
                                                let __h = std::env::var("HOME").unwrap_or_default();
                                                JString::from(__h.as_str())
                                            },
                                            _ => JString::from(""),
                                        }
                                    } })
                                }
                            }
                            "lineSeparator" => Ok(quote! { JString::from("\n") }),
                            _ => Err(CodegenError::Unsupported(format!(
                                "Unsupported java.lang.System static method: {}.{}",
                                name, method_name
                            ))),
                        };
                    }
                    // Math.x(...) — static Math methods → f64 method calls or std ops.
                    if name == "Math" {
                        let first_ty = args.first().map(|e| e.ty().clone());
                        let is_double = matches!(
                            first_ty.as_ref(),
                            Some(IrType::Double) | Some(IrType::Float)
                        );
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
                                    Ok(
                                        quote! { { let __a = #a as f64; let __b = #b as f64; if __a > __b { __a } else { __b } } },
                                    )
                                } else if is_long {
                                    Ok(
                                        quote! { { let __a = #a as i64; let __b = #b as i64; if __a > __b { __a } else { __b } } },
                                    )
                                } else {
                                    Ok(
                                        quote! { { let __a = #a as i32; let __b = #b as i32; if __a > __b { __a } else { __b } } },
                                    )
                                }
                            }
                            "min" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                if is_double {
                                    Ok(
                                        quote! { { let __a = #a as f64; let __b = #b as f64; if __a < __b { __a } else { __b } } },
                                    )
                                } else if is_long {
                                    Ok(
                                        quote! { { let __a = #a as i64; let __b = #b as i64; if __a < __b { __a } else { __b } } },
                                    )
                                } else {
                                    Ok(
                                        quote! { { let __a = #a as i32; let __b = #b as i32; if __a < __b { __a } else { __b } } },
                                    )
                                }
                            }
                            "pow" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { (#a as f64).powf(#b as f64) })
                            }
                            "sqrt" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as f64).sqrt() })
                            }
                            "floor" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as f64).floor() })
                            }
                            "ceil" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as f64).ceil() })
                            }
                            "round" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as f64).round() as i64 })
                            }
                            "log" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as f64).ln() })
                            }
                            "log10" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as f64).log10() })
                            }
                            "sin" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as f64).sin() })
                            }
                            "cos" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as f64).cos() })
                            }
                            "tan" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as f64).tan() })
                            }
                            "exp" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as f64).exp() })
                            }
                            "random" => Ok(quote! { 0.0_f64 }),
                            "PI" => Ok(quote! { std::f64::consts::PI }),
                            _ => Err(CodegenError::Unsupported(format!(
                                "unsupported Math method: {}",
                                method_name
                            ))),
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
                    // LocalDate.of(...) / LocalDate.now() / LocalDate.parse()
                    if name == "LocalDate" {
                        return match method_name.as_str() {
                            "of" => Ok(quote! { JLocalDate::of(#(#args_ts),*) }),
                            "now" => Ok(quote! { JLocalDate::now() }),
                            "parse" => {
                                let a = &args_ts[0];
                                Ok(quote! { JLocalDate::parse(&#a) })
                            }
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JLocalDate::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // LocalTime.of(...) / LocalTime.now() / LocalTime.parse()
                    if name == "LocalTime" {
                        return match method_name.as_str() {
                            "of" => match args_ts.len() {
                                2 => Ok(quote! { JLocalTime::of_hm(#(#args_ts),*) }),
                                3 => Ok(quote! { JLocalTime::of_hms(#(#args_ts),*) }),
                                _ => Ok(quote! { JLocalTime::of_hmsn(#(#args_ts),*) }),
                            },
                            "now" => Ok(quote! { JLocalTime::now() }),
                            "parse" => {
                                let a = &args_ts[0];
                                Ok(quote! { JLocalTime::parse(&#a) })
                            }
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JLocalTime::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // LocalDateTime.of(...) / LocalDateTime.now() / LocalDateTime.parse()
                    if name == "LocalDateTime" {
                        return match method_name.as_str() {
                            "of" => match args_ts.len() {
                                5 => Ok(quote! { JLocalDateTime::of_ymd_hm(#(#args_ts),*) }),
                                6 => Ok(quote! { JLocalDateTime::of_ymd_hms(#(#args_ts),*) }),
                                7 => Ok(quote! { JLocalDateTime::of_ymd_hmsn(#(#args_ts),*) }),
                                2 => {
                                    let a = &args_ts[0];
                                    let b = &args_ts[1];
                                    Ok(quote! { JLocalDateTime::of_dt(#a, #b) })
                                }
                                _ => Ok(quote! { JLocalDateTime::of_ymd_hms(#(#args_ts),*) }),
                            },
                            "now" => Ok(quote! { JLocalDateTime::now() }),
                            "parse" => {
                                let a = &args_ts[0];
                                Ok(quote! { JLocalDateTime::parse(&#a) })
                            }
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JLocalDateTime::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // Instant.now() / Instant.ofEpochSecond() / Instant.ofEpochMilli()
                    if name == "Instant" {
                        return match method_name.as_str() {
                            "now" => Ok(quote! { JInstant::now() }),
                            "ofEpochSecond" => {
                                Ok(quote! { JInstant::ofEpochSecond(#(#args_ts),*) })
                            }
                            "ofEpochMilli" => Ok(quote! { JInstant::ofEpochMilli(#(#args_ts),*) }),
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JInstant::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // Duration.ofSeconds() / Duration.ofMillis() / Duration.between() etc.
                    if name == "Duration" {
                        return match method_name.as_str() {
                            "ofSeconds" => Ok(quote! { JDuration::ofSeconds(#(#args_ts),*) }),
                            "ofMillis" => Ok(quote! { JDuration::ofMillis(#(#args_ts),*) }),
                            "ofMinutes" => Ok(quote! { JDuration::ofMinutes(#(#args_ts),*) }),
                            "ofHours" => Ok(quote! { JDuration::ofHours(#(#args_ts),*) }),
                            "ofDays" => Ok(quote! { JDuration::ofDays(#(#args_ts),*) }),
                            "ofNanos" => Ok(quote! { JDuration::ofNanos(#(#args_ts),*) }),
                            "between" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { JDuration::between(&#a, &#b) })
                            }
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JDuration::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // Period.of() / Period.between() / Period.ofDays() etc.
                    if name == "Period" {
                        return match method_name.as_str() {
                            "of" => Ok(quote! { JPeriod::of(#(#args_ts),*) }),
                            "ofDays" => Ok(quote! { JPeriod::ofDays(#(#args_ts),*) }),
                            "ofMonths" => Ok(quote! { JPeriod::ofMonths(#(#args_ts),*) }),
                            "ofYears" => Ok(quote! { JPeriod::ofYears(#(#args_ts),*) }),
                            "ofWeeks" => Ok(quote! { JPeriod::ofWeeks(#(#args_ts),*) }),
                            "between" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { JPeriod::between(&#a, &#b) })
                            }
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JPeriod::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // DateTimeFormatter.ofPattern()
                    if name == "DateTimeFormatter" {
                        return match method_name.as_str() {
                            "ofPattern" => {
                                let a = &args_ts[0];
                                Ok(quote! { JDateTimeFormatter::ofPattern(&#a) })
                            }
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JDateTimeFormatter::#m(#(#args_ts),*) })
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
                    // BigDecimal.valueOf / BigDecimal.ZERO / ONE / TEN
                    if name == "BigDecimal" {
                        return match method_name.as_str() {
                            "valueOf" => {
                                if args_ts.len() == 2 {
                                    let a = &args_ts[0];
                                    let b = &args_ts[1];
                                    Ok(quote! { JBigDecimal::value_of_scaled(#a, #b) })
                                } else {
                                    let first_ty = args.first().map(|e| e.ty().clone());
                                    let is_double = matches!(
                                        first_ty.as_ref(),
                                        Some(IrType::Double) | Some(IrType::Float)
                                    );
                                    let a = &args_ts[0];
                                    if is_double {
                                        Ok(quote! { JBigDecimal::value_of_double(#a as f64) })
                                    } else {
                                        Ok(quote! { JBigDecimal::value_of(#a as i64) })
                                    }
                                }
                            }
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JBigDecimal::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // MathContext named constants
                    if name == "MathContext" {
                        let m = ident(method_name);
                        return Ok(quote! { JMathContext::#m(#(#args_ts),*) });
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
                            "format" => {
                                // String.format(fmt, args...) → jformat(fmt, &[args...])
                                if args_ts.is_empty() {
                                    Ok(quote! { JString::from("") })
                                } else {
                                    let fmt = &args_ts[0];
                                    let rest = &args_ts[1..];
                                    let rest_strs: Vec<TokenStream> = rest
                                        .iter()
                                        .map(|a| {
                                            quote! { format!("{}", #a) }
                                        })
                                        .collect();
                                    Ok(quote! { java_compat::jformat(#fmt, &[#(#rest_strs),*]) })
                                }
                            }
                            "join" => {
                                // String.join(delimiter, elements...)
                                let delim = &args_ts[0];
                                let rest = &args_ts[1..];
                                let rest_strs: Vec<TokenStream> = rest
                                    .iter()
                                    .map(|a| {
                                        quote! { format!("{}", #a) }
                                    })
                                    .collect();
                                Ok(quote! { JString::from(
                                    [#(#rest_strs),*].join(#delim.as_str()).as_str()
                                ) })
                            }
                            _ => Ok(quote! { JString::from("") }),
                        };
                    }
                    // Collections.sort / Collections.reverse / Collections.unmodifiable* / etc.
                    if name == "Collections" {
                        return match method_name.as_str() {
                            "sort" => {
                                if args_ts.len() == 1 {
                                    let a = &args_ts[0];
                                    Ok(quote! { (#a).sort() })
                                } else {
                                    let a = &args_ts[0];
                                    let b = &args_ts[1];
                                    Ok(quote! { (#a).sort_with(#b) })
                                }
                            }
                            "reverse" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a).reverse() })
                            }
                            "unmodifiableList" | "unmodifiableMap" | "unmodifiableSet" => {
                                let a = &args_ts[0];
                                Ok(quote! { #a })
                            }
                            "emptyList" => Ok(quote! { JList::new() }),
                            "emptyMap" => Ok(quote! { JMap::new() }),
                            "emptySet" => Ok(quote! { JSet::new() }),
                            "singletonList" => {
                                let a = &args_ts[0];
                                Ok(quote! { JList::singleton(#a) })
                            }
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { java_compat::collections_util::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // Arrays.asList(...)
                    if name == "Arrays" && method_name == "asList" {
                        return Ok(quote! { {
                            let mut __list = JList::new();
                            #( __list.add(#args_ts); )*
                            __list
                        } });
                    }
                    // EnumSet.noneOf(...) / EnumSet.of(...) / EnumSet.allOf(...)
                    // Class-literal args (e.g. Color.class) are filtered out by the
                    // walker, so args_ts is empty for noneOf/allOf.
                    if name == "EnumSet" {
                        return match method_name.as_str() {
                            "noneOf" => Ok(quote! { JEnumSet::new() }),
                            "of" => Ok(quote! { JEnumSet::of(vec![#(#args_ts),*]) }),
                            "allOf" => {
                                // allOf requires all enum variants, which this code generator
                                // cannot construct statically — panic at runtime rather than
                                // silently returning an incorrect empty set.
                                Ok(quote! {{
                                    panic!(
                                        "EnumSet::allOf(...) is not supported by this code generator; \
                                         use EnumSet::of(...) or EnumSet::noneOf(...) instead"
                                    )
                                }})
                            }
                            "copyOf" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a).clone() })
                            }
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JEnumSet::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // Files.readString / Files.writeString / etc.
                    if name == "Files" {
                        return match method_name.as_str() {
                            "readString" => {
                                let a = &args_ts[0];
                                Ok(quote! { JFiles::readString(&#a) })
                            }
                            "writeString" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { JFiles::writeString(&#a, #b) })
                            }
                            "readAllLines" => {
                                let a = &args_ts[0];
                                Ok(quote! { JFiles::readAllLines(&#a) })
                            }
                            "write" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { JFiles::write_lines(&#a, &#b) })
                            }
                            "exists" => {
                                let a = &args_ts[0];
                                Ok(quote! { JFiles::exists(&#a) })
                            }
                            "isDirectory" => {
                                let a = &args_ts[0];
                                Ok(quote! { JFiles::isDirectory(&#a) })
                            }
                            "isRegularFile" => {
                                let a = &args_ts[0];
                                Ok(quote! { JFiles::isRegularFile(&#a) })
                            }
                            "size" => {
                                let a = &args_ts[0];
                                Ok(quote! { JFiles::size(&#a) })
                            }
                            "delete" => {
                                let a = &args_ts[0];
                                Ok(quote! { JFiles::delete(&#a) })
                            }
                            "deleteIfExists" => {
                                let a = &args_ts[0];
                                Ok(quote! { JFiles::deleteIfExists(&#a) })
                            }
                            "createDirectory" => {
                                let a = &args_ts[0];
                                Ok(quote! { JFiles::createDirectory(&#a) })
                            }
                            "createDirectories" => {
                                let a = &args_ts[0];
                                Ok(quote! { JFiles::createDirectories(&#a) })
                            }
                            "copy" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { JFiles::copy(&#a, &#b) })
                            }
                            "move" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { JFiles::move_path(&#a, &#b) })
                            }
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JFiles::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // Paths.get(...)
                    if name == "Paths" {
                        return match method_name.as_str() {
                            "get" => {
                                let a = &args_ts[0];
                                Ok(quote! { JPath::get(#a) })
                            }
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JPaths::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // Enum static method calls: Color.values(), Color.valueOf(...)
                    // Resolve through alias map for mangled inner enum names.
                    let canonical_enum =
                        ENUM_NAMES.with(|names| names.borrow().get(name.as_str()).cloned());
                    if let Some(canonical_name) = canonical_enum {
                        let enum_ident = ident(&canonical_name);
                        let m = ident(method_name);
                        return Ok(quote! { #enum_ident::#m(#(#args_ts),*) });
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

                // Handle method overloads that need special dispatch.
                if method_name == "substring" && args_ts.len() == 2 {
                    let a = &args_ts[0];
                    let b = &args_ts[1];
                    return Ok(quote! { (#recv_ts).substring_range(#a, #b) });
                }

                // BigDecimal.divide(BigDecimal, int, RoundingMode) → divide_with_scale
                if method_name == "divide"
                    && args_ts.len() == 3
                    && matches!(recv.ty(), IrType::Class(c) if c == "BigDecimal")
                {
                    let a = &args_ts[0];
                    let b = &args_ts[1];
                    let c = &args_ts[2];
                    return Ok(quote! { (#recv_ts).divide_with_scale(#a, #b, #c) });
                }

                // BigDecimal.setScale(int, RoundingMode) → setScale
                if method_name == "setScale"
                    && args_ts.len() == 2
                    && matches!(recv.ty(), IrType::Class(c) if c == "BigDecimal")
                {
                    let a = &args_ts[0];
                    let b = &args_ts[1];
                    return Ok(quote! { (#recv_ts).setScale(#a, #b) });
                }

                // LocalDate.atTime(hour, minute) → atTime_hm(hour, minute)
                if method_name == "atTime"
                    && args_ts.len() == 2
                    && matches!(recv.ty(), IrType::Class(c) if c == "LocalDate")
                {
                    let a = &args_ts[0];
                    let b = &args_ts[1];
                    return Ok(quote! { (#recv_ts).atTime_hm(#a, #b) });
                }

                // isBefore/isAfter/isEqual on time types — pass arg by reference
                if (method_name == "isBefore"
                    || method_name == "isAfter"
                    || method_name == "isEqual")
                    && args_ts.len() == 1
                    && matches!(recv.ty(), IrType::Class(c) if
                        c == "LocalDate" || c == "LocalTime" || c == "LocalDateTime"
                        || c == "Instant" || c == "Duration" || c == "Period"
                    )
                {
                    let a = &args_ts[0];
                    let m = ident(method_name);
                    return Ok(quote! { (#recv_ts).#m(&#a) });
                }

                // Duration/Period plus/minus — pass arg by reference
                if (method_name == "plus" || method_name == "minus")
                    && args_ts.len() == 1
                    && matches!(recv.ty(), IrType::Class(c) if c == "Duration" || c == "Period")
                {
                    let a = &args_ts[0];
                    let m = ident(method_name);
                    return Ok(quote! { (#recv_ts).#m(&#a) });
                }

                // LocalDate/LocalDateTime/LocalTime.format(formatter) — pass formatter by reference
                if method_name == "format"
                    && args_ts.len() == 1
                    && matches!(recv.ty(), IrType::Class(c) if
                        c == "LocalDate" || c == "LocalDateTime" || c == "LocalTime"
                    )
                {
                    let a = &args_ts[0];
                    return Ok(quote! { (#recv_ts).format(&#a) });
                }

                // String.equals(obj) — pass by reference (JString::equals takes &JString).
                if method_name == "equals" && args_ts.len() == 1 && *recv.ty() == IrType::String {
                    let a = &args_ts[0];
                    return Ok(quote! { (#recv_ts).equals(&#a) });
                }

                // ExecutorService.submit(runnable) / execute(runnable) — wrap in move-closure
                if (method_name == "submit" || method_name == "execute")
                    && args_ts.len() == 1
                    && type_name_matches(recv.ty(), "ExecutorService")
                {
                    let task = &args_ts[0];
                    if method_name == "submit" {
                        return Ok(quote! { (#recv_ts).submit_runnable({
                            let mut __r = #task;
                            move || { (__r).run(); }
                        }) });
                    } else {
                        return Ok(quote! { (#recv_ts).execute({
                            let mut __r = #task;
                            move || { (__r).run(); }
                        }) });
                    }
                }

                // executor.awaitTermination(timeout, TimeUnit.X) — convert to millis
                if method_name == "awaitTermination"
                    && args_ts.len() == 2
                    && type_name_matches(recv.ty(), "ExecutorService")
                {
                    let timeout = &args_ts[0];
                    let unit = &args_ts[1];
                    return Ok(quote! { (#recv_ts).awaitTermination(#unit.toMillis(#timeout)) });
                }

                // ConcurrentHashMap.get(key) / containsKey(key) / remove(key) / getOrDefault(key, default) — pass key by ref
                if type_name_matches(recv.ty(), "ConcurrentHashMap") {
                    match method_name.as_str() {
                        "get" | "containsKey" | "remove" => {
                            let a = &args_ts[0];
                            let m = ident(method_name);
                            return Ok(quote! { (#recv_ts).#m(&#a) });
                        }
                        "getOrDefault" if args_ts.len() == 2 => {
                            let k = &args_ts[0];
                            let v = &args_ts[1];
                            return Ok(quote! { (#recv_ts).getOrDefault(&#k, #v) });
                        }
                        _ => {}
                    }
                }

                // CopyOnWriteArrayList.contains(e) / indexOf(e) — pass by ref
                if type_name_matches(recv.ty(), "CopyOnWriteArrayList") {
                    match method_name.as_str() {
                        "contains" | "indexOf" => {
                            let a = &args_ts[0];
                            let m = ident(method_name);
                            return Ok(quote! { (#recv_ts).#m(&#a) });
                        }
                        "remove" if args_ts.len() == 1 => {
                            let a = &args_ts[0];
                            return Ok(quote! { (#recv_ts).remove_at(#a) });
                        }
                        _ => {}
                    }
                }

                // Runtime instance → exec() produces a JProcess.
                if matches!(recv.ty(), IrType::Class(c) if c == "Runtime")
                    && method_name == "exec"
                    && !args_ts.is_empty()
                {
                    let cmd = &args_ts[0];
                    if matches!(args.first().map(|e| e.ty()), Some(IrType::Array(_)))
                        && args_ts.len() == 1
                    {
                        return Ok(quote! { JProcessBuilder::exec_array(#cmd) });
                    }
                    return Ok(quote! { JProcessBuilder::exec_string(#cmd) });
                }

                // Rename Java method names to their Rust runtime equivalents.
                let method = match method_name.as_str() {
                    "await" => ident("await_"),
                    "mod" => ident("mod_"),
                    "charAt" => ident("char_at"),
                    "indexOf" => ident("index_of"),
                    "isEmpty" => ident("isEmpty"),
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
            if IN_STATIC_METHOD.with(|c| c.get()) {
                Ok(quote! { Self::#method(#(#args_ts),*) })
            } else {
                Ok(quote! { self.#method(#(#args_ts),*) })
            }
        }

        IrExpr::New { class, args, .. } => {
            let args_ts: Vec<TokenStream> = args.iter().map(emit_expr).collect::<Result<_, _>>()?;
            // Map Java constructors to their runtime equivalents.
            match class.as_str() {
                "ArrayList" => Ok(quote! { JList::new() }),
                "LinkedList" | "ArrayDeque" => Ok(quote! { JLinkedList::new() }),
                "HashMap" | "Hashtable" => Ok(quote! { JMap::new() }),
                "LinkedHashMap" => Ok(quote! { JLinkedHashMap::new() }),
                "TreeMap" => Ok(quote! { JTreeMap::new() }),
                "HashSet" => Ok(quote! { JSet::new() }),
                "LinkedHashSet" => Ok(quote! { JLinkedHashSet::new() }),
                "TreeSet" => Ok(quote! { JTreeSet::new() }),
                "EnumMap" => Ok(quote! { JEnumMap::new() }),
                "EnumSet" => Ok(quote! { JEnumSet::new() }),
                "PriorityQueue" => Ok(quote! { JPriorityQueue::new() }),
                "AtomicInteger" => Ok(quote! { JAtomicInteger::new(#(#args_ts),*) }),
                "AtomicLong" => Ok(quote! { JAtomicLong::new(#(#args_ts),*) }),
                "AtomicBoolean" => Ok(quote! { JAtomicBoolean::new(#(#args_ts),*) }),
                "CountDownLatch" => Ok(quote! { JCountDownLatch::new(#(#args_ts),*) }),
                "Semaphore" => Ok(quote! { JSemaphore::new(#(#args_ts),*) }),
                "ReentrantLock" => Ok(quote! { JReentrantLock::new() }),
                "ReentrantReadWriteLock" => Ok(quote! { JReentrantReadWriteLock::new() }),
                "ConcurrentHashMap" => Ok(quote! { JConcurrentHashMap::new() }),
                "CopyOnWriteArrayList" => Ok(quote! { JCopyOnWriteArrayList::new() }),
                "ThreadLocal" => Ok(quote! { JThreadLocal::new() }),
                "StringBuilder" => {
                    if args_ts.is_empty() {
                        Ok(quote! { JStringBuilder::new() })
                    } else {
                        Ok(quote! { JStringBuilder::new_from_string(#(#args_ts),*) })
                    }
                }
                "BigInteger" => Ok(quote! { JBigInteger::from_string(#(#args_ts),*) }),
                "BigDecimal" => {
                    if args_ts.is_empty() {
                        Ok(quote! { JBigDecimal::zero() })
                    } else {
                        let a = &args_ts[0];
                        let arg_ty = args.first().map(|e| e.ty());
                        match arg_ty {
                            Some(IrType::Int) | Some(IrType::Long) => {
                                Ok(quote! { JBigDecimal::from_long(#a as i64) })
                            }
                            Some(IrType::Double) | Some(IrType::Float) => {
                                Ok(quote! { JBigDecimal::from_double(#a as f64) })
                            }
                            _ => Ok(quote! { JBigDecimal::from_string(#a) }),
                        }
                    }
                }
                "MathContext" => Ok(quote! { JMathContext::new(#(#args_ts),*) }),
                "URL" => Ok(quote! { JURL::new(#(#args_ts),*) }),
                "Socket" => Ok(quote! { JSocket::new(#(#args_ts),*) }),
                "ServerSocket" => Ok(quote! { JServerSocket::new(#(#args_ts),*) }),
                "File" => {
                    if args_ts.len() == 2 {
                        let a = &args_ts[0];
                        let b = &args_ts[1];
                        Ok(quote! { JFile::new_child(#a, #b) })
                    } else {
                        Ok(quote! { JFile::new(#(#args_ts),*) })
                    }
                }
                "BufferedReader" => {
                    // new BufferedReader(new FileReader(...)) or (new InputStreamReader(...))
                    if args_ts.is_empty() {
                        Ok(quote! { JBufferedReader::new_stdin() })
                    } else {
                        // Detect: new BufferedReader(new InputStreamReader(X.getInputStream()))
                        // or new BufferedReader(new InputStreamReader(X.getErrorStream()))
                        // → lower to X.getInputStream() / X.getErrorStream()
                        if let Some(IrExpr::New {
                            class: inner_cls,
                            args: inner_args,
                            ..
                        }) = args.first()
                        {
                            if inner_cls == "InputStreamReader" {
                                if let Some(first_inner) = inner_args.first() {
                                    if let IrExpr::MethodCall {
                                        receiver: Some(recv),
                                        method_name,
                                        ..
                                    } = first_inner
                                    {
                                        if method_name == "getInputStream"
                                            || method_name == "getErrorStream"
                                        {
                                            let recv_ts = emit_expr(recv)?;
                                            let m = ident(method_name);
                                            return Ok(quote! { (#recv_ts).#m() });
                                        }
                                    }
                                    // Only map InputStreamReader(System.in) → stdin.
                                    let is_system_in = matches!(
                                        first_inner,
                                        IrExpr::FieldAccess { receiver: r, field_name: f, .. }
                                        if f == "in" && matches!(r.as_ref(), IrExpr::Var { name, .. } if name == "System")
                                    );
                                    if is_system_in {
                                        return Ok(quote! { JBufferedReader::new_stdin() });
                                    }
                                    return Err(CodegenError::Unsupported(
                                        "new BufferedReader(new InputStreamReader(...)) is only \
                                         supported for getInputStream()/getErrorStream() or \
                                         System.in"
                                            .into(),
                                    ));
                                }
                                // InputStreamReader with no args → stdin
                                return Ok(quote! { JBufferedReader::new_stdin() });
                            }
                        }
                        let a = &args_ts[0];
                        let arg_ty = args.first().map(|e| e.ty());
                        match arg_ty {
                            Some(IrType::Class(ref c)) if c == "InputStreamReader" => {
                                Ok(quote! { JBufferedReader::new_stdin() })
                            }
                            _ => Ok(quote! { JBufferedReader::from_reader(#a) }),
                        }
                    }
                }
                "BufferedWriter" => {
                    if args_ts.is_empty() {
                        Ok(quote! { JBufferedWriter::default() })
                    } else {
                        let a = &args_ts[0];
                        Ok(quote! { JBufferedWriter::from_writer(#a) })
                    }
                }
                "PrintWriter" => {
                    if args_ts.is_empty() {
                        Ok(quote! { JPrintWriter::default() })
                    } else {
                        let a = &args_ts[0];
                        // Check if arg is a FileWriter/File type or a String path
                        let arg_ty = args.first().map(|e| e.ty());
                        match arg_ty {
                            Some(IrType::Class(ref c)) if c == "FileWriter" => {
                                Ok(quote! { JPrintWriter::from_writer(#a) })
                            }
                            Some(IrType::Class(ref c)) if c == "File" => {
                                Ok(quote! { JPrintWriter::from_file(&#a) })
                            }
                            _ => Ok(quote! { JPrintWriter::new_from_path(#a) }),
                        }
                    }
                }
                "FileReader" => Ok(quote! { JFileReader::new(#(#args_ts),*) }),
                "FileWriter" => {
                    if args_ts.len() == 2 {
                        let a = &args_ts[0];
                        let b = &args_ts[1];
                        Ok(quote! { JFileWriter::new_append(#a, #b) })
                    } else {
                        Ok(quote! { JFileWriter::new(#(#args_ts),*) })
                    }
                }
                "FileInputStream" => Ok(quote! { JFileInputStream::new(#(#args_ts),*) }),
                "FileOutputStream" => {
                    if args_ts.len() == 2 {
                        let a = &args_ts[0];
                        let b = &args_ts[1];
                        Ok(quote! { JFileOutputStream::new_append(#a, #b) })
                    } else {
                        Ok(quote! { JFileOutputStream::new(#(#args_ts),*) })
                    }
                }
                "InputStreamReader" => {
                    // new InputStreamReader(System.in) → placeholder, typically wrapped in BufferedReader
                    Ok(quote! { JFileReader::default() })
                }
                "Scanner" => {
                    if args_ts.is_empty() {
                        Ok(quote! { JScanner::new_stdin() })
                    } else {
                        let a = &args_ts[0];
                        let arg_ty = args.first().map(|e| e.ty());
                        match arg_ty {
                            Some(IrType::Class(ref c)) if c == "File" => {
                                Ok(quote! { JScanner::from_file(&#a) })
                            }
                            _ => Ok(quote! { JScanner::from_string(#a) }),
                        }
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
                "ProcessBuilder" => {
                    // new ProcessBuilder(String... args)  or  new ProcessBuilder(List<String>)
                    if args_ts.is_empty() {
                        Ok(quote! { JProcessBuilder::new_varargs(vec![]) })
                    } else {
                        let arg_ty = args.first().map(|e| e.ty());
                        let is_list = matches!(
                            arg_ty,
                            Some(IrType::Generic { base, .. })
                            if matches!(base.as_ref(),
                                IrType::Class(c) if matches!(c.as_str(),
                                    "List" | "ArrayList" | "Collection"))
                        );
                        if is_list && args_ts.len() == 1 {
                            let a = &args_ts[0];
                            Ok(quote! { JProcessBuilder::new_list(#a) })
                        } else {
                            Ok(quote! { JProcessBuilder::new_varargs(vec![#(#args_ts),*]) })
                        }
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
            // Char arithmetic: Java allows char ± char and char ± int → int.
            if (lhs.ty() == &IrType::Char || rhs.ty() == &IrType::Char)
                && matches!(op, BinOp::Sub | BinOp::Add)
            {
                let l_cast = if lhs.ty() == &IrType::Char {
                    quote! { (#l as i32) }
                } else {
                    l.clone()
                };
                let r_cast = if rhs.ty() == &IrType::Char {
                    quote! { (#r as i32) }
                } else {
                    r.clone()
                };
                let op_ts = emit_binop(op);
                return Ok(quote! { (#l_cast #op_ts #r_cast) });
            }
            // Char comparison — keep as-is (Rust supports char == char).
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

        IrExpr::Lambda {
            params,
            body,
            body_stmts,
            ..
        } => {
            let param_idents: Vec<Ident> = params.iter().map(|p| ident(p)).collect();
            let body_ts = emit_expr(body)?;
            if body_stmts.is_empty() {
                if param_idents.len() == 1 {
                    let p = &param_idents[0];
                    Ok(quote! { |#p| { #body_ts } })
                } else {
                    Ok(quote! { |#(#param_idents),*| { #body_ts } })
                }
            } else {
                let stmts_ts = emit_stmts(body_stmts)?;
                Ok(quote! { |#(#param_idents),*| { #(#stmts_ts)* #body_ts } })
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
            let print_macro = if macro_name == "eprintln" {
                ident("eprint")
            } else {
                ident("print")
            };
            if args.is_empty() {
                Ok(quote! { #print_macro!("") })
            } else {
                let first = &args[0];
                if is_float {
                    Ok(quote! { #print_macro!("{:?}", #first) })
                } else {
                    Ok(quote! { #print_macro!("{}", #first) })
                }
            }
        }
        "printf" | "format" => {
            let print_macro = if macro_name == "eprintln" {
                ident("eprint")
            } else {
                ident("print")
            };
            if args.is_empty() {
                Ok(quote! { #print_macro!("") })
            } else {
                let fmt = &args[0];
                let rest = &args[1..];
                let rest_strs: Vec<TokenStream> = rest
                    .iter()
                    .map(|a| {
                        quote! { format!("{}", #a) }
                    })
                    .collect();
                Ok(quote! { #print_macro!("{}", java_compat::jformat(#fmt, &[#(#rest_strs),*])) })
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

/// Map Java type parameter bounds to extra Rust trait bounds.
///
/// The base bounds `Clone + Default + Debug` are always present. This function
/// returns additional bounds based on the Java `extends` clause:
/// - `Comparable<T>` → `+ PartialOrd + Ord`
/// - `Cloneable` → (already Clone)
/// - `Serializable` → (ignored, no Rust equivalent)
/// - `Iterable` → (not emitted; runtime collection types do not implement IntoIterator)
/// - Other bounds → ignored (covered by the base bounds)
fn extra_bounds_for_type_param(tp: &ir::IrTypeParam) -> TokenStream {
    let mut extra = TokenStream::new();
    for bound in &tp.bounds {
        let bound_name = match bound {
            IrType::Class(name) => name.as_str(),
            IrType::Generic { base, .. } => {
                if let IrType::Class(name) = base.as_ref() {
                    name.as_str()
                } else {
                    continue;
                }
            }
            _ => continue,
        };
        if bound_name == "Comparable" {
            extra.extend(quote! { + PartialOrd + Ord });
        }
        // Cloneable, Serializable, Iterable, etc. — no extra Rust bound needed
    }
    extra
}

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
                "ReentrantLock" => quote! { JReentrantLock },
                "Condition" => quote! { JCondition },
                "ReentrantReadWriteLock" => quote! { JReentrantReadWriteLock },
                "ReadWriteLock" => quote! { JReentrantReadWriteLock },
                "Lock" => quote! { JReentrantLock },
                "ConcurrentHashMap" => quote! { JConcurrentHashMap },
                "CopyOnWriteArrayList" => quote! { JCopyOnWriteArrayList },
                "ThreadLocal" => quote! { JThreadLocal },
                "ExecutorService" => quote! { JExecutorService },
                "Executors" => quote! { JExecutors },
                "Future" => quote! { JFuture },
                "CompletableFuture" => quote! { JCompletableFuture },
                "TimeUnit" => quote! { JTimeUnit },
                "Optional" => quote! { JOptional },
                "StringBuilder" => quote! { JStringBuilder },
                "BigInteger" => quote! { JBigInteger },
                "BigDecimal" => quote! { JBigDecimal },
                "MathContext" => quote! { JMathContext },
                "RoundingMode" => quote! { JRoundingMode },
                "URL" => quote! { JURL },
                "Socket" => quote! { JSocket },
                "ServerSocket" => quote! { JServerSocket },
                "HttpURLConnection" => quote! { JHttpURLConnection },
                "Pattern" => quote! { JPattern },
                "Matcher" => quote! { JMatcher },
                "LocalDate" => quote! { JLocalDate },
                "LocalTime" => quote! { JLocalTime },
                "LocalDateTime" => quote! { JLocalDateTime },
                "Instant" => quote! { JInstant },
                "Duration" => quote! { JDuration },
                "Period" => quote! { JPeriod },
                "DateTimeFormatter" => quote! { JDateTimeFormatter },
                "File" => quote! { JFile },
                "BufferedReader" => quote! { JBufferedReader },
                "BufferedWriter" => quote! { JBufferedWriter },
                "PrintWriter" => quote! { JPrintWriter },
                "FileReader" => quote! { JFileReader },
                "FileWriter" => quote! { JFileWriter },
                "FileInputStream" => quote! { JFileInputStream },
                "FileOutputStream" => quote! { JFileOutputStream },
                "InputStreamReader" => quote! { JFileReader },
                "Scanner" => quote! { JScanner },
                "Path" => quote! { JPath },
                "Files" => quote! { JFiles },
                "JStream" => quote! { JStream },
                // Raw types: collection classes without type parameters → default to JavaObject
                "List" | "ArrayList" | "Collection" | "Iterable" => {
                    quote! { JList<JavaObject> }
                }
                "LinkedList" | "ArrayDeque" => quote! { JLinkedList<JavaObject> },
                "PriorityQueue" => quote! { JPriorityQueue<JavaObject> },
                "Map" | "HashMap" | "Hashtable" => quote! { JMap<JavaObject, JavaObject> },
                "LinkedHashMap" => quote! { JLinkedHashMap<JavaObject, JavaObject> },
                "TreeMap" => quote! { JTreeMap<JavaObject, JavaObject> },
                "Set" | "HashSet" => quote! { JSet<JavaObject> },
                "LinkedHashSet" => quote! { JLinkedHashSet<JavaObject> },
                "TreeSet" => quote! { JTreeSet<JavaObject> },
                "Iterator" => quote! { JIterator<JavaObject> },
                "Map.Entry" => quote! { JMapEntry<JavaObject, JavaObject> },
                "Spliterator" => quote! { JSpliterator<JavaObject> },
                "ProcessBuilder" => quote! { JProcessBuilder },
                "Process" => quote! { JProcess },
                "Object" => quote! { JavaObject },
                _ => {
                    // Resolve through the enum alias map so that a type
                    // annotation like `Season` emits `EnumCompare_Season`
                    // when `Season` is an inner-enum promoted with mangling.
                    let canonical =
                        ENUM_NAMES.with(|names| names.borrow().get(name.as_str()).cloned());
                    let emit_name = canonical.as_deref().unwrap_or(name.as_str());
                    let id = ident(emit_name);
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
                    "List" | "ArrayList" | "Collection" | "Iterable" => {
                        let a = emit_type(args.first().unwrap_or(&IrType::Unknown));
                        return quote! { JList<#a> };
                    }
                    "LinkedList" | "ArrayDeque" => {
                        let a = emit_type(args.first().unwrap_or(&IrType::Unknown));
                        return quote! { JLinkedList<#a> };
                    }
                    "PriorityQueue" => {
                        let a = emit_type(args.first().unwrap_or(&IrType::Unknown));
                        return quote! { JPriorityQueue<#a> };
                    }
                    "Map" | "HashMap" | "Hashtable" => {
                        let k = emit_type(args.first().unwrap_or(&IrType::Unknown));
                        let v = emit_type(args.get(1).unwrap_or(&IrType::Unknown));
                        return quote! { JMap<#k, #v> };
                    }
                    "LinkedHashMap" => {
                        let k = emit_type(args.first().unwrap_or(&IrType::Unknown));
                        let v = emit_type(args.get(1).unwrap_or(&IrType::Unknown));
                        return quote! { JLinkedHashMap<#k, #v> };
                    }
                    "TreeMap" => {
                        let k = emit_type(args.first().unwrap_or(&IrType::Unknown));
                        let v = emit_type(args.get(1).unwrap_or(&IrType::Unknown));
                        return quote! { JTreeMap<#k, #v> };
                    }
                    "EnumMap" => {
                        let k = emit_type(args.first().unwrap_or(&IrType::Unknown));
                        let v = emit_type(args.get(1).unwrap_or(&IrType::Unknown));
                        return quote! { JEnumMap<#k, #v> };
                    }
                    "Set" | "HashSet" => {
                        let a = emit_type(args.first().unwrap_or(&IrType::Unknown));
                        return quote! { JSet<#a> };
                    }
                    "LinkedHashSet" => {
                        let a = emit_type(args.first().unwrap_or(&IrType::Unknown));
                        return quote! { JLinkedHashSet<#a> };
                    }
                    "TreeSet" => {
                        let a = emit_type(args.first().unwrap_or(&IrType::Unknown));
                        return quote! { JTreeSet<#a> };
                    }
                    "EnumSet" => {
                        let a = emit_type(args.first().unwrap_or(&IrType::Unknown));
                        return quote! { JEnumSet<#a> };
                    }
                    "Optional" => {
                        let a = emit_type(args.first().unwrap_or(&IrType::Unknown));
                        return quote! { JOptional<#a> };
                    }
                    "Stream" => {
                        let a = emit_type(args.first().unwrap_or(&IrType::Unknown));
                        return quote! { JStream<#a> };
                    }
                    "Map.Entry" => {
                        let k = emit_type(args.first().unwrap_or(&IrType::Unknown));
                        let v = emit_type(args.get(1).unwrap_or(&IrType::Unknown));
                        return quote! { JMapEntry<#k, #v> };
                    }
                    "Spliterator" => {
                        let a = emit_type(args.first().unwrap_or(&IrType::Unknown));
                        return quote! { JSpliterator<#a> };
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
        IrType::Wildcard { bound: _ } => {
            // Rust has no wildcards — erase all wildcards to JavaObject to
            // avoid referencing unmapped JDK bound types (e.g., Number).
            quote! { JavaObject }
        }
    }
}

// ─── helpers ─────────────────────────────────────────────────────────────────

/// Check if an IrType has the given class name (works for both Class and Generic).
fn type_name_matches(ty: &IrType, name: &str) -> bool {
    match ty {
        IrType::Class(c) => c == name,
        IrType::Generic { base, .. } => type_name_matches(base, name),
        _ => false,
    }
}

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
            }],
            constructors: vec![],
        }
    }

    fn gen(module: &IrModule) -> String {
        generate(module).expect("codegen should succeed")
    }

    // ── Basic generation ──────────────────────────────────────────────────

    #[test]
    fn generate_empty_module() {
        let module = IrModule::new("");
        let result = generate(&module);
        assert!(result.is_ok(), "empty module should generate without error");
    }

    /// Helper: construct System.out.println(expr) the way the real parser does.
    fn sysout_println(arg: IrExpr) -> IrStmt {
        IrStmt::Expr(IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::FieldAccess {
                receiver: Box::new(IrExpr::Var {
                    name: "System".into(),
                    ty: IrType::Class("System".into()),
                }),
                field_name: "out".into(),
                ty: IrType::Class("PrintStream".into()),
            })),
            method_name: "println".into(),
            args: vec![arg],
            ty: IrType::Void,
        })
    }

    #[test]
    fn generate_hello_world() {
        let mut module = IrModule::new("");
        let print = sysout_println(IrExpr::LitString("Hello".into()));
        module
            .decls
            .push(IrDecl::Class(make_class("Hello", vec![print])));
        let code = gen(&module);
        assert!(code.contains("println!"), "should contain println macro");
        assert!(code.contains("Hello"), "should contain the string literal");
        assert!(code.contains("fn main()"), "should contain fn main()");
    }

    // ── Expressions ───────────────────────────────────────────────────────

    #[test]
    fn generate_arithmetic_ops() {
        let mut module = IrModule::new("");
        let expr = IrExpr::BinOp {
            op: IrBinOp::Add,
            lhs: Box::new(IrExpr::LitInt(1)),
            rhs: Box::new(IrExpr::LitInt(2)),
            ty: IrType::Int,
        };
        let stmt = sysout_println(expr);
        module
            .decls
            .push(IrDecl::Class(make_class("Arith", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains('+'), "should contain addition");
    }

    #[test]
    fn generate_comparison_ops() {
        let mut module = IrModule::new("");
        let ops = vec![
            IrBinOp::Lt,
            IrBinOp::Le,
            IrBinOp::Gt,
            IrBinOp::Ge,
            IrBinOp::Eq,
            IrBinOp::Ne,
        ];
        let mut stmts = Vec::new();
        for op in ops {
            let expr = IrExpr::BinOp {
                op,
                lhs: Box::new(IrExpr::LitInt(1)),
                rhs: Box::new(IrExpr::LitInt(2)),
                ty: IrType::Bool,
            };
            stmts.push(sysout_println(expr));
        }
        module.decls.push(IrDecl::Class(make_class("Comp", stmts)));
        let code = gen(&module);
        assert!(code.contains('<'), "should contain less-than");
        assert!(code.contains("!="), "should contain not-equal");
    }

    #[test]
    fn generate_unary_ops() {
        let mut module = IrModule::new("");
        let neg = IrExpr::UnOp {
            op: IrUnOp::Neg,
            operand: Box::new(IrExpr::LitInt(5)),
            ty: IrType::Int,
        };
        let not = IrExpr::UnOp {
            op: IrUnOp::Not,
            operand: Box::new(IrExpr::LitBool(true)),
            ty: IrType::Bool,
        };
        let bitnot = IrExpr::UnOp {
            op: IrUnOp::BitNot,
            operand: Box::new(IrExpr::LitInt(0)),
            ty: IrType::Int,
        };
        let stmts: Vec<IrStmt> = vec![neg, not, bitnot]
            .into_iter()
            .map(sysout_println)
            .collect();
        module.decls.push(IrDecl::Class(make_class("Unary", stmts)));
        let code = gen(&module);
        assert!(code.contains('-'), "should contain negation");
        assert!(code.contains('!'), "should contain not");
    }

    #[test]
    fn generate_ternary() {
        let mut module = IrModule::new("");
        let ternary = IrExpr::Ternary {
            cond: Box::new(IrExpr::LitBool(true)),
            then_: Box::new(IrExpr::LitInt(1)),
            else_: Box::new(IrExpr::LitInt(2)),
            ty: IrType::Int,
        };
        let stmt = sysout_println(ternary);
        module
            .decls
            .push(IrDecl::Class(make_class("Tern", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("if"), "ternary should use if expression");
    }

    #[test]
    fn generate_string_concat() {
        let mut module = IrModule::new("");
        let concat = IrExpr::BinOp {
            op: IrBinOp::Concat,
            lhs: Box::new(IrExpr::LitString("a".into())),
            rhs: Box::new(IrExpr::LitString("b".into())),
            ty: IrType::String,
        };
        let stmt = sysout_println(concat);
        module
            .decls
            .push(IrDecl::Class(make_class("Cat", vec![stmt])));
        let code = gen(&module);
        // Concat produces JString::from(format!(...)) or similar
        assert!(
            code.contains("JString::from") || code.contains("format!"),
            "string concat should use JString::from or format!, got: {}",
            code
        );
    }

    #[test]
    fn generate_cast() {
        let mut module = IrModule::new("");
        let cast = IrExpr::Cast {
            target: IrType::Double,
            expr: Box::new(IrExpr::LitInt(42)),
        };
        let stmt = sysout_println(cast);
        module
            .decls
            .push(IrDecl::Class(make_class("CastTest", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("as f64"), "should contain cast to f64");
    }

    #[test]
    fn generate_literal_types() {
        let mut module = IrModule::new("");
        let lits = vec![
            IrExpr::LitBool(true),
            IrExpr::LitInt(42),
            IrExpr::LitLong(100),
            IrExpr::LitFloat(1.5),
            IrExpr::LitDouble(2.5),
            IrExpr::LitChar('A'),
            IrExpr::LitString("test".into()),
            IrExpr::LitNull,
        ];
        let stmts: Vec<IrStmt> = lits.into_iter().map(sysout_println).collect();
        module.decls.push(IrDecl::Class(make_class("Lits", stmts)));
        let code = gen(&module);
        assert!(code.contains("true"), "should contain bool literal");
        assert!(code.contains("42"), "should contain int literal");
        assert!(code.contains("test"), "should contain string literal");
    }

    // ── Statements ────────────────────────────────────────────────────────

    #[test]
    fn generate_if_else() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::If {
            cond: IrExpr::LitBool(true),
            then_: vec![IrStmt::Return(None)],
            else_: Some(vec![IrStmt::Return(None)]),
        };
        module
            .decls
            .push(IrDecl::Class(make_class("IfElse", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("if"), "should contain if");
        assert!(code.contains("else"), "should contain else");
    }

    #[test]
    fn generate_while_loop() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::While {
            cond: IrExpr::LitBool(false),
            body: vec![IrStmt::Break(None)],
        };
        module
            .decls
            .push(IrDecl::Class(make_class("WhileLoop", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("while"), "should contain while");
        assert!(code.contains("break"), "should contain break");
    }

    #[test]
    fn generate_do_while() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::DoWhile {
            body: vec![IrStmt::Continue(None)],
            cond: IrExpr::LitBool(false),
        };
        module
            .decls
            .push(IrDecl::Class(make_class("DoWhile", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("loop"), "do-while should use loop");
    }

    #[test]
    fn generate_for_loop() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::For {
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
                ty: IrType::Bool,
            }),
            update: vec![IrExpr::UnOp {
                op: IrUnOp::PostInc,
                operand: Box::new(IrExpr::Var {
                    name: "i".into(),
                    ty: IrType::Int,
                }),
                ty: IrType::Int,
            }],
            body: vec![],
        };
        module
            .decls
            .push(IrDecl::Class(make_class("ForLoop", vec![stmt])));
        gen(&module); // just verify it doesn't fail
    }

    #[test]
    fn generate_for_each() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::ForEach {
            var: "x".into(),
            var_ty: IrType::Int,
            iterable: IrExpr::Var {
                name: "list".into(),
                ty: IrType::Class("ArrayList".into()),
            },
            body: vec![],
        };
        let init = IrStmt::LocalVar {
            name: "list".into(),
            ty: IrType::Class("ArrayList".into()),
            init: Some(IrExpr::New {
                class: "ArrayList".into(),
                args: vec![],
                ty: IrType::Class("ArrayList".into()),
            }),
        };
        module
            .decls
            .push(IrDecl::Class(make_class("ForEach", vec![init, stmt])));
        let code = gen(&module);
        assert!(code.contains("for"), "should contain for loop");
    }

    #[test]
    fn generate_switch() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::Switch {
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
        let init = IrStmt::LocalVar {
            name: "x".into(),
            ty: IrType::Int,
            init: Some(IrExpr::LitInt(1)),
        };
        module
            .decls
            .push(IrDecl::Class(make_class("Switch", vec![init, stmt])));
        let code = gen(&module);
        assert!(code.contains("match"), "switch should use match");
    }

    #[test]
    fn generate_try_catch() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::TryCatch {
            body: vec![IrStmt::Expr(IrExpr::MethodCall {
                receiver: None,
                method_name: "System.out.println".into(),
                args: vec![IrExpr::LitString("try".into())],
                ty: IrType::Void,
            })],
            catches: vec![CatchClause {
                exception_types: vec!["Exception".into()],
                var: "e".into(),
                body: vec![],
            }],
            finally: Some(vec![IrStmt::Expr(IrExpr::MethodCall {
                receiver: None,
                method_name: "System.out.println".into(),
                args: vec![IrExpr::LitString("finally".into())],
                ty: IrType::Void,
            })]),
        };
        module
            .decls
            .push(IrDecl::Class(make_class("TryCatch", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("catch_unwind"), "try should use catch_unwind");
    }

    #[test]
    fn generate_throw() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::Throw(IrExpr::New {
            class: "RuntimeException".into(),
            args: vec![IrExpr::LitString("oops".into())],
            ty: IrType::Class("RuntimeException".into()),
        });
        module
            .decls
            .push(IrDecl::Class(make_class("Throw", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("panic!"), "throw should use panic!");
    }

    #[test]
    fn generate_local_var() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::LocalVar {
            name: "x".into(),
            ty: IrType::Int,
            init: Some(IrExpr::LitInt(42)),
        };
        module
            .decls
            .push(IrDecl::Class(make_class("LocalVar", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("let mut x"), "should declare mutable local");
        assert!(code.contains("i32"), "should have correct type");
    }

    #[test]
    fn generate_return_value() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::Return(Some(IrExpr::LitInt(42)));
        module
            .decls
            .push(IrDecl::Class(make_class("Ret", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("return"), "should contain return");
    }

    // ── Classes and OOP ───────────────────────────────────────────────────

    #[test]
    fn generate_class_with_field() {
        let mut module = IrModule::new("");
        let mut cls = make_class("Point", vec![]);
        cls.fields.push(IrField {
            name: "x".into(),
            ty: IrType::Int,
            visibility: Visibility::Public,
            is_static: false,
            is_final: false,
            is_volatile: false,
            init: None,
        });
        cls.fields.push(IrField {
            name: "y".into(),
            ty: IrType::Int,
            visibility: Visibility::Public,
            is_static: false,
            is_final: false,
            is_volatile: false,
            init: None,
        });
        module.decls.push(IrDecl::Class(cls));
        let code = gen(&module);
        assert!(code.contains("pub x: i32"), "should have field x");
        assert!(code.contains("pub y: i32"), "should have field y");
    }

    #[test]
    fn generate_constructor() {
        let mut module = IrModule::new("");
        let mut cls = make_class("Foo", vec![]);
        cls.constructors.push(IrConstructor {
            visibility: Visibility::Public,
            params: vec![IrParam {
                name: "val".into(),
                ty: IrType::Int,
                is_varargs: false,
            }],
            body: vec![IrStmt::Expr(IrExpr::Assign {
                lhs: Box::new(IrExpr::FieldAccess {
                    receiver: Box::new(IrExpr::Var {
                        name: "this".into(),
                        ty: IrType::Class("Foo".into()),
                    }),
                    field_name: "val".into(),
                    ty: IrType::Int,
                }),
                rhs: Box::new(IrExpr::Var {
                    name: "val".into(),
                    ty: IrType::Int,
                }),
                ty: IrType::Int,
            })],
            throws: vec![],
        });
        cls.fields.push(IrField {
            name: "val".into(),
            ty: IrType::Int,
            visibility: Visibility::Public,
            is_static: false,
            is_final: false,
            is_volatile: false,
            init: None,
        });
        module.decls.push(IrDecl::Class(cls));
        let code = gen(&module);
        assert!(code.contains("fn new"), "should contain constructor");
    }

    #[test]
    fn generate_instance_method() {
        let mut module = IrModule::new("");
        let mut cls = make_class("Counter", vec![]);
        // Remove the main method and add an instance method
        cls.methods = vec![IrMethod {
            name: "inc".into(),
            visibility: Visibility::Public,
            is_static: false,
            is_abstract: false,
            is_final: false,
            is_synchronized: false,
            type_params: vec![],
            params: vec![],
            return_ty: IrType::Void,
            body: Some(vec![]),
            throws: vec![],
        }];
        module.decls.push(IrDecl::Class(cls));
        let code = gen(&module);
        assert!(
            code.contains("&mut self"),
            "instance method should have self"
        );
    }

    #[test]
    fn generate_interface() {
        let mut module = IrModule::new("");
        module.decls.push(IrDecl::Interface(IrInterface {
            name: "Greetable".into(),
            visibility: Visibility::Public,
            type_params: vec![],
            extends: vec![],
            methods: vec![IrMethod {
                name: "greet".into(),
                visibility: Visibility::Public,
                is_static: false,
                is_abstract: true,
                is_final: false,
                is_synchronized: false,
                type_params: vec![],
                params: vec![],
                return_ty: IrType::String,
                body: None,
                throws: vec![],
            }],
        }));
        let code = gen(&module);
        assert!(code.contains("trait Greetable"), "should contain trait");
        assert!(code.contains("fn greet"), "should contain method signature");
    }

    // ── Collection / new expressions ──────────────────────────────────────

    #[test]
    fn generate_new_array() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::LocalVar {
            name: "arr".into(),
            ty: IrType::Array(Box::new(IrType::Int)),
            init: Some(IrExpr::NewArray {
                elem_ty: IrType::Int,
                len: Box::new(IrExpr::LitInt(10)),
                ty: IrType::Array(Box::new(IrType::Int)),
            }),
        };
        module
            .decls
            .push(IrDecl::Class(make_class("ArrTest", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("JArray"), "should contain JArray");
    }

    #[test]
    fn generate_new_list() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::LocalVar {
            name: "list".into(),
            ty: IrType::Class("ArrayList".into()),
            init: Some(IrExpr::New {
                class: "ArrayList".into(),
                args: vec![],
                ty: IrType::Class("ArrayList".into()),
            }),
        };
        module
            .decls
            .push(IrDecl::Class(make_class("ListTest", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("JList::new()"),
            "ArrayList should become JList"
        );
    }

    #[test]
    fn generate_new_map() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::LocalVar {
            name: "map".into(),
            ty: IrType::Class("HashMap".into()),
            init: Some(IrExpr::New {
                class: "HashMap".into(),
                args: vec![],
                ty: IrType::Class("HashMap".into()),
            }),
        };
        module
            .decls
            .push(IrDecl::Class(make_class("MapTest", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("JMap::new()"), "HashMap should become JMap");
    }

    // ── Bitwise and logical ops ───────────────────────────────────────────

    #[test]
    fn generate_bitwise_ops() {
        let mut module = IrModule::new("");
        let ops = vec![
            IrBinOp::BitAnd,
            IrBinOp::BitOr,
            IrBinOp::BitXor,
            IrBinOp::Shl,
            IrBinOp::Shr,
        ];
        let stmts: Vec<IrStmt> = ops
            .into_iter()
            .map(|op| {
                sysout_println(IrExpr::BinOp {
                    op,
                    lhs: Box::new(IrExpr::LitInt(0xFF)),
                    rhs: Box::new(IrExpr::LitInt(4)),
                    ty: IrType::Int,
                })
            })
            .collect();
        module
            .decls
            .push(IrDecl::Class(make_class("Bitwise", stmts)));
        let code = gen(&module);
        assert!(code.contains("<<"), "should contain shl");
        assert!(code.contains(">>"), "should contain shr");
    }

    // ── Compound assignment ───────────────────────────────────────────────

    #[test]
    fn generate_compound_assign() {
        let mut module = IrModule::new("");
        let init = IrStmt::LocalVar {
            name: "x".into(),
            ty: IrType::Int,
            init: Some(IrExpr::LitInt(10)),
        };
        let add_assign = IrStmt::Expr(IrExpr::CompoundAssign {
            op: IrBinOp::Add,
            lhs: Box::new(IrExpr::Var {
                name: "x".into(),
                ty: IrType::Int,
            }),
            rhs: Box::new(IrExpr::LitInt(5)),
            ty: IrType::Int,
        });
        module.decls.push(IrDecl::Class(make_class(
            "CompAssign",
            vec![init, add_assign],
        )));
        let code = gen(&module);
        assert!(code.contains("+="), "should contain +=");
    }

    // ── Type mapping ──────────────────────────────────────────────────────

    #[test]
    fn generate_all_primitive_types() {
        let mut module = IrModule::new("");
        let types = vec![
            ("a", IrType::Int),
            ("b", IrType::Long),
            ("c", IrType::Double),
            ("d", IrType::Float),
            ("e", IrType::Bool),
            ("f", IrType::Char),
            ("g", IrType::String),
        ];
        let stmts: Vec<IrStmt> = types
            .into_iter()
            .map(|(name, ty)| IrStmt::LocalVar {
                name: name.into(),
                ty,
                init: None,
            })
            .collect();
        module.decls.push(IrDecl::Class(make_class("Types", stmts)));
        let code = gen(&module);
        assert!(code.contains("i32"), "should contain i32");
        assert!(code.contains("i64"), "should contain i64");
        assert!(code.contains("f64"), "should contain f64");
        assert!(code.contains("f32"), "should contain f32");
        assert!(code.contains("bool"), "should contain bool");
        assert!(code.contains("char"), "should contain char");
        assert!(code.contains("JString"), "should contain JString");
    }

    // ── Assignment expression ─────────────────────────────────────────────

    #[test]
    fn generate_assignment() {
        let mut module = IrModule::new("");
        let init = IrStmt::LocalVar {
            name: "x".into(),
            ty: IrType::Int,
            init: Some(IrExpr::LitInt(0)),
        };
        let assign = IrStmt::Expr(IrExpr::Assign {
            lhs: Box::new(IrExpr::Var {
                name: "x".into(),
                ty: IrType::Int,
            }),
            rhs: Box::new(IrExpr::LitInt(42)),
            ty: IrType::Int,
        });
        module
            .decls
            .push(IrDecl::Class(make_class("Assign", vec![init, assign])));
        gen(&module);
    }

    // ── Instanceof and FieldAccess ────────────────────────────────────────

    #[test]
    fn generate_instanceof() {
        let mut module = IrModule::new("");
        let expr = IrExpr::InstanceOf {
            expr: Box::new(IrExpr::Var {
                name: "obj".into(),
                ty: IrType::Class("Object".into()),
            }),
            check_type: IrType::Class("String".into()),
        };
        let init = IrStmt::LocalVar {
            name: "obj".into(),
            ty: IrType::Class("Object".into()),
            init: None,
        };
        let check = IrStmt::If {
            cond: expr,
            then_: vec![],
            else_: None,
        };
        module.decls.push(IrDecl::Class(make_class(
            "InstanceOfTest",
            vec![init, check],
        )));
        let code = gen(&module);
        assert!(
            code.contains("_instanceof"),
            "should contain instanceof check"
        );
    }

    // ── Block statements ──────────────────────────────────────────────────

    #[test]
    fn generate_block() {
        let mut module = IrModule::new("");
        let block = IrStmt::Block(vec![IrStmt::LocalVar {
            name: "inner".into(),
            ty: IrType::Int,
            init: Some(IrExpr::LitInt(1)),
        }]);
        module
            .decls
            .push(IrDecl::Class(make_class("BlockTest", vec![block])));
        gen(&module);
    }

    // ── Static field (class constant) ─────────────────────────────────────

    #[test]
    fn generate_static_field() {
        let mut module = IrModule::new("");
        let mut cls = make_class("StaticField", vec![]);
        cls.fields.push(IrField {
            name: "MAX".into(),
            ty: IrType::Int,
            visibility: Visibility::Public,
            is_static: true,
            is_final: true,
            is_volatile: false,
            init: Some(IrExpr::LitInt(100)),
        });
        module.decls.push(IrDecl::Class(cls));
        let code = gen(&module);
        // Static fields are typically emitted as consts or statics
        assert!(code.contains("100"), "should contain static field value");
    }

    // ── Lambda expression ─────────────────────────────────────────────────

    #[test]
    fn generate_lambda() {
        let mut module = IrModule::new("");
        let lambda = IrExpr::Lambda {
            params: vec!["x".into()],
            body: Box::new(IrExpr::BinOp {
                op: IrBinOp::Mul,
                lhs: Box::new(IrExpr::Var {
                    name: "x".into(),
                    ty: IrType::Int,
                }),
                rhs: Box::new(IrExpr::LitInt(2)),
                ty: IrType::Int,
            }),
            body_stmts: vec![],
            ty: IrType::Class("Function".into()),
        };
        let stmt = IrStmt::LocalVar {
            name: "f".into(),
            ty: IrType::Class("Function".into()),
            init: Some(lambda),
        };
        module
            .decls
            .push(IrDecl::Class(make_class("LambdaTest", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("|"), "lambda should contain closure syntax");
    }

    // ── Array access ──────────────────────────────────────────────────────

    #[test]
    fn generate_array_access() {
        let mut module = IrModule::new("");
        let arr_init = IrStmt::LocalVar {
            name: "arr".into(),
            ty: IrType::Array(Box::new(IrType::Int)),
            init: Some(IrExpr::NewArray {
                elem_ty: IrType::Int,
                len: Box::new(IrExpr::LitInt(5)),
                ty: IrType::Array(Box::new(IrType::Int)),
            }),
        };
        let access = IrStmt::Expr(IrExpr::ArrayAccess {
            array: Box::new(IrExpr::Var {
                name: "arr".into(),
                ty: IrType::Array(Box::new(IrType::Int)),
            }),
            index: Box::new(IrExpr::LitInt(0)),
            ty: IrType::Int,
        });
        module.decls.push(IrDecl::Class(make_class(
            "ArrAccess",
            vec![arr_init, access],
        )));
        let code = gen(&module);
        assert!(code.contains("get("), "array access should use get()");
    }

    // ── Pre/Post increment ────────────────────────────────────────────────

    #[test]
    fn generate_pre_post_increment() {
        let mut module = IrModule::new("");
        let init = IrStmt::LocalVar {
            name: "x".into(),
            ty: IrType::Int,
            init: Some(IrExpr::LitInt(0)),
        };
        let pre_inc = IrStmt::Expr(IrExpr::UnOp {
            op: IrUnOp::PreInc,
            operand: Box::new(IrExpr::Var {
                name: "x".into(),
                ty: IrType::Int,
            }),
            ty: IrType::Int,
        });
        let post_inc = IrStmt::Expr(IrExpr::UnOp {
            op: IrUnOp::PostInc,
            operand: Box::new(IrExpr::Var {
                name: "x".into(),
                ty: IrType::Int,
            }),
            ty: IrType::Int,
        });
        let pre_dec = IrStmt::Expr(IrExpr::UnOp {
            op: IrUnOp::PreDec,
            operand: Box::new(IrExpr::Var {
                name: "x".into(),
                ty: IrType::Int,
            }),
            ty: IrType::Int,
        });
        let post_dec = IrStmt::Expr(IrExpr::UnOp {
            op: IrUnOp::PostDec,
            operand: Box::new(IrExpr::Var {
                name: "x".into(),
                ty: IrType::Int,
            }),
            ty: IrType::Int,
        });
        module.decls.push(IrDecl::Class(make_class(
            "IncDec",
            vec![init, pre_inc, post_inc, pre_dec, post_dec],
        )));
        let code = gen(&module);
        assert!(code.contains("+="), "increment should use +=");
    }

    // ── Method renaming ───────────────────────────────────────────────────

    #[test]
    fn generate_char_at_rename() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "s".into(),
                ty: IrType::String,
            })),
            method_name: "charAt".into(),
            args: vec![IrExpr::LitInt(0)],
            ty: IrType::Char,
        };
        let init = IrStmt::LocalVar {
            name: "s".into(),
            ty: IrType::String,
            init: Some(IrExpr::LitString("hello".into())),
        };
        let stmt = IrStmt::Expr(call);
        module
            .decls
            .push(IrDecl::Class(make_class("CharAt", vec![init, stmt])));
        let code = gen(&module);
        assert!(code.contains("char_at"), "charAt should become char_at");
    }

    // ── String.equals special-case ────────────────────────────────────────

    #[test]
    fn generate_string_equals_adds_ref() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "s".into(),
                ty: IrType::String,
            })),
            method_name: "equals".into(),
            args: vec![IrExpr::Var {
                name: "other".into(),
                ty: IrType::String,
            }],
            ty: IrType::Bool,
        };
        let init1 = IrStmt::LocalVar {
            name: "s".into(),
            ty: IrType::String,
            init: Some(IrExpr::LitString("a".into())),
        };
        let init2 = IrStmt::LocalVar {
            name: "other".into(),
            ty: IrType::String,
            init: Some(IrExpr::LitString("b".into())),
        };
        let stmt = IrStmt::Expr(call);
        module.decls.push(IrDecl::Class(make_class(
            "StrEquals",
            vec![init1, init2, stmt],
        )));
        let code = gen(&module);
        assert!(
            code.contains(".equals(& ") || code.contains(".equals(&"),
            "String equals should add &, got: {}",
            code
        );
    }

    #[test]
    fn generate_non_string_equals_no_ref() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "a".into(),
                ty: IrType::Class("Box".into()),
            })),
            method_name: "equals".into(),
            args: vec![IrExpr::Var {
                name: "b".into(),
                ty: IrType::Class("Box".into()),
            }],
            ty: IrType::Bool,
        };
        let init1 = IrStmt::LocalVar {
            name: "a".into(),
            ty: IrType::Class("Box".into()),
            init: None,
        };
        let init2 = IrStmt::LocalVar {
            name: "b".into(),
            ty: IrType::Class("Box".into()),
            init: None,
        };
        let stmt = IrStmt::Expr(call);
        module.decls.push(IrDecl::Class(make_class(
            "BoxEquals",
            vec![init1, init2, stmt],
        )));
        let code = gen(&module);
        assert!(
            !code.contains("equals(& "),
            "non-String equals should NOT add &"
        );
    }

    // ── Static vs instance method context ─────────────────────────────────

    #[test]
    fn generate_static_method_uses_self_uppercase() {
        let mut module = IrModule::new("");
        let mut cls = make_class("StaticCtx", vec![]);
        cls.methods.push(IrMethod {
            name: "helper".into(),
            visibility: Visibility::Public,
            is_static: true,
            is_abstract: false,
            is_final: false,
            is_synchronized: false,
            type_params: vec![],
            params: vec![],
            return_ty: IrType::Void,
            body: Some(vec![]),
            throws: vec![],
        });
        // Make the main method call helper()
        cls.methods[0].body = Some(vec![IrStmt::Expr(IrExpr::MethodCall {
            receiver: None,
            method_name: "helper".into(),
            args: vec![],
            ty: IrType::Void,
        })]);
        module.decls.push(IrDecl::Class(cls));
        let code = gen(&module);
        assert!(
            code.contains("Self::helper"),
            "static context should use Self::"
        );
    }

    // ── Inheritance ───────────────────────────────────────────────────────

    #[test]
    fn generate_class_with_superclass() {
        let mut module = IrModule::new("");
        // Parent class: Animal with a speak() method
        let parent = IrClass {
            name: "Animal".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            type_params: vec![],
            superclass: None,
            interfaces: vec![],
            fields: vec![IrField {
                name: "name".into(),
                ty: IrType::String,
                visibility: Visibility::Public,
                is_static: false,
                is_final: false,
                is_volatile: false,
                init: None,
            }],
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
            }],
            constructors: vec![],
        };
        // Child class: Dog extends Animal
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
                body: Some(vec![]),
                throws: vec![],
            }],
            constructors: vec![],
        };
        module.decls.push(IrDecl::Class(parent));
        module.decls.push(IrDecl::Class(child));
        let code = gen(&module);
        assert!(
            code.contains("_super: Animal"),
            "child should have parent field"
        );
    }

    #[test]
    fn generate_generic_class() {
        let mut module = IrModule::new("");
        let cls = IrClass {
            name: "Container".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            type_params: vec!["T".into()],
            superclass: None,
            interfaces: vec![],
            fields: vec![IrField {
                name: "value".into(),
                ty: IrType::TypeVar("T".into()),
                visibility: Visibility::Public,
                is_static: false,
                is_final: false,
                is_volatile: false,
                init: None,
            }],
            methods: vec![],
            constructors: vec![],
        };
        module.decls.push(IrDecl::Class(cls));
        let code = gen(&module);
        assert!(
            code.contains("Container<T>") || code.contains("Container < T >"),
            "should have generic parameter"
        );
    }

    #[test]
    fn generate_class_implements_interface() {
        let mut module = IrModule::new("");
        module.decls.push(IrDecl::Interface(IrInterface {
            name: "Greetable".into(),
            visibility: Visibility::Public,
            type_params: vec![],
            extends: vec![],
            methods: vec![IrMethod {
                name: "greet".into(),
                visibility: Visibility::Public,
                is_static: false,
                is_abstract: true,
                is_final: false,
                is_synchronized: false,
                type_params: vec![],
                params: vec![],
                return_ty: IrType::String,
                body: None,
                throws: vec![],
            }],
        }));
        let cls = IrClass {
            name: "Hello".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            type_params: vec![],
            superclass: None,
            interfaces: vec!["Greetable".into()],
            fields: vec![],
            methods: vec![
                IrMethod {
                    name: "greet".into(),
                    visibility: Visibility::Public,
                    is_static: false,
                    is_abstract: false,
                    is_final: false,
                    is_synchronized: false,
                    type_params: vec![],
                    params: vec![],
                    return_ty: IrType::String,
                    body: Some(vec![IrStmt::Return(Some(IrExpr::LitString("hi".into())))]),
                    throws: vec![],
                },
                IrMethod {
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
                    body: Some(vec![]),
                    throws: vec![],
                },
            ],
            constructors: vec![],
        };
        module.decls.push(IrDecl::Class(cls));
        let code = gen(&module);
        assert!(
            code.contains("impl Greetable for Hello"),
            "should implement trait"
        );
    }

    // ── Math static methods ───────────────────────────────────────────────

    #[test]
    fn generate_math_abs() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "Math".into(),
                ty: IrType::Class("Math".into()),
            })),
            method_name: "abs".into(),
            args: vec![IrExpr::LitInt(-5)],
            ty: IrType::Int,
        };
        let stmt = sysout_println(call);
        module
            .decls
            .push(IrDecl::Class(make_class("MathAbs", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains(".abs()"), "should contain abs()");
    }

    #[test]
    fn generate_math_max_min() {
        let mut module = IrModule::new("");
        let max_call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "Math".into(),
                ty: IrType::Class("Math".into()),
            })),
            method_name: "max".into(),
            args: vec![IrExpr::LitInt(1), IrExpr::LitInt(2)],
            ty: IrType::Int,
        };
        let min_call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "Math".into(),
                ty: IrType::Class("Math".into()),
            })),
            method_name: "min".into(),
            args: vec![IrExpr::LitInt(1), IrExpr::LitInt(2)],
            ty: IrType::Int,
        };
        let stmts = vec![sysout_println(max_call), sysout_println(min_call)];
        module
            .decls
            .push(IrDecl::Class(make_class("MathMaxMin", stmts)));
        let code = gen(&module);
        assert!(code.contains("__a"), "should contain max/min helper vars");
    }

    #[test]
    fn generate_math_pow_sqrt() {
        let mut module = IrModule::new("");
        let pow = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "Math".into(),
                ty: IrType::Class("Math".into()),
            })),
            method_name: "pow".into(),
            args: vec![IrExpr::LitDouble(2.0), IrExpr::LitDouble(3.0)],
            ty: IrType::Double,
        };
        let sqrt = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "Math".into(),
                ty: IrType::Class("Math".into()),
            })),
            method_name: "sqrt".into(),
            args: vec![IrExpr::LitDouble(4.0)],
            ty: IrType::Double,
        };
        let stmts = vec![sysout_println(pow), sysout_println(sqrt)];
        module
            .decls
            .push(IrDecl::Class(make_class("MathPowSqrt", stmts)));
        let code = gen(&module);
        assert!(code.contains("powf"), "should contain powf");
        assert!(code.contains("sqrt"), "should contain sqrt");
    }

    #[test]
    fn generate_math_floor_ceil_round() {
        let mut module = IrModule::new("");
        let methods = vec!["floor", "ceil", "round"];
        let stmts: Vec<IrStmt> = methods
            .into_iter()
            .map(|m| {
                sysout_println(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "Math".into(),
                        ty: IrType::Class("Math".into()),
                    })),
                    method_name: m.into(),
                    args: vec![IrExpr::LitDouble(1.5)],
                    ty: IrType::Double,
                })
            })
            .collect();
        module
            .decls
            .push(IrDecl::Class(make_class("MathRound", stmts)));
        let code = gen(&module);
        assert!(code.contains("floor"), "should contain floor");
        assert!(code.contains("ceil"), "should contain ceil");
        assert!(code.contains("round"), "should contain round");
    }

    #[test]
    fn generate_math_trig() {
        let mut module = IrModule::new("");
        let methods = vec!["sin", "cos", "tan", "log", "exp"];
        let stmts: Vec<IrStmt> = methods
            .into_iter()
            .map(|m| {
                sysout_println(IrExpr::MethodCall {
                    receiver: Some(Box::new(IrExpr::Var {
                        name: "Math".into(),
                        ty: IrType::Class("Math".into()),
                    })),
                    method_name: m.into(),
                    args: vec![IrExpr::LitDouble(1.0)],
                    ty: IrType::Double,
                })
            })
            .collect();
        module
            .decls
            .push(IrDecl::Class(make_class("MathTrig", stmts)));
        let code = gen(&module);
        assert!(code.contains("sin"), "should contain sin");
        assert!(code.contains("cos"), "should contain cos");
    }

    #[test]
    fn generate_math_random() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "Math".into(),
                ty: IrType::Class("Math".into()),
            })),
            method_name: "random".into(),
            args: vec![],
            ty: IrType::Double,
        };
        let stmt = sysout_println(call);
        module
            .decls
            .push(IrDecl::Class(make_class("MathRandom", vec![stmt])));
        gen(&module);
    }

    // ── Standard library mappings ─────────────────────────────────────────

    #[test]
    fn generate_integer_parseint() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "Integer".into(),
                ty: IrType::Class("Integer".into()),
            })),
            method_name: "parseInt".into(),
            args: vec![IrExpr::LitString("42".into())],
            ty: IrType::Int,
        };
        let stmt = sysout_println(call);
        module
            .decls
            .push(IrDecl::Class(make_class("ParseInt", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("parse::"),
            "should contain parse::, got: {}",
            code
        );
    }

    #[test]
    fn generate_string_valueof() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "String".into(),
                ty: IrType::Class("String".into()),
            })),
            method_name: "valueOf".into(),
            args: vec![IrExpr::LitInt(42)],
            ty: IrType::String,
        };
        let stmt = sysout_println(call);
        module
            .decls
            .push(IrDecl::Class(make_class("StrValueOf", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("JString::from") || code.contains("to_string"),
            "should convert to string"
        );
    }

    // ── Synchronized method ───────────────────────────────────────────────

    #[test]
    fn generate_synchronized_method() {
        let mut module = IrModule::new("");
        let mut cls = make_class("SyncClass", vec![]);
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
            name: "increment".into(),
            visibility: Visibility::Public,
            is_static: false,
            is_abstract: false,
            is_final: false,
            is_synchronized: true,
            type_params: vec![],
            params: vec![],
            return_ty: IrType::Void,
            body: Some(vec![IrStmt::Expr(IrExpr::CompoundAssign {
                op: IrBinOp::Add,
                lhs: Box::new(IrExpr::FieldAccess {
                    receiver: Box::new(IrExpr::Var {
                        name: "this".into(),
                        ty: IrType::Class("SyncClass".into()),
                    }),
                    field_name: "count".into(),
                    ty: IrType::Int,
                }),
                rhs: Box::new(IrExpr::LitInt(1)),
                ty: IrType::Int,
            })]),
            throws: vec![],
        });
        module.decls.push(IrDecl::Class(cls));
        let code = gen(&module);
        assert!(
            code.contains("Mutex") || code.contains("OnceLock"),
            "synchronized should use Mutex/OnceLock"
        );
    }

    // ── toString → Display impl ───────────────────────────────────────────

    #[test]
    fn generate_tostring_display() {
        let mut module = IrModule::new("");
        let mut cls = make_class("Point", vec![]);
        cls.methods = vec![IrMethod {
            name: "toString".into(),
            visibility: Visibility::Public,
            is_static: false,
            is_abstract: false,
            is_final: false,
            is_synchronized: false,
            type_params: vec![],
            params: vec![],
            return_ty: IrType::String,
            body: Some(vec![IrStmt::Return(Some(IrExpr::LitString(
                "Point".into(),
            )))]),
            throws: vec![],
        }];
        module.decls.push(IrDecl::Class(cls));
        let code = gen(&module);
        assert!(
            code.contains("impl") && code.contains("Display"),
            "toString should generate Display impl, got: {}",
            code
        );
    }

    // ── Volatile (atomic) field ───────────────────────────────────────────

    #[test]
    fn generate_volatile_field() {
        let mut module = IrModule::new("");
        let mut cls = make_class("AtomicField", vec![]);
        cls.fields.push(IrField {
            name: "counter".into(),
            ty: IrType::Atomic(Box::new(IrType::Int)),
            visibility: Visibility::Public,
            is_static: false,
            is_final: false,
            is_volatile: true,
            init: None,
        });
        module.decls.push(IrDecl::Class(cls));
        let code = gen(&module);
        assert!(
            code.contains("AtomicI32") || code.contains("Atomic"),
            "volatile int should use AtomicI32"
        );
    }

    // ── Collectors.toList ─────────────────────────────────────────────────

    #[test]
    fn generate_new_sets_and_maps() {
        let mut module = IrModule::new("");
        let stmts = vec![
            IrStmt::LocalVar {
                name: "s".into(),
                ty: IrType::Class("HashSet".into()),
                init: Some(IrExpr::New {
                    class: "HashSet".into(),
                    args: vec![],
                    ty: IrType::Class("HashSet".into()),
                }),
            },
            IrStmt::LocalVar {
                name: "t".into(),
                ty: IrType::Class("TreeMap".into()),
                init: Some(IrExpr::New {
                    class: "TreeMap".into(),
                    args: vec![],
                    ty: IrType::Class("TreeMap".into()),
                }),
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("Collections", stmts)));
        let code = gen(&module);
        assert!(code.contains("JSet::new()"), "HashSet should become JSet");
        assert!(
            code.contains("JTreeMap::new()"),
            "TreeMap should become JTreeMap"
        );
    }

    // ── Synchronized block ────────────────────────────────────────────────

    #[test]
    fn generate_synchronized_block() {
        let mut module = IrModule::new("");
        let sync = IrStmt::Synchronized {
            monitor: IrExpr::Var {
                name: "this".into(),
                ty: IrType::Class("Foo".into()),
            },
            body: vec![IrStmt::Expr(IrExpr::LitInt(1))],
        };
        module
            .decls
            .push(IrDecl::Class(make_class("SyncBlock", vec![sync])));
        let code = gen(&module);
        assert!(
            code.contains("__sync_block_monitor") || code.contains("lock"),
            "synchronized block should acquire lock"
        );
    }

    // ── Super constructor call ────────────────────────────────────────────

    #[test]
    fn generate_constructor_with_super() {
        let mut module = IrModule::new("");
        let parent_cls = IrClass {
            name: "Base".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            type_params: vec![],
            superclass: None,
            interfaces: vec![],
            fields: vec![IrField {
                name: "id".into(),
                ty: IrType::Int,
                visibility: Visibility::Public,
                is_static: false,
                is_final: false,
                is_volatile: false,
                init: None,
            }],
            methods: vec![],
            constructors: vec![IrConstructor {
                visibility: Visibility::Public,
                params: vec![IrParam {
                    name: "id".into(),
                    ty: IrType::Int,
                    is_varargs: false,
                }],
                body: vec![IrStmt::Expr(IrExpr::Assign {
                    lhs: Box::new(IrExpr::FieldAccess {
                        receiver: Box::new(IrExpr::Var {
                            name: "this".into(),
                            ty: IrType::Class("Base".into()),
                        }),
                        field_name: "id".into(),
                        ty: IrType::Int,
                    }),
                    rhs: Box::new(IrExpr::Var {
                        name: "id".into(),
                        ty: IrType::Int,
                    }),
                    ty: IrType::Int,
                })],
                throws: vec![],
            }],
        };
        let child_cls = IrClass {
            name: "Child".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            type_params: vec![],
            superclass: Some("Base".into()),
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
                body: Some(vec![]),
                throws: vec![],
            }],
            constructors: vec![IrConstructor {
                visibility: Visibility::Public,
                params: vec![IrParam {
                    name: "id".into(),
                    ty: IrType::Int,
                    is_varargs: false,
                }],
                body: vec![IrStmt::SuperConstructorCall {
                    args: vec![IrExpr::Var {
                        name: "id".into(),
                        ty: IrType::Int,
                    }],
                }],
                throws: vec![],
            }],
        };
        module.decls.push(IrDecl::Class(parent_cls));
        module.decls.push(IrDecl::Class(child_cls));
        let code = gen(&module);
        assert!(code.contains("Base::new"), "should call parent constructor");
    }

    // ── Multi-catch ───────────────────────────────────────────────────────

    #[test]
    fn generate_multi_catch() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::TryCatch {
            body: vec![IrStmt::Throw(IrExpr::New {
                class: "RuntimeException".into(),
                args: vec![IrExpr::LitString("err".into())],
                ty: IrType::Class("RuntimeException".into()),
            })],
            catches: vec![CatchClause {
                exception_types: vec!["IOException".into(), "Exception".into()],
                var: "e".into(),
                body: vec![],
            }],
            finally: None,
        };
        module
            .decls
            .push(IrDecl::Class(make_class("MultiCatch", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("catch_unwind"), "should use catch_unwind");
    }

    // ── System.err.println ────────────────────────────────────────────────

    #[test]
    fn generate_stderr_print() {
        let mut module = IrModule::new("");
        let print = IrStmt::Expr(IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::FieldAccess {
                receiver: Box::new(IrExpr::Var {
                    name: "System".into(),
                    ty: IrType::Class("System".into()),
                }),
                field_name: "err".into(),
                ty: IrType::Class("PrintStream".into()),
            })),
            method_name: "println".into(),
            args: vec![IrExpr::LitString("error".into())],
            ty: IrType::Void,
        });
        module
            .decls
            .push(IrDecl::Class(make_class("StdErr", vec![print])));
        let code = gen(&module);
        assert!(code.contains("eprintln!"), "should use eprintln!");
    }

    // ── Thread.sleep ──────────────────────────────────────────────────────

    #[test]
    fn generate_thread_sleep() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "Thread".into(),
                ty: IrType::Class("Thread".into()),
            })),
            method_name: "sleep".into(),
            args: vec![IrExpr::LitLong(100)],
            ty: IrType::Void,
        };
        let stmt = IrStmt::Expr(call);
        module
            .decls
            .push(IrDecl::Class(make_class("SleepTest", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("JThread::sleep") || code.contains("thread::sleep"),
            "should use JThread::sleep, got: {}",
            code
        );
    }

    // ── For loop with continue → labeled blocks ──────────────────────────

    #[test]
    fn generate_for_loop_with_continue() {
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
                ty: IrType::Bool,
            }),
            update: vec![IrExpr::UnOp {
                op: IrUnOp::PreInc,
                operand: Box::new(IrExpr::Var {
                    name: "i".into(),
                    ty: IrType::Int,
                }),
                ty: IrType::Int,
            }],
            body: vec![IrStmt::Continue(None)],
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("ForContinue", stmts)));
        let code = gen(&module);
        assert!(
            code.contains("for_body") || code.contains("for_loop") || code.contains("loop"),
            "should use labeled loops for continue"
        );
    }

    // ── String method receiver dispatch ──────────────────────────────────

    #[test]
    fn generate_string_length() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "s".into(),
                ty: IrType::String,
            })),
            method_name: "length".into(),
            args: vec![],
            ty: IrType::Int,
        };
        let stmt = sysout_println(call);
        module
            .decls
            .push(IrDecl::Class(make_class("StrLen", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("length()"), "should emit length() call");
    }

    #[test]
    fn generate_string_substring() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "s".into(),
                ty: IrType::String,
            })),
            method_name: "substring".into(),
            args: vec![IrExpr::LitInt(0), IrExpr::LitInt(3)],
            ty: IrType::String,
        };
        let stmt = sysout_println(call);
        module
            .decls
            .push(IrDecl::Class(make_class("StrSub", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("substring") || code.contains("slice"),
            "should map substring"
        );
    }

    #[test]
    fn generate_string_contains() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "s".into(),
                ty: IrType::String,
            })),
            method_name: "contains".into(),
            args: vec![IrExpr::LitString("x".into())],
            ty: IrType::Bool,
        };
        let stmt = sysout_println(call);
        module
            .decls
            .push(IrDecl::Class(make_class("StrContains", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("contains"), "should map contains");
    }

    #[test]
    fn generate_string_trim_tolower() {
        let mut module = IrModule::new("");
        let stmts = vec![
            sysout_println(IrExpr::MethodCall {
                receiver: Some(Box::new(IrExpr::Var {
                    name: "s".into(),
                    ty: IrType::String,
                })),
                method_name: "trim".into(),
                args: vec![],
                ty: IrType::String,
            }),
            sysout_println(IrExpr::MethodCall {
                receiver: Some(Box::new(IrExpr::Var {
                    name: "s".into(),
                    ty: IrType::String,
                })),
                method_name: "toLowerCase".into(),
                args: vec![],
                ty: IrType::String,
            }),
            sysout_println(IrExpr::MethodCall {
                receiver: Some(Box::new(IrExpr::Var {
                    name: "s".into(),
                    ty: IrType::String,
                })),
                method_name: "toUpperCase".into(),
                args: vec![],
                ty: IrType::String,
            }),
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("StrOps", stmts)));
        let code = gen(&module);
        assert!(code.contains("trim"), "should have trim");
        assert!(
            code.contains("to_lowercase") || code.contains("toLowerCase"),
            "should map toLowerCase"
        );
    }

    #[test]
    fn generate_string_startswith_endswith() {
        let mut module = IrModule::new("");
        let stmts = vec![
            sysout_println(IrExpr::MethodCall {
                receiver: Some(Box::new(IrExpr::Var {
                    name: "s".into(),
                    ty: IrType::String,
                })),
                method_name: "startsWith".into(),
                args: vec![IrExpr::LitString("x".into())],
                ty: IrType::Bool,
            }),
            sysout_println(IrExpr::MethodCall {
                receiver: Some(Box::new(IrExpr::Var {
                    name: "s".into(),
                    ty: IrType::String,
                })),
                method_name: "endsWith".into(),
                args: vec![IrExpr::LitString("y".into())],
                ty: IrType::Bool,
            }),
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("StrCheck", stmts)));
        let code = gen(&module);
        assert!(code.contains("startsWith"), "should emit startsWith call");
        assert!(code.contains("endsWith"), "should emit endsWith call");
    }

    #[test]
    fn generate_string_replace() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "s".into(),
                ty: IrType::String,
            })),
            method_name: "replace".into(),
            args: vec![IrExpr::LitString("a".into()), IrExpr::LitString("b".into())],
            ty: IrType::String,
        };
        let stmt = sysout_println(call);
        module
            .decls
            .push(IrDecl::Class(make_class("StrReplace", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("replace"), "should map replace");
    }

    #[test]
    fn generate_string_indexof() {
        let mut module = IrModule::new("");
        let stmts = vec![
            sysout_println(IrExpr::MethodCall {
                receiver: Some(Box::new(IrExpr::Var {
                    name: "s".into(),
                    ty: IrType::String,
                })),
                method_name: "indexOf".into(),
                args: vec![IrExpr::LitString("x".into())],
                ty: IrType::Int,
            }),
            sysout_println(IrExpr::MethodCall {
                receiver: Some(Box::new(IrExpr::Var {
                    name: "s".into(),
                    ty: IrType::String,
                })),
                method_name: "isEmpty".into(),
                args: vec![],
                ty: IrType::Bool,
            }),
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("StrIdx", stmts)));
        let code = gen(&module);
        assert!(
            code.contains("indexOf") || code.contains("index_of"),
            "should map indexOf"
        );
    }

    // ── New constructors for runtime types ────────────────────────────────

    #[test]
    fn generate_new_atomic_long() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "al".into(),
            ty: IrType::Class("AtomicLong".into()),
            init: Some(IrExpr::New {
                class: "AtomicLong".into(),
                args: vec![IrExpr::LitLong(0)],
                ty: IrType::Class("AtomicLong".into()),
            }),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("NewAtomicLong", stmts)));
        let code = gen(&module);
        assert!(
            code.contains("JAtomicLong::new"),
            "should use JAtomicLong::new"
        );
    }

    #[test]
    fn generate_new_atomic_boolean() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "ab".into(),
            ty: IrType::Class("AtomicBoolean".into()),
            init: Some(IrExpr::New {
                class: "AtomicBoolean".into(),
                args: vec![IrExpr::LitBool(false)],
                ty: IrType::Class("AtomicBoolean".into()),
            }),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("NewAtomBool", stmts)));
        let code = gen(&module);
        assert!(
            code.contains("JAtomicBoolean::new"),
            "should use JAtomicBoolean::new"
        );
    }

    #[test]
    fn generate_new_countdown_latch() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "l".into(),
            ty: IrType::Class("CountDownLatch".into()),
            init: Some(IrExpr::New {
                class: "CountDownLatch".into(),
                args: vec![IrExpr::LitInt(3)],
                ty: IrType::Class("CountDownLatch".into()),
            }),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("NewLatch", stmts)));
        let code = gen(&module);
        assert!(
            code.contains("JCountDownLatch::new"),
            "should use JCountDownLatch::new"
        );
    }

    #[test]
    fn generate_new_semaphore() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "s".into(),
            ty: IrType::Class("Semaphore".into()),
            init: Some(IrExpr::New {
                class: "Semaphore".into(),
                args: vec![IrExpr::LitInt(1)],
                ty: IrType::Class("Semaphore".into()),
            }),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("NewSemaphore", stmts)));
        let code = gen(&module);
        assert!(
            code.contains("JSemaphore::new"),
            "should use JSemaphore::new"
        );
    }

    #[test]
    fn generate_new_stringbuilder_with_arg() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "sb".into(),
            ty: IrType::Class("StringBuilder".into()),
            init: Some(IrExpr::New {
                class: "StringBuilder".into(),
                args: vec![IrExpr::LitString("hello".into())],
                ty: IrType::Class("StringBuilder".into()),
            }),
        }];
        module.decls.push(IrDecl::Class(make_class("NewSB", stmts)));
        let code = gen(&module);
        assert!(
            code.contains("JStringBuilder::new_from_string"),
            "should use JStringBuilder::new_from_string"
        );
    }

    #[test]
    fn generate_new_biginteger() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "bi".into(),
            ty: IrType::Class("BigInteger".into()),
            init: Some(IrExpr::New {
                class: "BigInteger".into(),
                args: vec![IrExpr::LitString("12345".into())],
                ty: IrType::Class("BigInteger".into()),
            }),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("NewBigInt", stmts)));
        let code = gen(&module);
        assert!(
            code.contains("JBigInteger::from_string"),
            "should use JBigInteger::from_string"
        );
    }

    #[test]
    fn generate_new_file() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "f".into(),
            ty: IrType::Class("File".into()),
            init: Some(IrExpr::New {
                class: "File".into(),
                args: vec![IrExpr::LitString("test.txt".into())],
                ty: IrType::Class("File".into()),
            }),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("NewFile", stmts)));
        let code = gen(&module);
        assert!(code.contains("JFile::new"), "should use JFile::new");
    }

    #[test]
    fn generate_new_file_two_args() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "f".into(),
            ty: IrType::Class("File".into()),
            init: Some(IrExpr::New {
                class: "File".into(),
                args: vec![
                    IrExpr::LitString("/tmp".into()),
                    IrExpr::LitString("test.txt".into()),
                ],
                ty: IrType::Class("File".into()),
            }),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("NewFile2", stmts)));
        let code = gen(&module);
        assert!(
            code.contains("JFile::new_child"),
            "should use JFile::new_child"
        );
    }

    #[test]
    fn generate_new_thread_with_runnable() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::LocalVar {
            name: "t".into(),
            ty: IrType::Class("Thread".into()),
            init: Some(IrExpr::New {
                class: "Thread".into(),
                args: vec![IrExpr::Var {
                    name: "task".into(),
                    ty: IrType::Class("Runnable".into()),
                }],
                ty: IrType::Class("Thread".into()),
            }),
        }];
        module
            .decls
            .push(IrDecl::Class(make_class("NewThread", stmts)));
        let code = gen(&module);
        assert!(code.contains("JThread::new"), "should use JThread::new");
    }

    // ── Assign volatile field (Atomic store) ──────────────────────────────

    #[test]
    fn generate_volatile_field_write() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::Expr(IrExpr::Assign {
            lhs: Box::new(IrExpr::FieldAccess {
                receiver: Box::new(IrExpr::Var {
                    name: "this".into(),
                    ty: IrType::Class("Ctr".into()),
                }),
                field_name: "val".into(),
                ty: IrType::Atomic(Box::new(IrType::Int)),
            }),
            rhs: Box::new(IrExpr::LitInt(42)),
            ty: IrType::Int,
        })];
        module
            .decls
            .push(IrDecl::Class(make_class("VolWrite", stmts)));
        let code = gen(&module);
        assert!(
            code.contains("store") && code.contains("SeqCst"),
            "volatile write should use .store with SeqCst"
        );
    }

    // ── print / printf variants ───────────────────────────────────────────

    #[test]
    fn generate_system_out_print_no_ln() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::Expr(IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::FieldAccess {
                receiver: Box::new(IrExpr::Var {
                    name: "System".into(),
                    ty: IrType::Class("System".into()),
                }),
                field_name: "out".into(),
                ty: IrType::Class("PrintStream".into()),
            })),
            method_name: "print".into(),
            args: vec![IrExpr::LitString("hello".into())],
            ty: IrType::Void,
        });
        module
            .decls
            .push(IrDecl::Class(make_class("PrintNoLn", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("print!"), "should use print! macro");
    }

    #[test]
    fn generate_system_out_printf() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::Expr(IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::FieldAccess {
                receiver: Box::new(IrExpr::Var {
                    name: "System".into(),
                    ty: IrType::Class("System".into()),
                }),
                field_name: "out".into(),
                ty: IrType::Class("PrintStream".into()),
            })),
            method_name: "printf".into(),
            args: vec![
                IrExpr::LitString("hello %s".into()),
                IrExpr::LitString("world".into()),
            ],
            ty: IrType::Void,
        });
        module
            .decls
            .push(IrDecl::Class(make_class("Printf", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("print!"), "printf should use print! macro");
    }

    // ── Long.parseLong / Double.parseDouble ───────────────────────────────

    #[test]
    fn generate_long_parselong() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "Long".into(),
                ty: IrType::Class("Long".into()),
            })),
            method_name: "parseLong".into(),
            args: vec![IrExpr::LitString("42".into())],
            ty: IrType::Long,
        };
        let stmt = sysout_println(call);
        module
            .decls
            .push(IrDecl::Class(make_class("ParseLong", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("parseLong") || code.contains("parse::") || code.contains("i64"),
            "should emit parseLong call"
        );
    }

    #[test]
    fn generate_double_parsedouble() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "Double".into(),
                ty: IrType::Class("Double".into()),
            })),
            method_name: "parseDouble".into(),
            args: vec![IrExpr::LitString("3.14".into())],
            ty: IrType::Double,
        };
        let stmt = sysout_println(call);
        module
            .decls
            .push(IrDecl::Class(make_class("ParseDouble", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("parseDouble") || code.contains("parse::") || code.contains("f64"),
            "should emit parseDouble call"
        );
    }

    // ── Stream API ────────────────────────────────────────────────────────

    #[test]
    fn generate_stream_filter_map() {
        let mut module = IrModule::new("");
        let stmts = vec![sysout_println(IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "list".into(),
                ty: IrType::Class("JList".into()),
            })),
            method_name: "stream".into(),
            args: vec![],
            ty: IrType::Class("JStream".into()),
        })];
        module
            .decls
            .push(IrDecl::Class(make_class("StreamTest", stmts)));
        let code = gen(&module);
        assert!(code.contains("stream"), "should use stream");
    }

    // ── List methods ──────────────────────────────────────────────────────

    #[test]
    fn generate_list_add_get_size() {
        let mut module = IrModule::new("");
        let list_var = Box::new(IrExpr::Var {
            name: "list".into(),
            ty: IrType::Class("ArrayList".into()),
        });
        let stmts = vec![
            IrStmt::Expr(IrExpr::MethodCall {
                receiver: Some(list_var.clone()),
                method_name: "add".into(),
                args: vec![IrExpr::LitInt(1)],
                ty: IrType::Bool,
            }),
            sysout_println(IrExpr::MethodCall {
                receiver: Some(list_var.clone()),
                method_name: "get".into(),
                args: vec![IrExpr::LitInt(0)],
                ty: IrType::Unknown,
            }),
            sysout_println(IrExpr::MethodCall {
                receiver: Some(list_var.clone()),
                method_name: "size".into(),
                args: vec![],
                ty: IrType::Int,
            }),
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("ListOps", stmts)));
        let code = gen(&module);
        assert!(
            code.contains("add") && code.contains("get") && code.contains("size"),
            "should have list methods"
        );
    }

    // ── Array assign ──────────────────────────────────────────────────────

    #[test]
    fn generate_array_assign() {
        let mut module = IrModule::new("");
        let stmts = vec![IrStmt::Expr(IrExpr::Assign {
            lhs: Box::new(IrExpr::ArrayAccess {
                array: Box::new(IrExpr::Var {
                    name: "arr".into(),
                    ty: IrType::Array(Box::new(IrType::Int)),
                }),
                index: Box::new(IrExpr::LitInt(0)),
                ty: IrType::Int,
            }),
            rhs: Box::new(IrExpr::LitInt(42)),
            ty: IrType::Int,
        })];
        module
            .decls
            .push(IrDecl::Class(make_class("ArrSet", stmts)));
        let code = gen(&module);
        assert!(code.contains(".set("), "array assignment should use .set()");
    }

    // ── println with double (uses {:?} format) ───────────────────────────

    #[test]
    fn generate_println_double_format() {
        let mut module = IrModule::new("");
        let stmt = sysout_println(IrExpr::Var {
            name: "x".into(),
            ty: IrType::Double,
        });
        module
            .decls
            .push(IrDecl::Class(make_class("PrintDouble", vec![stmt])));
        let code = gen(&module);
        // Double arguments might use {:?} for float formatting
        assert!(code.contains("println!"), "should use println!");
    }

    // ── Generic type emission ─────────────────────────────────────────────

    #[test]
    fn generate_generic_list_type() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::LocalVar {
            name: "xs".into(),
            ty: IrType::Generic {
                base: Box::new(IrType::Class("ArrayList".into())),
                args: vec![IrType::Int],
            },
            init: Some(IrExpr::New {
                class: "ArrayList".into(),
                args: vec![],
                ty: IrType::Generic {
                    base: Box::new(IrType::Class("ArrayList".into())),
                    args: vec![IrType::Int],
                },
            }),
        };
        module
            .decls
            .push(IrDecl::Class(make_class("GenList", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("JList"),
            "should map ArrayList<T> to JList<T>"
        );
    }

    #[test]
    fn generate_generic_map_type() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::LocalVar {
            name: "m".into(),
            ty: IrType::Generic {
                base: Box::new(IrType::Class("HashMap".into())),
                args: vec![IrType::String, IrType::Int],
            },
            init: None,
        };
        module
            .decls
            .push(IrDecl::Class(make_class("GenMap", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("JMap"),
            "should map HashMap<K,V> to JMap<K,V>"
        );
    }

    #[test]
    fn generate_generic_set_type() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::LocalVar {
            name: "s".into(),
            ty: IrType::Generic {
                base: Box::new(IrType::Class("HashSet".into())),
                args: vec![IrType::String],
            },
            init: None,
        };
        module
            .decls
            .push(IrDecl::Class(make_class("GenSet", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("JSet"), "should map HashSet<T> to JSet<T>");
    }

    #[test]
    fn generate_generic_optional_type() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::LocalVar {
            name: "o".into(),
            ty: IrType::Generic {
                base: Box::new(IrType::Class("Optional".into())),
                args: vec![IrType::String],
            },
            init: None,
        };
        module
            .decls
            .push(IrDecl::Class(make_class("GenOpt", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("JOptional"),
            "should map Optional<T> to JOptional<T>"
        );
    }

    #[test]
    fn generate_generic_stream_type() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::LocalVar {
            name: "st".into(),
            ty: IrType::Generic {
                base: Box::new(IrType::Class("Stream".into())),
                args: vec![IrType::Int],
            },
            init: None,
        };
        module
            .decls
            .push(IrDecl::Class(make_class("GenStream", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("JStream"),
            "should map Stream<T> to JStream<T>"
        );
    }

    // ── Atomic type emission ──────────────────────────────────────────────

    #[test]
    fn generate_atomic_int_type() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::LocalVar {
            name: "ai".into(),
            ty: IrType::Atomic(Box::new(IrType::Int)),
            init: None,
        };
        module
            .decls
            .push(IrDecl::Class(make_class("AtomicIntTy", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("AtomicI32"),
            "should emit AtomicI32 for Atomic<Int>"
        );
    }

    #[test]
    fn generate_atomic_long_type() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::LocalVar {
            name: "al".into(),
            ty: IrType::Atomic(Box::new(IrType::Long)),
            init: None,
        };
        module
            .decls
            .push(IrDecl::Class(make_class("AtomicLngTy", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("AtomicI64"),
            "should emit AtomicI64 for Atomic<Long>"
        );
    }

    #[test]
    fn generate_atomic_bool_type() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::LocalVar {
            name: "ab".into(),
            ty: IrType::Atomic(Box::new(IrType::Bool)),
            init: None,
        };
        module
            .decls
            .push(IrDecl::Class(make_class("AtomicBoolTy", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("AtomicBool"),
            "should emit AtomicBool for Atomic<Bool>"
        );
    }

    // ── Keyword sanitization ──────────────────────────────────────────────

    #[test]
    fn generate_keyword_sanitization() {
        let mut module = IrModule::new("");
        // Use Rust keywords as variable names — they should be sanitized
        let stmts = vec![
            IrStmt::LocalVar {
                name: "type".into(),
                ty: IrType::Int,
                init: Some(IrExpr::LitInt(1)),
            },
            IrStmt::LocalVar {
                name: "match".into(),
                ty: IrType::Int,
                init: Some(IrExpr::LitInt(2)),
            },
            IrStmt::LocalVar {
                name: "loop".into(),
                ty: IrType::Int,
                init: Some(IrExpr::LitInt(3)),
            },
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("Keywords", stmts)));
        let code = gen(&module);
        assert!(code.contains("type_"), "should sanitize 'type' keyword");
        assert!(code.contains("match_"), "should sanitize 'match' keyword");
        assert!(code.contains("loop_"), "should sanitize 'loop' keyword");
    }

    // ── Cast expressions ──────────────────────────────────────────────────

    #[test]
    fn generate_cast_expression() {
        let mut module = IrModule::new("");
        let cast = IrExpr::Cast {
            target: IrType::Long,
            expr: Box::new(IrExpr::Var {
                name: "x".into(),
                ty: IrType::Int,
            }),
        };
        let stmt = sysout_println(cast);
        module
            .decls
            .push(IrDecl::Class(make_class("CastExpr", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("as i64"), "should emit cast as i64");
    }

    // ── InstanceOf ────────────────────────────────────────────────────────

    #[test]
    fn generate_instanceof_expr() {
        let mut module = IrModule::new("");
        let expr = IrExpr::InstanceOf {
            expr: Box::new(IrExpr::Var {
                name: "obj".into(),
                ty: IrType::Class("Object".into()),
            }),
            check_type: IrType::Class("String".into()),
        };
        let stmt = sysout_println(expr);
        module
            .decls
            .push(IrDecl::Class(make_class("InstOf", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("_instanceof"), "should emit _instanceof call");
    }

    // ── Lambda expression ─────────────────────────────────────────────────

    #[test]
    fn generate_lambda_expression() {
        let mut module = IrModule::new("");
        let lambda = IrExpr::Lambda {
            params: vec!["x".into()],
            body: Box::new(IrExpr::BinOp {
                op: IrBinOp::Mul,
                lhs: Box::new(IrExpr::Var {
                    name: "x".into(),
                    ty: IrType::Int,
                }),
                rhs: Box::new(IrExpr::LitInt(2)),
                ty: IrType::Int,
            }),
            body_stmts: vec![],
            ty: IrType::Unknown,
        };
        let stmt = IrStmt::LocalVar {
            name: "fn_".into(),
            ty: IrType::Unknown,
            init: Some(lambda),
        };
        module
            .decls
            .push(IrDecl::Class(make_class("Lambda", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("|x|") || code.contains("| x"),
            "should emit lambda closure"
        );
    }

    // ── CompoundAssign (bitwise) ──────────────────────────────────────────

    #[test]
    fn generate_compound_assign_bitwise() {
        let mut module = IrModule::new("");
        let stmts = vec![
            IrStmt::Expr(IrExpr::CompoundAssign {
                op: IrBinOp::BitAnd,
                lhs: Box::new(IrExpr::Var {
                    name: "x".into(),
                    ty: IrType::Int,
                }),
                rhs: Box::new(IrExpr::LitInt(0xFF)),
                ty: IrType::Int,
            }),
            IrStmt::Expr(IrExpr::CompoundAssign {
                op: IrBinOp::BitOr,
                lhs: Box::new(IrExpr::Var {
                    name: "x".into(),
                    ty: IrType::Int,
                }),
                rhs: Box::new(IrExpr::LitInt(1)),
                ty: IrType::Int,
            }),
            IrStmt::Expr(IrExpr::CompoundAssign {
                op: IrBinOp::BitXor,
                lhs: Box::new(IrExpr::Var {
                    name: "x".into(),
                    ty: IrType::Int,
                }),
                rhs: Box::new(IrExpr::LitInt(0x0F)),
                ty: IrType::Int,
            }),
            IrStmt::Expr(IrExpr::CompoundAssign {
                op: IrBinOp::Shl,
                lhs: Box::new(IrExpr::Var {
                    name: "x".into(),
                    ty: IrType::Int,
                }),
                rhs: Box::new(IrExpr::LitInt(2)),
                ty: IrType::Int,
            }),
            IrStmt::Expr(IrExpr::CompoundAssign {
                op: IrBinOp::Shr,
                lhs: Box::new(IrExpr::Var {
                    name: "x".into(),
                    ty: IrType::Int,
                }),
                rhs: Box::new(IrExpr::LitInt(1)),
                ty: IrType::Int,
            }),
        ];
        module
            .decls
            .push(IrDecl::Class(make_class("CompBit", stmts)));
        let code = gen(&module);
        assert!(code.contains("&="), "should emit &=");
        assert!(code.contains("|="), "should emit |=");
        assert!(code.contains("^="), "should emit ^=");
        assert!(code.contains("<<="), "should emit <<=");
        assert!(code.contains(">>="), "should emit >>=");
    }

    // ── Math methods with long args ───────────────────────────────────────

    #[test]
    fn generate_math_abs_long() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "Math".into(),
                ty: IrType::Class("Math".into()),
            })),
            method_name: "abs".into(),
            args: vec![IrExpr::Var {
                name: "n".into(),
                ty: IrType::Long,
            }],
            ty: IrType::Long,
        };
        let stmt = sysout_println(call);
        module
            .decls
            .push(IrDecl::Class(make_class("MathAbsLong", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("i64"), "should cast to i64 for long abs");
    }

    #[test]
    fn generate_math_max_long() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "Math".into(),
                ty: IrType::Class("Math".into()),
            })),
            method_name: "max".into(),
            args: vec![
                IrExpr::Var {
                    name: "a".into(),
                    ty: IrType::Long,
                },
                IrExpr::Var {
                    name: "b".into(),
                    ty: IrType::Long,
                },
            ],
            ty: IrType::Long,
        };
        let stmt = sysout_println(call);
        module
            .decls
            .push(IrDecl::Class(make_class("MathMaxLong", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("i64"), "should cast to i64 for long max");
    }

    #[test]
    fn generate_math_min_double() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "Math".into(),
                ty: IrType::Class("Math".into()),
            })),
            method_name: "min".into(),
            args: vec![
                IrExpr::Var {
                    name: "a".into(),
                    ty: IrType::Double,
                },
                IrExpr::Var {
                    name: "b".into(),
                    ty: IrType::Double,
                },
            ],
            ty: IrType::Double,
        };
        let stmt = sysout_println(call);
        module
            .decls
            .push(IrDecl::Class(make_class("MathMinDbl", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("f64"), "should cast to f64 for double min");
    }

    // ── Math log/log10/exp/trig ───────────────────────────────────────────

    #[test]
    fn generate_math_log_exp() {
        let mut module = IrModule::new("");
        let methods = ["log", "log10", "exp"];
        let mut stmts = vec![];
        for m in &methods {
            stmts.push(sysout_println(IrExpr::MethodCall {
                receiver: Some(Box::new(IrExpr::Var {
                    name: "Math".into(),
                    ty: IrType::Class("Math".into()),
                })),
                method_name: (*m).into(),
                args: vec![IrExpr::Var {
                    name: "x".into(),
                    ty: IrType::Double,
                }],
                ty: IrType::Double,
            }));
        }
        module
            .decls
            .push(IrDecl::Class(make_class("MathLog", stmts)));
        let code = gen(&module);
        assert!(code.contains("ln()"), "should map log to ln()");
        assert!(code.contains("log10()"), "should emit log10()");
        assert!(code.contains("exp()"), "should emit exp()");
    }

    // ── Optional static methods ───────────────────────────────────────────

    #[test]
    fn generate_optional_empty() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "Optional".into(),
                ty: IrType::Class("Optional".into()),
            })),
            method_name: "empty".into(),
            args: vec![],
            ty: IrType::Class("Optional".into()),
        };
        let stmt = IrStmt::LocalVar {
            name: "o".into(),
            ty: IrType::Unknown,
            init: Some(call),
        };
        module
            .decls
            .push(IrDecl::Class(make_class("OptEmpty", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("JOptional::empty"),
            "should emit JOptional::empty()"
        );
    }

    #[test]
    fn generate_optional_of_nullable() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "Optional".into(),
                ty: IrType::Class("Optional".into()),
            })),
            method_name: "ofNullable".into(),
            args: vec![IrExpr::LitString("hello".into())],
            ty: IrType::Class("Optional".into()),
        };
        let stmt = IrStmt::LocalVar {
            name: "o".into(),
            ty: IrType::Unknown,
            init: Some(call),
        };
        module
            .decls
            .push(IrDecl::Class(make_class("OptNullable", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("JOptional::of_nullable"),
            "should emit JOptional::of_nullable()"
        );
    }

    // ── Pattern static methods ────────────────────────────────────────────

    #[test]
    fn generate_pattern_compile() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "Pattern".into(),
                ty: IrType::Class("Pattern".into()),
            })),
            method_name: "compile".into(),
            args: vec![IrExpr::LitString("\\d+".into())],
            ty: IrType::Class("Pattern".into()),
        };
        let stmt = IrStmt::LocalVar {
            name: "p".into(),
            ty: IrType::Unknown,
            init: Some(call),
        };
        module
            .decls
            .push(IrDecl::Class(make_class("PatCompile", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("JPattern::compile"),
            "should emit JPattern::compile()"
        );
    }

    #[test]
    fn generate_pattern_matches() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "Pattern".into(),
                ty: IrType::Class("Pattern".into()),
            })),
            method_name: "matches".into(),
            args: vec![
                IrExpr::LitString("\\d+".into()),
                IrExpr::LitString("123".into()),
            ],
            ty: IrType::Bool,
        };
        let stmt = sysout_println(call);
        module
            .decls
            .push(IrDecl::Class(make_class("PatMatch", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("JPattern::static_matches"),
            "should emit JPattern::static_matches()"
        );
    }

    // ── LocalDate static methods ──────────────────────────────────────────

    #[test]
    fn generate_localdate_of() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "LocalDate".into(),
                ty: IrType::Class("LocalDate".into()),
            })),
            method_name: "of".into(),
            args: vec![IrExpr::LitInt(2024), IrExpr::LitInt(1), IrExpr::LitInt(15)],
            ty: IrType::Class("LocalDate".into()),
        };
        let stmt = IrStmt::LocalVar {
            name: "d".into(),
            ty: IrType::Unknown,
            init: Some(call),
        };
        module
            .decls
            .push(IrDecl::Class(make_class("LdOf", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("JLocalDate::of"),
            "should emit JLocalDate::of()"
        );
    }

    #[test]
    fn generate_localdate_now() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "LocalDate".into(),
                ty: IrType::Class("LocalDate".into()),
            })),
            method_name: "now".into(),
            args: vec![],
            ty: IrType::Class("LocalDate".into()),
        };
        let stmt = IrStmt::LocalVar {
            name: "d".into(),
            ty: IrType::Unknown,
            init: Some(call),
        };
        module
            .decls
            .push(IrDecl::Class(make_class("LdNow", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("JLocalDate::now"),
            "should emit JLocalDate::now()"
        );
    }

    // ── field_default_val for Atomic types ────────────────────────────────

    #[test]
    fn generate_atomic_field_defaults() {
        let mut module = IrModule::new("");
        let cls = IrClass {
            name: "AtomDefaults".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            superclass: None,
            interfaces: vec![],
            type_params: vec![],
            fields: vec![
                ir::decl::IrField {
                    name: "ai".into(),
                    ty: IrType::Atomic(Box::new(IrType::Int)),
                    visibility: Visibility::Public,
                    is_static: false,
                    is_volatile: false,
                    is_final: false,
                    init: None,
                },
                ir::decl::IrField {
                    name: "al".into(),
                    ty: IrType::Atomic(Box::new(IrType::Long)),
                    visibility: Visibility::Public,
                    is_static: false,
                    is_volatile: false,
                    is_final: false,
                    init: None,
                },
                ir::decl::IrField {
                    name: "ab".into(),
                    ty: IrType::Atomic(Box::new(IrType::Bool)),
                    visibility: Visibility::Public,
                    is_static: false,
                    is_volatile: false,
                    is_final: false,
                    init: None,
                },
            ],
            methods: vec![],
            constructors: vec![ir::decl::IrConstructor {
                visibility: Visibility::Public,
                params: vec![],
                body: vec![],
                throws: vec![],
            }],
        };
        module.decls.push(IrDecl::Class(cls));
        let code = gen(&module);
        assert!(
            code.contains("AtomicI32::new(0)"),
            "should default AtomicI32 to 0"
        );
        assert!(
            code.contains("AtomicI64::new(0)"),
            "should default AtomicI64 to 0"
        );
        assert!(
            code.contains("AtomicBool::new(false)"),
            "should default AtomicBool to false"
        );
    }

    // ── Delegation stubs for inherited methods ────────────────────────────

    #[test]
    fn generate_delegation_stub() {
        let mut module = IrModule::new("");
        let parent = IrClass {
            name: "Base".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            superclass: None,
            interfaces: vec![],
            type_params: vec![],
            fields: vec![],
            methods: vec![ir::decl::IrMethod {
                name: "greet".into(),
                visibility: Visibility::Public,
                is_static: false,
                is_abstract: false,
                is_final: false,
                is_synchronized: false,
                type_params: vec![],
                params: vec![],
                return_ty: IrType::String,
                body: Some(vec![IrStmt::Return(Some(IrExpr::LitString("hi".into())))]),
                throws: vec![],
            }],
            constructors: vec![],
        };
        let child = IrClass {
            name: "Child".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            superclass: Some("Base".into()),
            interfaces: vec![],
            type_params: vec![],
            fields: vec![],
            methods: vec![],
            constructors: vec![ir::decl::IrConstructor {
                visibility: Visibility::Public,
                params: vec![],
                body: vec![],
                throws: vec![],
            }],
        };
        module.decls.push(IrDecl::Class(parent));
        module.decls.push(IrDecl::Class(child));
        let code = gen(&module);
        assert!(
            code.contains("self._super.greet"),
            "should delegate to _super"
        );
    }

    // ── Nullable type emission ────────────────────────────────────────────

    #[test]
    fn generate_nullable_type() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::LocalVar {
            name: "x".into(),
            ty: IrType::Nullable(Box::new(IrType::String)),
            init: None,
        };
        module
            .decls
            .push(IrDecl::Class(make_class("NullableType", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("Option"),
            "should emit Option<T> for Nullable"
        );
    }

    // ── TypeVar emission ──────────────────────────────────────────────────

    #[test]
    fn generate_type_var() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::LocalVar {
            name: "t".into(),
            ty: IrType::TypeVar("T".into()),
            init: None,
        };
        module
            .decls
            .push(IrDecl::Class(make_class("TypeVarEmit", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("T"), "should emit type variable T");
    }

    // ── DoWhile loop ──────────────────────────────────────────────────────

    #[test]
    fn generate_do_while_loop() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::DoWhile {
            body: vec![IrStmt::Expr(IrExpr::UnOp {
                op: IrUnOp::PostInc,
                operand: Box::new(IrExpr::Var {
                    name: "i".into(),
                    ty: IrType::Int,
                }),
                ty: IrType::Int,
            })],
            cond: IrExpr::BinOp {
                op: IrBinOp::Lt,
                lhs: Box::new(IrExpr::Var {
                    name: "i".into(),
                    ty: IrType::Int,
                }),
                rhs: Box::new(IrExpr::LitInt(10)),
                ty: IrType::Bool,
            },
        };
        module
            .decls
            .push(IrDecl::Class(make_class("DoWhile", vec![stmt])));
        let code = gen(&module);
        assert!(code.contains("loop"), "should emit loop for do-while");
    }

    // ── Collect Collectors.toList() ───────────────────────────────────────

    #[test]
    fn generate_collect_to_list() {
        let mut module = IrModule::new("");
        let collectors_call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "Collectors".into(),
                ty: IrType::Class("Collectors".into()),
            })),
            method_name: "toList".into(),
            args: vec![],
            ty: IrType::Unknown,
        };
        let collect = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "stream".into(),
                ty: IrType::Class("JStream".into()),
            })),
            method_name: "collect".into(),
            args: vec![collectors_call],
            ty: IrType::Unknown,
        };
        let stmt = IrStmt::LocalVar {
            name: "result".into(),
            ty: IrType::Unknown,
            init: Some(collect),
        };
        module
            .decls
            .push(IrDecl::Class(make_class("CollectList", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("collect_to_list"),
            "should map collect(Collectors.toList()) to collect_to_list()"
        );
    }

    // ── String.valueOf ────────────────────────────────────────────────────

    #[test]
    fn generate_string_valueof_int() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "String".into(),
                ty: IrType::Class("String".into()),
            })),
            method_name: "valueOf".into(),
            args: vec![IrExpr::LitInt(42)],
            ty: IrType::String,
        };
        let stmt = sysout_println(call);
        module
            .decls
            .push(IrDecl::Class(make_class("StrValueOf", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("JString::from") || code.contains("format!"),
            "should convert to JString"
        );
    }

    // ── Integer.toString ──────────────────────────────────────────────────

    #[test]
    fn generate_integer_tostring_static() {
        let mut module = IrModule::new("");
        let call = IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::Var {
                name: "Integer".into(),
                ty: IrType::Class("Integer".into()),
            })),
            method_name: "toString".into(),
            args: vec![IrExpr::LitInt(42)],
            ty: IrType::String,
        };
        let stmt = sysout_println(call);
        module
            .decls
            .push(IrDecl::Class(make_class("IntToStr", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("JString::from"),
            "should emit JString::from for Integer.toString"
        );
    }

    // ── FieldAccess with volatile read ────────────────────────────────────

    #[test]
    fn generate_volatile_field_read() {
        let mut module = IrModule::new("");
        let cls = IrClass {
            name: "VolRead".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            superclass: None,
            interfaces: vec![],
            type_params: vec![],
            fields: vec![ir::decl::IrField {
                name: "count".into(),
                ty: IrType::Atomic(Box::new(IrType::Int)),
                visibility: Visibility::Public,
                is_static: false,
                is_volatile: true,
                is_final: false,
                init: None,
            }],
            methods: vec![ir::decl::IrMethod {
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
                        name: "self".into(),
                        ty: IrType::Class("VolRead".into()),
                    }),
                    field_name: "count".into(),
                    ty: IrType::Atomic(Box::new(IrType::Int)),
                }))]),
                throws: vec![],
            }],
            constructors: vec![],
        };
        module.decls.push(IrDecl::Class(cls));
        let code = gen(&module);
        assert!(
            code.contains("load") && code.contains("SeqCst"),
            "should emit atomic load for volatile read"
        );
    }

    // ── Interface with non-void return ────────────────────────────────────

    #[test]
    fn generate_interface_non_void() {
        let mut module = IrModule::new("");
        let iface = ir::decl::IrInterface {
            name: "Countable".into(),
            visibility: Visibility::Public,
            type_params: vec![],
            extends: vec![],
            methods: vec![
                ir::decl::IrMethod {
                    name: "count".into(),
                    visibility: Visibility::Public,
                    is_static: false,
                    is_abstract: false,
                    is_final: false,
                    is_synchronized: false,
                    type_params: vec![],
                    params: vec![],
                    return_ty: IrType::Int,
                    body: None,
                    throws: vec![],
                },
                ir::decl::IrMethod {
                    name: "reset".into(),
                    visibility: Visibility::Public,
                    is_static: false,
                    is_abstract: false,
                    is_final: false,
                    is_synchronized: false,
                    type_params: vec![],
                    params: vec![],
                    return_ty: IrType::Void,
                    body: None,
                    throws: vec![],
                },
            ],
        };
        module.decls.push(IrDecl::Interface(iface));
        let code = gen(&module);
        assert!(
            code.contains("fn count") && code.contains("-> i32"),
            "should emit non-void interface method"
        );
        assert!(
            code.contains("fn reset"),
            "should emit void interface method"
        );
    }

    // ── Abstract method (no body) ─────────────────────────────────────────

    #[test]
    fn generate_abstract_method() {
        let mut module = IrModule::new("");
        let cls = IrClass {
            name: "Abs".into(),
            visibility: Visibility::Public,
            is_abstract: true,
            is_final: false,
            superclass: None,
            interfaces: vec![],
            type_params: vec![],
            fields: vec![],
            methods: vec![ir::decl::IrMethod {
                name: "doWork".into(),
                visibility: Visibility::Public,
                is_static: false,
                is_abstract: false,
                is_final: false,
                is_synchronized: false,
                type_params: vec![],
                params: vec![],
                return_ty: IrType::Void,
                body: None,
                throws: vec![],
            }],
            constructors: vec![],
        };
        module.decls.push(IrDecl::Class(cls));
        gen(&module); // should not panic on abstract method (no body)
    }

    // ── print (no newline) ────────────────────────────────────────────────

    #[test]
    fn generate_print_float() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::Expr(IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::FieldAccess {
                receiver: Box::new(IrExpr::Var {
                    name: "System".into(),
                    ty: IrType::Class("System".into()),
                }),
                field_name: "out".into(),
                ty: IrType::Class("PrintStream".into()),
            })),
            method_name: "print".into(),
            args: vec![IrExpr::Var {
                name: "x".into(),
                ty: IrType::Float,
            }],
            ty: IrType::Void,
        });
        module
            .decls
            .push(IrDecl::Class(make_class("PrintFloat", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("print!") && code.contains("{:?}"),
            "should use print! with float format"
        );
    }

    // ── empty println (no args) ───────────────────────────────────────────

    #[test]
    fn generate_println_empty() {
        let mut module = IrModule::new("");
        let stmt = IrStmt::Expr(IrExpr::MethodCall {
            receiver: Some(Box::new(IrExpr::FieldAccess {
                receiver: Box::new(IrExpr::Var {
                    name: "System".into(),
                    ty: IrType::Class("System".into()),
                }),
                field_name: "out".into(),
                ty: IrType::Class("PrintStream".into()),
            })),
            method_name: "println".into(),
            args: vec![],
            ty: IrType::Void,
        });
        module
            .decls
            .push(IrDecl::Class(make_class("PrintlnEmpty", vec![stmt])));
        let code = gen(&module);
        assert!(
            code.contains("println!"),
            "should emit println!() for empty println"
        );
    }

    // ── Const static field ────────────────────────────────────────────────

    #[test]
    fn generate_const_static_field() {
        let mut module = IrModule::new("");
        let cls = IrClass {
            name: "Constants".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            superclass: None,
            interfaces: vec![],
            type_params: vec![],
            fields: vec![ir::decl::IrField {
                name: "MAX".into(),
                ty: IrType::Int,
                visibility: Visibility::Public,
                is_static: true,
                is_volatile: false,
                is_final: true,
                init: Some(IrExpr::LitInt(100)),
            }],
            methods: vec![],
            constructors: vec![],
        };
        module.decls.push(IrDecl::Class(cls));
        let code = gen(&module);
        assert!(
            code.contains("const MAX"),
            "should emit const for static final field"
        );
    }
}
