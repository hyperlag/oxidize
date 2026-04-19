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

/// Synthetic local-variable name used when lowering a pattern-`instanceof`
/// condition to avoid evaluating the checked expression twice.
const INSTANCEOF_TMP: &str = "__instanceof_tmp__";

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
    /// Maps "ClassName::method_name" → number of non-varargs parameters for varargs methods.
    /// Keyed by fully-qualified class+method so same-named methods in different classes
    /// don't accidentally wrap each other's call sites.
    /// Populated from ALL classes before any method body is emitted, so
    /// cross-class varargs calls resolve correctly.
    static VARARGS_METHODS: std::cell::RefCell<std::collections::HashMap<String, usize>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Maps Java field name → mangled Rust static name for mutable static
    /// (non-const) *primitive* fields backed by an Atomic* type.
    /// Cleared at the start of each `emit_class` call.
    static STATIC_ATOMIC_FIELDS: std::cell::RefCell<std::collections::HashMap<String, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Maps Java field name → mangled Rust static name for mutable static
    /// reference-type fields backed by `OnceLock<T>`.
    /// Cleared at the start of each `emit_class` call.
    static STATIC_ONCE_LOCK_FIELDS: std::cell::RefCell<std::collections::HashMap<String, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Cross-class lookup: `"ClassName::field_name"` → mangled Rust name for
    /// Atomic-backed static fields.  Populated once (pre-scan) before codegen,
    /// never cleared.
    static GLOBAL_STATIC_ATOMIC: std::cell::RefCell<std::collections::HashMap<String, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Cross-class lookup: `"ClassName::field_name"` → mangled Rust name for
    /// OnceLock-backed static fields.  Populated once (pre-scan) before codegen,
    /// never cleared.
    static GLOBAL_STATIC_ONCE_LOCK: std::cell::RefCell<std::collections::HashMap<String, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Whether the class currently being emitted has a `__clinit__` method
    /// (static initializer block).
    static HAS_STATIC_INIT: Cell<bool> = const { Cell::new(false) };
    /// Name of the class currently being emitted (used for static Once guard).
    static CURRENT_CLASS_NAME: std::cell::RefCell<String> =
        const { std::cell::RefCell::new(String::new()) };
    /// Maps a simple (original Java) class name to its hoisted mangled name
    /// for the class currently being emitted.  Used to rename `new Inner()`
    /// → `new Outer$Inner()` and `new Local()` → `new Outer__loc__Local()`.
    /// Populated per-class before emit_class; cleared afterwards.
    static INNER_CLASS_MAP: std::cell::RefCell<std::collections::HashMap<String, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// User-defined exception classes (extends RuntimeException / Exception / etc.).
    /// Populated in one pre-scan pass before code generation starts.
    static EXCEPTION_CLASSES: std::cell::RefCell<std::collections::HashSet<String>> =
        std::cell::RefCell::new(std::collections::HashSet::new());
    /// Type parameter names for the class currently being emitted
    /// (e.g. `{"T", "K", "V"}`).  Used so compareTo on T dispatches to cmp.
    static CLASS_TYPE_PARAMS: std::cell::RefCell<std::collections::HashSet<String>> =
        std::cell::RefCell::new(std::collections::HashSet::new());
    /// Type parameter names for the method currently being emitted.
    /// Cleared and re-populated at the start of each method body emission.
    static METHOD_TYPE_PARAMS: std::cell::RefCell<std::collections::HashSet<String>> =
        std::cell::RefCell::new(std::collections::HashSet::new());
    /// User-defined exception class hierarchy: child → direct parent.
    /// Used by the catch chain emitter to match subtypes.
    static EXCEPTION_HIERARCHY: std::cell::RefCell<std::collections::HashMap<String, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Non-static inner class → direct outer class name.
    /// e.g. `"Outer$Inner"` → `"Outer"`.  Populated in one pre-scan pass.
    static INNER_CLASS_OUTERS: std::cell::RefCell<std::collections::HashMap<String, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// User-defined RecursiveTask / RecursiveAction subclasses.
    /// Populated in one pre-scan pass before code generation starts.
    static RECURSIVE_TASK_CLASSES: std::cell::RefCell<std::collections::HashSet<String>> =
        std::cell::RefCell::new(std::collections::HashSet::new());
    /// All user-defined class names in the current module (populated in
    /// pre-scan).  Used to determine whether a `synchronized(obj)` monitor
    /// expression has an injected `__monitor` field.
    static USER_CLASSES: std::cell::RefCell<std::collections::HashSet<String>> =
        std::cell::RefCell::new(std::collections::HashSet::new());
    /// Name of the current `synchronized(expr)` monitor variable (if the
    /// monitor is a simple variable reference, not `this`).  Set while
    /// emitting the body of an arbitrary-object synchronized block so that
    /// `recv.wait()` / `recv.notify()` inside the block can route to the
    /// correct condvar.  `None` when inside a `this`-monitor block or outside
    /// any synchronized block.
    static SYNC_MONITOR_EXPR: std::cell::RefCell<Option<String>> =
        const { std::cell::RefCell::new(None) };
    /// Captured variable names for the anonymous class currently being emitted.
    /// Cleared and re-populated per class in `emit_class`.
    static ANON_CAPTURES: std::cell::RefCell<Vec<String>> =
        const { std::cell::RefCell::new(Vec::new()) };
    /// Maps anonymous class name → list of captured variable names.
    /// Populated in a pre-scan pass so that `New` sites know which captures
    /// to pass as constructor arguments.
    static ANON_CAPTURE_MAP: std::cell::RefCell<std::collections::HashMap<String, Vec<String>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Returns `true` if `name` is a well-known Java exception base class.
fn is_exception_base(name: &str) -> bool {
    matches!(
        name,
        "Exception"
            | "RuntimeException"
            | "Throwable"
            | "Error"
            | "IOException"
            | "IllegalArgumentException"
            | "IllegalStateException"
            | "NullPointerException"
            | "IndexOutOfBoundsException"
            | "ArrayIndexOutOfBoundsException"
            | "StringIndexOutOfBoundsException"
            | "ClassCastException"
            | "ArithmeticException"
            | "NumberFormatException"
            | "UnsupportedOperationException"
            | "StackOverflowError"
            | "ConcurrentModificationException"
            | "CloneNotSupportedException"
    )
}

/// Returns `true` if `name` is a built-in Java exception base class or a
/// user-defined class whose superclass is one.
fn is_exception_class(name: &str) -> bool {
    is_exception_base(name) || EXCEPTION_CLASSES.with(|ec| ec.borrow().contains(name))
}

/// Returns `true` if `name` is the built-in `RecursiveTask` or `RecursiveAction`
/// base class (the direct Java SDK parents of fork/join task classes).
/// The superclass stored in `IrClass` may include type parameters
/// (e.g. `"RecursiveTask<Integer>"`), so we check with `starts_with` as well.
fn is_recursive_task_base(name: &str) -> bool {
    matches!(name, "RecursiveTask" | "RecursiveAction")
        || name.starts_with("RecursiveTask<")
        || name.starts_with("RecursiveAction<")
}

/// Returns `true` if `name` is a user-defined class that extends
/// `RecursiveTask` or `RecursiveAction` (directly or transitively).
fn is_recursive_task_class(name: &str) -> bool {
    RECURSIVE_TASK_CLASSES.with(|rc| rc.borrow().contains(name))
}

/// Returns `true` if `name` is a user-defined class present in the current
/// module (and therefore has an injected `__monitor` field).  Used to choose
/// between per-object locking and the global fallback in `synchronized(obj)`
/// blocks.
fn is_user_class(name: &str) -> bool {
    USER_CLASSES.with(|uc| uc.borrow().contains(name))
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

    // Module-level allows to keep the generated code warning-free.
    items.push(quote! {
        #![allow(non_upper_case_globals, non_snake_case, unused_mut, unused_variables)]
    });

    // Emit a use for the runtime crate
    items.push(quote! {
        #[allow(unused_imports, non_upper_case_globals)]
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
            JInputStream, JOutputStream, JReader, JWriter,
            JBigDecimal, JMathContext, JRoundingMode,
            JURL, JSocket, JServerSocket, JHttpURLConnection,
            JSpliterator, JavaObject,
            JProcessBuilder, JProcess,
            JProperties,
            JTimer, JTimerTask,
            JZonedDateTime, JZoneId, JClock,
            JHttpClient, JHttpRequestBuilder, JHttpRequest, JHttpResponse,
            JStringWriter, JStringReader, JByteArrayOutputStream, JByteArrayInputStream,
            JResourceBundle,
            JStampedLock, JForkJoinPool, JForkJoinHandle, JMonitor,
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

    let interface_map: std::collections::HashMap<String, &IrInterface> = {
        let mut map = std::collections::HashMap::new();
        for d in &module.decls {
            if let IrDecl::Interface(i) = d {
                // Full mangled name: "Outer$Greeter"
                map.insert(i.name.clone(), i);
                // Simple-name alias for inner interfaces: "Greeter" → same entry.
                if let Some(simple) = i.name.rfind('$').map(|idx| &i.name[idx + 1..]) {
                    map.entry(simple.to_owned()).or_insert(i);
                }
            }
        }
        map
    };

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

    // Pre-scan ALL methods across all classes to populate VARARGS_METHODS
    // before any method body is emitted.  This ensures varargs call sites in
    // class B resolve even when the varargs method is defined in class A.
    // Keys are "ClassName::method_name" to avoid false-positives when
    // different classes happen to share the same method name.
    VARARGS_METHODS.with(|vm| {
        let mut map = vm.borrow_mut();
        map.clear();
        for decl in &module.decls {
            if let IrDecl::Class(cls) = decl {
                for method in &cls.methods {
                    if let Some(varargs_pos) = method.params.iter().position(|p| p.is_varargs) {
                        // Number of regular (non-varargs) parameters before the varargs one
                        let key = format!("{}::{}", cls.name, method.name);
                        map.insert(key, varargs_pos);
                    }
                }
            }
        }
    });

    // Pre-scan ALL static fields across all classes to populate the global
    // cross-class lookup maps.  This allows `ClassName.field` accesses from
    // other classes to resolve to the correct mangled Rust statics.
    GLOBAL_STATIC_ATOMIC.with(|m| m.borrow_mut().clear());
    GLOBAL_STATIC_ONCE_LOCK.with(|m| m.borrow_mut().clear());
    for decl in &module.decls {
        if let IrDecl::Class(cls) = decl {
            for f in cls.fields.iter().filter(|f| f.is_static) {
                if f.is_final {
                    if let Some(init) = &f.init {
                        if as_const_literal(init).is_some() {
                            continue; // emitted as `const`, no static backing
                        }
                    }
                }
                let mangled = format!("{}_{}", cls.name, f.name);
                let key = format!("{}::{}", cls.name, f.name);
                match &f.ty {
                    IrType::Int | IrType::Short | IrType::Byte | IrType::Long | IrType::Bool => {
                        GLOBAL_STATIC_ATOMIC.with(|m| {
                            m.borrow_mut().insert(key, mangled);
                        });
                    }
                    _ => {
                        GLOBAL_STATIC_ONCE_LOCK.with(|m| {
                            m.borrow_mut().insert(key, mangled);
                        });
                    }
                }
            }
        }
    }

    // Pre-scan: discover user-defined exception classes (classes that extend a
    // built-in or user-defined exception type).  Repeat until stable so that
    // multi-level hierarchies (A extends B extends RuntimeException) resolve.
    EXCEPTION_CLASSES.with(|ec| ec.borrow_mut().clear());
    loop {
        let mut changed = false;
        for decl in &module.decls {
            if let IrDecl::Class(cls) = decl {
                if !EXCEPTION_CLASSES.with(|ec| ec.borrow().contains(&cls.name)) {
                    if let Some(parent) = &cls.superclass {
                        if is_exception_class(parent) {
                            EXCEPTION_CLASSES.with(|ec| {
                                ec.borrow_mut().insert(cls.name.clone());
                            });
                            changed = true;
                        }
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }

    // Build the exception hierarchy map: child → direct parent (for user exceptions only).
    EXCEPTION_HIERARCHY.with(|eh| {
        let mut map = eh.borrow_mut();
        map.clear();
        for decl in &module.decls {
            if let IrDecl::Class(cls) = decl {
                if is_exception_class(&cls.name) {
                    if let Some(parent) = &cls.superclass {
                        map.insert(cls.name.clone(), parent.clone());
                    }
                }
            }
        }
    });

    // Pre-scan: discover user-defined RecursiveTask / RecursiveAction subclasses.
    // Repeat until stable so that multi-level hierarchies resolve.
    // Normalize superclass names by stripping generic type arguments (e.g.
    // `"MyTask<Integer>"` → `"MyTask"`) before lookup so that transitive
    // subclasses of generic RecursiveTask subclasses are detected correctly.
    RECURSIVE_TASK_CLASSES.with(|rc| rc.borrow_mut().clear());
    loop {
        let mut changed = false;
        for decl in &module.decls {
            if let IrDecl::Class(cls) = decl {
                if !RECURSIVE_TASK_CLASSES.with(|rc| rc.borrow().contains(&cls.name)) {
                    if let Some(parent) = &cls.superclass {
                        let parent_base = parent.split('<').next().unwrap_or(parent);
                        if is_recursive_task_base(parent) || is_recursive_task_class(parent_base) {
                            RECURSIVE_TASK_CLASSES.with(|rc| {
                                rc.borrow_mut().insert(cls.name.clone());
                            });
                            changed = true;
                        }
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }

    // Pre-scan: collect all user-defined class names.  Used to determine
    // whether a `synchronized(obj)` monitor expression has an injected
    // `__monitor` field (user classes do; runtime/built-in types don't).
    USER_CLASSES.with(|uc| {
        let mut set = uc.borrow_mut();
        set.clear();
        for decl in &module.decls {
            if let IrDecl::Class(cls) = decl {
                set.insert(cls.name.clone());
            }
        }
    });

    // Pre-scan: build anonymous-class capture map.
    ANON_CAPTURE_MAP.with(|m| {
        let mut map = m.borrow_mut();
        map.clear();
        for decl in &module.decls {
            if let IrDecl::Class(cls) = decl {
                if !cls.captures.is_empty() {
                    let names: Vec<String> = cls.captures.iter().map(|(n, _)| n.clone()).collect();
                    map.insert(cls.name.clone(), names);
                }
            }
        }
    });

    // Pre-scan: build inner-class → outer-class map.
    // Only `$`-prefixed classes are non-static inner classes; `__loc__` classes
    // are local (method-body) classes and don't need the __outer back-reference.
    INNER_CLASS_OUTERS.with(|m| {
        let mut map = m.borrow_mut();
        map.clear();
        for decl in &module.decls {
            if let IrDecl::Class(cls) = decl {
                if cls.name.contains('$') {
                    // Split at the LAST `$` to get the direct outer class.
                    if let Some(dollar_pos) = cls.name.rfind('$') {
                        let outer = cls.name[..dollar_pos].to_owned();
                        map.insert(cls.name.clone(), outer);
                    }
                }
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
                // Build INNER_CLASS_MAP for this class:
                // - inner class  "Outer$Inner"       → "Inner" → "Outer$Inner"
                // - local class  "Outer__loc__Local" → "Local" → "Outer__loc__Local"
                {
                    let outer_prefix = format!("{}$", cls.name);
                    let loc_prefix = format!("{}__loc__", cls.name);
                    let mut imap = std::collections::HashMap::new();
                    for other in &module.decls {
                        if let IrDecl::Class(c) = other {
                            if let Some(rest) = c.name.strip_prefix(&outer_prefix) {
                                // Key is the suffix after removing the `"{Outer}$"` prefix;
                                // it may still contain additional `$` separators (for example,
                                // `Inner$Nested` for `Outer$Inner$Nested`).
                                imap.insert(rest.to_owned(), c.name.clone());
                            } else if let Some(rest) = c.name.strip_prefix(&loc_prefix) {
                                imap.insert(rest.to_owned(), c.name.clone());
                            }
                        }
                    }
                    INNER_CLASS_MAP.with(|m| *m.borrow_mut() = imap);
                }
                items.push(emit_class(cls, &class_map, &interface_map, &enum_map)?);
                INNER_CLASS_MAP.with(|m| m.borrow_mut().clear());
            }
            IrDecl::Interface(iface) => {
                items.push(emit_interface(iface)?);
            }
            IrDecl::Enum(enm) => {
                if enm.methods.iter().any(|m| m.name == "main" && m.is_static) {
                    main_class = Some(enm.name.clone());
                }
                items.push(emit_enum(enm, &enum_map, &interface_map)?);
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
            if m.is_default {
                // Java 8+ default method: emit as a Rust trait method with a body.
                let body = m
                    .body
                    .as_deref()
                    .map(emit_stmts)
                    .unwrap_or_else(|| Ok(vec![]))?;
                if m.return_ty == IrType::Void {
                    Ok(quote! { fn #mname(&mut self, #(#params),*) { #(#body)* } })
                } else {
                    Ok(quote! { fn #mname(&mut self, #(#params),*) -> #ret_ty { #(#body)* } })
                }
            } else if m.return_ty == IrType::Void {
                Ok(quote! { fn #mname(&mut self, #(#params),*); })
            } else {
                Ok(quote! { fn #mname(&mut self, #(#params),*) -> #ret_ty; })
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
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
    interface_map: &std::collections::HashMap<String, &IrInterface>,
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

    // Collect method names that have at least one per-constant override (5C).
    let variant_override_names: std::collections::HashSet<&str> = enm
        .constants
        .iter()
        .flat_map(|c| c.body.iter().map(|m| m.name.as_str()))
        .collect();

    for method in &enm.methods {
        if variant_override_names.contains(method.name.as_str()) {
            // Generate a match-dispatch method for constant-specific overrides.
            let mname = ident(&method.name);
            let params = emit_params_sig(&method.params);
            let ret_ty = emit_type(&method.return_ty);
            let self_param = if method.is_static {
                quote! {}
            } else {
                quote! { &self, }
            };
            let arms: Vec<TokenStream> = enm
                .constants
                .iter()
                .map(|c| {
                    let cname = ident(&c.name);
                    // Use the constant's own override if present, else the
                    // default body from the enum-level method.
                    let body_method = c
                        .body
                        .iter()
                        .find(|m| m.name == method.name)
                        .unwrap_or(method);
                    let stmts = if let Some(body) = &body_method.body {
                        emit_stmts(body)?
                    } else {
                        vec![quote! { unimplemented!(); }]
                    };
                    Ok(quote! { Self::#cname => { #(#stmts)* } })
                })
                .collect::<Result<Vec<_>, CodegenError>>()?;
            let param_names: Vec<_> = method.params.iter().map(|p| ident(&p.name)).collect();
            let _ = param_names; // suppress unused warning
            if method.return_ty == IrType::Void {
                impl_methods.push(quote! {
                    pub fn #mname(#self_param #(#params),*) {
                        match self { #(#arms),* }
                    }
                });
            } else {
                impl_methods.push(quote! {
                    pub fn #mname(#self_param #(#params),*) -> #ret_ty {
                        match self { #(#arms),* }
                    }
                });
            }
        } else {
            let m = emit_enum_method(method)?;
            impl_methods.push(m);
        }
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

    // `impl Trait for Enum` blocks (5B)
    let mut trait_impls: Vec<TokenStream> = Vec::new();
    for iface_name in &enm.interfaces {
        if let Some(iface) = interface_map.get(iface_name.as_str()) {
            let iface_ident = ident(&iface.name);
            let impl_methods_ts: Vec<TokenStream> = iface
                .methods
                .iter()
                .filter(|m| !m.is_static)
                .map(|iface_method| {
                    let body_method = enm
                        .methods
                        .iter()
                        .find(|m| m.name == iface_method.name);
                    match body_method {
                        Some(m) => emit_trait_method(m),
                        None => {
                            let mname = ident(&iface_method.name);
                            let params = emit_params_sig(&iface_method.params);
                            let ret_ty = emit_type(&iface_method.return_ty);
                            let error_message = format!(
                                "enum `{}` claims to implement interface `{}` but does not provide required instance method `{}`",
                                enm.name, iface_name, iface_method.name
                            );
                            if iface_method.return_ty == IrType::Void {
                                Ok(quote! {
                                    fn #mname(&mut self, #(#params),*) {
                                        compile_error!(#error_message);
                                    }
                                })
                            } else {
                                Ok(quote! {
                                    fn #mname(&mut self, #(#params),*) -> #ret_ty {
                                        compile_error!(#error_message);
                                    }
                                })
                            }
                        }
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            trait_impls.push(quote! {
                impl #iface_ident for #name {
                    #(#impl_methods_ts)*
                }
            });
        }
    }

    Ok(quote! {
        #enum_def
        #impl_block
        #display_impl
        #(#trait_impls)*
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
    let (gen_names, gen_bounds): (Vec<Ident>, Vec<TokenStream>) = if cls.type_params.is_empty() {
        (vec![], vec![])
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
        (names, bounds)
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

    // Detect whether this is a non-static inner class (name contains '$').
    // If so, it needs a hidden `__outer: Rc<RefCell<Outer>>` reference field.
    let inner_outer_name: Option<String> =
        INNER_CLASS_OUTERS.with(|m| m.borrow().get(&cls.name).cloned());
    if let Some(ref outer_name) = inner_outer_name {
        let outer_ident = ident(outer_name);
        struct_fields
            .push(quote! { pub __outer: ::std::rc::Rc<::std::cell::RefCell<#outer_ident>>, });
    }

    if let Some(parent_name) = &cls.superclass {
        if is_exception_class(parent_name) {
            // Exception subclasses store a message string instead of a parent struct.
            struct_fields.push(quote! { pub message: JString, });
        } else if is_recursive_task_base(parent_name) || is_recursive_task_class(parent_name) {
            // RecursiveTask/RecursiveAction subclasses get a fork-join handle
            // injected instead of a _super field.  The handle type matches the
            // return type of the `compute()` method.
            let compute_ret = cls
                .methods
                .iter()
                .find(|m| m.name == "compute")
                .map(|m| m.return_ty.clone())
                .unwrap_or(IrType::Void);
            if compute_ret != IrType::Void {
                let ret_ty = emit_type(&compute_ret);
                struct_fields.push(quote! { pub __fork_handle: Option<JForkJoinHandle<#ret_ty>>, });
            }
        } else {
            // Use emit_type to map known Java parent names (e.g. TimerTask → JTimerTask).
            let parent_ty = emit_type(&IrType::Class(parent_name.clone()));
            struct_fields.push(quote! { pub _super: #parent_ty, });
        }
    }
    for f in &own_instance_fields {
        let fname = ident(&f.name);
        let fty = emit_type(&f.ty);
        struct_fields.push(quote! { pub #fname: #fty, });
    }

    // Anonymous-class captures: add `pub __cap_X: ConcreteType` fields.
    // Types are resolved in the parser from the enclosing scope.
    ANON_CAPTURES.with(|ac| ac.borrow_mut().clear());
    if !cls.captures.is_empty() {
        let cap_names: Vec<String> = cls.captures.iter().map(|(n, _)| n.clone()).collect();
        ANON_CAPTURES.with(|ac| *ac.borrow_mut() = cap_names);
        for (cap_name, cap_ty) in &cls.captures {
            let field_name = ident(&format!("__cap_{cap_name}"));
            let fty = emit_type(cap_ty);
            struct_fields.push(quote! { pub #field_name: #fty, });
        }
    }

    // Build final generic parameters from the collected names/bounds.
    let (struct_generics, impl_generics) = if gen_names.is_empty() {
        (quote! {}, quote! {})
    } else {
        (quote! { <#(#gen_names),*> }, quote! { <#(#gen_bounds),*> })
    };

    // Inject a per-object monitor so that `synchronized(obj)` blocks can
    // lock the *object's own* condvar.  Skip for exception subclasses (which
    // have no `_super` and use `message` instead) and for RecursiveTask
    // subclasses (which use `__fork_handle` instead), as well as for classes
    // that themselves extend another user class with a `_super` field that
    // already carries a monitor via composition.  For simplicity, inject into
    // every non-exception, non-fork-join class.
    let _parent_is_exception = cls
        .superclass
        .as_deref()
        .map(is_exception_class)
        .unwrap_or(false);
    let _parent_is_forkjoin = cls
        .superclass
        .as_deref()
        .map(|p| is_recursive_task_base(p) || is_recursive_task_class(p))
        .unwrap_or(false);
    if !_parent_is_exception && !_parent_is_forkjoin {
        struct_fields.push(quote! { pub __monitor: JMonitor, });
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

    // ── Static-field thread-local setup ──────────────────────────────────
    // Clear the per-class state and re-populate for this class.
    let cls_name_str = cls.name.clone();
    CURRENT_CLASS_NAME.with(|cn| *cn.borrow_mut() = cls_name_str.clone());
    STATIC_ATOMIC_FIELDS.with(|sf| sf.borrow_mut().clear());
    STATIC_ONCE_LOCK_FIELDS.with(|sf| sf.borrow_mut().clear());
    // Populate class-level type parameter names for compareTo dispatch.
    CLASS_TYPE_PARAMS.with(|ctp| {
        let mut set = ctp.borrow_mut();
        set.clear();
        for tp in &cls.type_params {
            set.insert(tp.name.clone());
        }
    });

    // Static fields → const items (for static final with literal value)
    //               → module-level Rust statics (for mutable primitive statics)
    let mut static_items: Vec<TokenStream> = Vec::new();
    let has_clinit = cls.methods.iter().any(|m| m.name == "__clinit__");
    HAS_STATIC_INIT.with(|c| c.set(has_clinit));
    if has_clinit {
        // Emit a Once guard for the static initializer block.
        let once_name = ident(&format!("__STATIC_INIT_ONCE_{}", cls.name));
        static_items.push(quote! {
            static #once_name: ::std::sync::Once = ::std::sync::Once::new();
        });
    }
    for f in cls.fields.iter().filter(|f| f.is_static) {
        // static final field with a compile-time literal → const
        if f.is_final {
            if let Some(init) = &f.init {
                if let Some(lit) = as_const_literal(init) {
                    let fname = ident(&f.name.to_uppercase());
                    let fty = emit_type(&f.ty);
                    static_items.push(quote! { pub const #fname: #fty = #lit; });
                    continue;
                }
            }
        }
        // Mutable static primitive field → AtomicI32 / AtomicI64 / AtomicBool
        let mangled = format!("{}_{}", cls.name, f.name);
        let mangled_ident = ident(&mangled);
        match &f.ty {
            IrType::Int | IrType::Short | IrType::Byte => {
                let init_val = f.init.as_ref().and_then(as_i32_literal).unwrap_or(0);
                let lit = Literal::i32_unsuffixed(init_val);
                static_items.push(quote! {
                    static #mangled_ident: ::std::sync::atomic::AtomicI32 =
                        ::std::sync::atomic::AtomicI32::new(#lit);
                });
                STATIC_ATOMIC_FIELDS.with(|sf| {
                    sf.borrow_mut().insert(f.name.clone(), mangled.clone());
                });
            }
            IrType::Long => {
                let init_val = f.init.as_ref().and_then(as_i64_literal).unwrap_or(0);
                let lit = Literal::i64_unsuffixed(init_val);
                static_items.push(quote! {
                    static #mangled_ident: ::std::sync::atomic::AtomicI64 =
                        ::std::sync::atomic::AtomicI64::new(#lit);
                });
                STATIC_ATOMIC_FIELDS.with(|sf| {
                    sf.borrow_mut().insert(f.name.clone(), mangled.clone());
                });
            }
            IrType::Bool => {
                let init_val = f.init.as_ref().and_then(as_bool_literal).unwrap_or(false);
                static_items.push(quote! {
                    static #mangled_ident: ::std::sync::atomic::AtomicBool =
                        ::std::sync::atomic::AtomicBool::new(#init_val);
                });
                STATIC_ATOMIC_FIELDS.with(|sf| {
                    sf.borrow_mut().insert(f.name.clone(), mangled.clone());
                });
            }
            _ => {
                // Non-primitive static: OnceLock for reference types.
                // Tracked in STATIC_ONCE_LOCK_FIELDS (not STATIC_ATOMIC_FIELDS) so
                // reads/writes use get_or_init / set instead of atomic ops.
                let fty = emit_type(&f.ty);
                static_items.push(quote! {
                    static #mangled_ident: ::std::sync::OnceLock<#fty> =
                        ::std::sync::OnceLock::new();
                });
                STATIC_ONCE_LOCK_FIELDS.with(|sf| {
                    sf.borrow_mut().insert(f.name.clone(), mangled.clone());
                });
            }
        }
    }

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

    // If the class has a static initializer block (`static { ... }`), emit a
    // Once-guarded `__run_static_init()` method.  All other static methods
    // will call this at their start.
    if has_clinit {
        if let Some(clinit) = cls.methods.iter().find(|m| m.name == "__clinit__") {
            let once_name = ident(&format!("__STATIC_INIT_ONCE_{}", cls.name));
            let body = emit_stmts(clinit.body.as_deref().unwrap_or(&[]))?;
            method_tokens.push(quote! {
                fn __run_static_init() {
                    #once_name.call_once(|| {
                        #(#body)*
                    });
                }
            });
        }
    }

    // Always emit a `pub fn new() -> Self` even when no explicit constructor
    // exists, so that `new Foo()` calls in the generated code compile.
    if cls.constructors.is_empty() {
        if !cls.captures.is_empty() {
            // Anonymous class with captured variables: generate new(cap0, cap1, ...)
            let cap_params: Vec<TokenStream> = cls
                .captures
                .iter()
                .map(|(cap_name, cap_ty)| {
                    let field = ident(&format!("__cap_{cap_name}"));
                    let ty = emit_type(cap_ty);
                    quote! { #field: #ty }
                })
                .collect();
            let cap_inits: Vec<TokenStream> = cls
                .captures
                .iter()
                .map(|(cap_name, _)| {
                    let field = ident(&format!("__cap_{cap_name}"));
                    quote! { #field }
                })
                .collect();
            method_tokens.push(quote! {
                pub fn new(#(#cap_params),*) -> Self {
                    Self { #(#cap_inits,)* ..Default::default() }
                }
            });
        } else if let Some(ref outer_name) = inner_outer_name {
            // Inner class: new() requires the outer reference as first arg.
            let outer_ident = ident(outer_name);
            method_tokens.push(quote! {
                pub fn new(
                    __outer: ::std::rc::Rc<::std::cell::RefCell<#outer_ident>>,
                ) -> Self {
                    Self { __outer, ..Default::default() }
                }
            });
        } else if let Some(parent_name) = &cls.superclass {
            // Subclass with a user-class parent and no explicit constructor:
            // pre-create _super so we can share its __monitor with this class.
            // Only user-defined parent classes have an injected __monitor field;
            // built-in/runtime parents (e.g. TimerTask → JTimerTask) do not.
            if !is_exception_class(parent_name)
                && !is_recursive_task_base(parent_name)
                && !is_recursive_task_class(parent_name)
                && is_user_class(parent_name)
            {
                let parent_ty = emit_type(&IrType::Class(parent_name.clone()));
                method_tokens.push(quote! {
                    pub fn new() -> Self {
                        let __self_super = #parent_ty::new();
                        let __self_monitor = __self_super.__monitor.clone();
                        Self { _super: __self_super, __monitor: __self_monitor, ..Default::default() }
                    }
                });
            } else {
                method_tokens.push(quote! {
                    pub fn new() -> Self {
                        Self::default()
                    }
                });
            }
        } else {
            method_tokens.push(quote! {
                pub fn new() -> Self {
                    Self::default()
                }
            });
        }
    }
    for ctor in &cls.constructors {
        method_tokens.push(emit_constructor(ctor, cls)?);
    }
    for method in &cls.methods {
        if method.name == "__clinit__" {
            continue; // already handled above as __run_static_init
        }
        if !interface_methods.contains(&method.name) {
            method_tokens.push(emit_method(method, &cls.name)?);
        }
    }
    method_tokens.extend(delegation_methods);

    // Inject fork() / join() for RecursiveTask / RecursiveAction subclasses.
    if let Some(parent_name) = &cls.superclass {
        if is_recursive_task_base(parent_name) || is_recursive_task_class(parent_name) {
            let compute_ret = cls
                .methods
                .iter()
                .find(|m| m.name == "compute")
                .map(|m| m.return_ty.clone())
                .unwrap_or(IrType::Void);
            if compute_ret != IrType::Void {
                let ret_ty = emit_type(&compute_ret);
                method_tokens.push(quote! {
                    /// Asynchronously execute `compute()` in a new thread and stash
                    /// the handle in `self.__fork_handle`.
                    pub fn fork(&mut self) {
                        let mut __fjp_copy = self.clone();
                        let __fjp_handle = JForkJoinHandle::<#ret_ty>::__new();
                        let __fjp_handle_clone = __fjp_handle.clone();
                        ::std::thread::spawn(move || {
                            let __fjp_result = __fjp_copy.compute();
                            __fjp_handle_clone.__set(__fjp_result);
                        });
                        self.__fork_handle = Some(__fjp_handle);
                    }
                });
                method_tokens.push(quote! {
                    /// Block until the previously `fork()`ed `compute()` finishes
                    /// and return its result.
                    pub fn join(&mut self) -> #ret_ty {
                        self.__fork_handle
                            .as_ref()
                            .expect("join() called before fork()")
                            .__get()
                    }
                });
            } else {
                // RecursiveAction: fork/join with no return value.
                method_tokens.push(quote! {
                    pub fn fork(&mut self) {
                        let mut __fjp_copy = self.clone();
                        ::std::thread::spawn(move || { __fjp_copy.compute(); });
                    }
                });
                method_tokens.push(quote! {
                    pub fn join(&mut self) {}
                });
            }
        }
    }

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
        let super_check = if let Some(parent_name) = &cls.superclass {
            if is_exception_class(parent_name) {
                // For exception subclasses, walk the exception hierarchy instead of
                // delegating to self._super (which doesn't exist for exception types).
                let ancestor_names: Vec<String> = EXCEPTION_HIERARCHY.with(|eh| {
                    let map = eh.borrow();
                    let mut names = Vec::new();
                    let mut current = Some(parent_name.as_str().to_owned());
                    while let Some(cur_name) = current {
                        names.push(cur_name.clone());
                        current = map.get(&cur_name).cloned();
                    }
                    // Always include the standard base names.
                    if !names.iter().any(|n| n == "Exception") {
                        names.push("Exception".to_owned());
                    }
                    if !names.iter().any(|n| n == "Throwable") {
                        names.push("Throwable".to_owned());
                    }
                    names
                });
                quote! {
                    #(|| type_name == #ancestor_names)*
                }
            } else if is_recursive_task_base(parent_name) || is_recursive_task_class(parent_name) {
                // RecursiveTask/RecursiveAction: no _super field; just check the
                // well-known base names.
                quote! { || type_name == "RecursiveTask" || type_name == "RecursiveAction" }
            } else {
                quote! { || self._super._instanceof(type_name) }
            }
        } else {
            quote! {}
        };
        method_tokens.push(quote! {
            pub fn _instanceof(&self, type_name: &str) -> bool {
                type_name == #own_name_str #(#iface_checks)* #super_check
            }
        });
    }

    // Record accessor methods: each component field gets a public getter,
    // unless an explicit zero-arg instance method with the same name already
    // exists (Java allows overriding canonical accessors).
    if cls.is_record {
        for f in cls.fields.iter().filter(|f| !f.is_static) {
            let already_defined = cls
                .methods
                .iter()
                .any(|m| m.name == f.name && !m.is_static && m.params.is_empty());
            if already_defined {
                continue;
            }
            let fname = ident(&f.name);
            let fty = emit_type(&f.ty);
            method_tokens.push(quote! {
                pub fn #fname(&self) -> #fty {
                    self.#fname.clone()
                }
            });
        }
    }

    // For exception classes: add `getMessage()` if not already defined.
    if is_exception_class(&cls.name)
        && !cls
            .methods
            .iter()
            .any(|m| m.name == "getMessage" && !m.is_static)
    {
        method_tokens.push(quote! {
            pub fn getMessage(&self) -> JString {
                self.message.clone()
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
            // Use the canonical (potentially mangled) name from the IrInterface,
            // since that is what emit_interface used to declare the trait.
            let iface_ident = ident(&iface.name);
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
                            if iface_method.is_default {
                                // Use the default method body from the interface.
                                emit_trait_method(iface_method)
                            } else {
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

    // `impl Display` — auto-generated for records (Java-format) and for classes
    // that define a `toString()` method.  Exception classes get their own Display.
    let display_impl = if cls.is_record {
        // Records emit: "ClassName[field1=VALUE1, field2=VALUE2]"
        let instance_fields: Vec<&ir::decl::IrField> =
            cls.fields.iter().filter(|f| !f.is_static).collect();
        let field_format_parts: Vec<String> = instance_fields
            .iter()
            .map(|f| format!("{}={{}}", f.name))
            .collect();
        let format_str = format!("{}[{}]", cls.name, field_format_parts.join(", "));
        let field_refs: Vec<TokenStream> = instance_fields
            .iter()
            .map(|f| {
                let field_ident = ident(&f.name);
                quote! { &self.#field_ident }
            })
            .collect();
        quote! {
            impl #impl_generics ::std::fmt::Display for #name #struct_generics {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                    write!(f, #format_str, #(#field_refs),*)
                }
            }
        }
    } else if is_exception_class(&cls.name) {
        // Exception classes: Display shows "ClassName: message".
        let class_name_str = &cls.name;
        quote! {
            impl #impl_generics ::std::fmt::Display for #name #struct_generics {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                    write!(f, "{}: {}", #class_name_str, self.message)
                }
            }
        }
    } else if cls
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

    // For exception classes, emit `std::error::Error`.
    let exception_error_impl = if is_exception_class(&cls.name) {
        quote! {
            impl #impl_generics ::std::error::Error for #name #struct_generics {}
        }
    } else {
        quote! {}
    };

    // If the class implements `Comparable<T>` and has a `compareTo` method,
    // emit PartialEq, Eq, PartialOrd, Ord delegating to compareTo.
    let comparable_impls = {
        let has_comparable = cls
            .interfaces
            .iter()
            .any(|iname| iname == "Comparable" || iname.starts_with("Comparable<"));
        let has_compare_to = cls
            .methods
            .iter()
            .any(|m| m.name == "compareTo" && !m.is_static);
        if has_comparable && has_compare_to {
            quote! {
                impl #impl_generics PartialEq for #name #struct_generics {
                    fn eq(&self, other: &Self) -> bool {
                        self.clone().compareTo(other.clone()) == 0
                    }
                }
                impl #impl_generics Eq for #name #struct_generics {}
                impl #impl_generics PartialOrd for #name #struct_generics {
                    fn partial_cmp(&self, other: &Self) -> Option<::std::cmp::Ordering> {
                        Some(self.cmp(other))
                    }
                }
                impl #impl_generics Ord for #name #struct_generics {
                    fn cmp(&self, other: &Self) -> ::std::cmp::Ordering {
                        let n = self.clone().compareTo(other.clone());
                        if n < 0 {
                            ::std::cmp::Ordering::Less
                        } else if n == 0 {
                            ::std::cmp::Ordering::Equal
                        } else {
                            ::std::cmp::Ordering::Greater
                        }
                    }
                }
            }
        } else {
            quote! {}
        }
    };

    Ok(quote! {
        #struct_def
        #(#static_items)*
        #impl_block
        #display_impl
        #exception_error_impl
        #comparable_impls
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

    // If this is a non-static inner class, prepend the `__outer` param and
    // include it in the struct initializer.
    let inner_outer_name: Option<String> =
        INNER_CLASS_OUTERS.with(|m| m.borrow().get(&cls.name).cloned());
    let outer_param: TokenStream = if let Some(ref outer_name) = inner_outer_name {
        let outer_ident = ident(outer_name);
        quote! { __outer: ::std::rc::Rc<::std::cell::RefCell<#outer_ident>>, }
    } else {
        quote! {}
    };
    let outer_field_init: TokenStream = if inner_outer_name.is_some() {
        quote! { __outer, }
    } else {
        quote! {}
    };

    // Build the struct initializer.
    // For user-class parents, pre-create the `_super` value so we can clone
    // its `__monitor` into this class's `__monitor`.  Both levels then share
    // the same underlying Arc<(Mutex, Condvar)>, preserving Java's single
    // per-object monitor regardless of the composition depth.
    let (super_pre_stmt, super_field_init): (TokenStream, Option<TokenStream>) =
        if let Some(parent_name) = &cls.superclass {
            if is_exception_class(parent_name) {
                // Exception parent: map super(msg) call to the `message` field.
                let msg_ts = super_args
                    .as_ref()
                    .and_then(|a| a.first())
                    .map(emit_expr)
                    .transpose()?
                    .unwrap_or_else(|| quote! { JString::from("") });
                (quote! {}, Some(quote! { message: #msg_ts, }))
            } else if is_recursive_task_base(parent_name) || is_recursive_task_class(parent_name) {
                // RecursiveTask/RecursiveAction: inject a None handle; no _super field.
                let compute_ret = cls
                    .methods
                    .iter()
                    .find(|m| m.name == "compute")
                    .map(|m| m.return_ty.clone())
                    .unwrap_or(IrType::Void);
                let field_init = if compute_ret != IrType::Void {
                    Some(quote! { __fork_handle: None, })
                } else {
                    None
                };
                (quote! {}, field_init)
            } else if is_user_class(parent_name) {
                // User-defined parent: pre-create _super so we can share its
                // __monitor with this class (Java objects have a single monitor).
                let parent_ty = emit_type(&IrType::Class(parent_name.clone()));
                let super_arg_ts: Vec<TokenStream> = super_args
                    .as_ref()
                    .map(|args| args.iter().map(emit_expr).collect::<Result<Vec<_>, _>>())
                    .transpose()?
                    .unwrap_or_default();
                let pre = quote! {
                    let __self_super = #parent_ty::new(#(#super_arg_ts),*);
                    let __self_monitor = __self_super.__monitor.clone();
                };
                let fields = quote! { _super: __self_super, __monitor: __self_monitor, };
                (pre, Some(fields))
            } else {
                // Built-in/runtime parent (e.g. TimerTask → JTimerTask): no
                // __monitor field on the parent, so emit the _super init normally.
                let parent_ty = emit_type(&IrType::Class(parent_name.clone()));
                let super_arg_ts: Vec<TokenStream> = super_args
                    .as_ref()
                    .map(|args| args.iter().map(emit_expr).collect::<Result<Vec<_>, _>>())
                    .transpose()?
                    .unwrap_or_default();
                (
                    quote! {},
                    Some(quote! { _super: #parent_ty::new(#(#super_arg_ts),*), }),
                )
            }
        } else {
            (quote! {}, None)
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
    // `..Default::default()` fills in any injected fields (e.g. `__monitor`)
    // that are not listed explicitly.
    let struct_init = quote! {
        #super_pre_stmt
        let mut __self__: Self = Self {
            #outer_field_init
            #super_field_init
            #(#own_field_inits)*
            ..Default::default()
        };
    };

    let body = emit_stmts(&rest_stmts)?;

    // Call __run_static_init() at the top of every constructor so that class
    // initialization runs before the first object construction, matching Java
    // class-initialization semantics.
    let static_init_preamble = if HAS_STATIC_INIT.with(|c| c.get()) {
        quote! { Self::__run_static_init(); }
    } else {
        quote! {}
    };

    Ok(quote! {
        pub fn new(#outer_param #(#params),*) -> Self {
            #static_init_preamble
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

    // Method-level generic type parameters: `fn foo<T: Clone + Debug + PartialOrd + Ord>(...)`.
    let method_type_params = if method.type_params.is_empty() {
        quote! {}
    } else {
        let bounds: Vec<_> = method
            .type_params
            .iter()
            .map(|tp| {
                let id = ident(&tp.name);
                let extra = extra_bounds_for_type_param(tp);
                quote! { #id: Clone + ::std::default::Default + ::std::fmt::Debug #extra }
            })
            .collect();
        quote! { <#(#bounds),*> }
    };
    // Populate method-level type param names so compareTo can dispatch to cmp.
    METHOD_TYPE_PARAMS.with(|mtp| {
        let mut set = mtp.borrow_mut();
        set.clear();
        for tp in &method.type_params {
            set.insert(tp.name.clone());
        }
    });
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

    // Static initializer guard: ensure `__run_static_init()` is called before
    // every method (static and instance alike) except the init method itself.
    // This covers all active uses of the class as required by Java semantics.
    let static_init_preamble = if HAS_STATIC_INIT.with(|c| c.get()) {
        quote! { Self::__run_static_init(); }
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
            pub fn #name #method_type_params(#self_param #(#params),*) #ret_clause {
                #sync_preamble
                #static_init_preamble
                #(#body)*
            }
        })
    } else {
        Ok(quote! {
            fn #name #method_type_params(#self_param #(#params),*) #ret_clause {
                #sync_preamble
                #static_init_preamble
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
            if p.is_varargs {
                // Java `T... args` → Rust `args: JArray<T>`
                // The parser stores the varargs param type as Array(elem) so
                // that .length accesses resolve via the normal Array path.
                let elem_ty = match &p.ty {
                    IrType::Array(inner) => emit_type(inner),
                    other => emit_type(other),
                };
                quote! { mut #name: JArray<#elem_ty> }
            } else {
                let ty = emit_type(&p.ty);
                quote! { mut #name: #ty }
            }
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
/// contains a `continue` that targets the enclosing for-loop (either a bare
/// `continue` or a labeled `continue` whose label matches `for_label`).
fn for_body_has_continue(stmts: &[IrStmt], for_label: Option<&str>) -> bool {
    stmts.iter().any(|s| for_stmt_has_continue(s, for_label))
}

fn for_stmt_has_continue(stmt: &IrStmt, for_label: Option<&str>) -> bool {
    match stmt {
        IrStmt::Continue(None) => true,
        IrStmt::Continue(Some(lbl)) => for_label == Some(lbl.as_str()),
        IrStmt::If { then_, else_, .. } => {
            for_body_has_continue(then_, for_label)
                || else_
                    .as_deref()
                    .map(|s| for_body_has_continue(s, for_label))
                    .unwrap_or(false)
        }
        IrStmt::Block(inner) => for_body_has_continue(inner, for_label),
        // Nested loops own their own breaks/continues — don't recurse.
        IrStmt::While { .. }
        | IrStmt::For { .. }
        | IrStmt::DoWhile { .. }
        | IrStmt::ForEach { .. } => false,
        _ => false,
    }
}

/// Re-emit `stmts` for use inside a labeled for-body loop, replacing bare
/// `continue` (and labeled `continue` targeting `for_label`) → `break 'for_body`
/// and bare `break` → `break 'for_loop`.
fn transform_for_body(
    stmts: &[IrStmt],
    for_label: Option<&str>,
) -> Result<Vec<TokenStream>, CodegenError> {
    stmts
        .iter()
        .map(|s| transform_for_body_stmt(s, for_label))
        .collect()
}

fn transform_for_body_stmt(
    stmt: &IrStmt,
    for_label: Option<&str>,
) -> Result<TokenStream, CodegenError> {
    match stmt {
        IrStmt::Continue(None) => Ok(quote! { break 'for_body; }),
        IrStmt::Continue(Some(lbl)) if for_label == Some(lbl.as_str()) => {
            // `continue <for_label>` in Java means "run the update then loop":
            // translate to `break 'for_body` so the update step still executes.
            Ok(quote! { break 'for_body; })
        }
        IrStmt::Continue(Some(lbl)) => {
            let lt = label_lifetime(lbl);
            Ok(quote! { continue #lt; })
        }
        IrStmt::Break(None) => Ok(quote! { break 'for_loop; }),
        IrStmt::Break(Some(lbl)) => {
            let lt = label_lifetime(lbl);
            Ok(quote! { break #lt; })
        }
        IrStmt::If { cond, then_, else_ } => {
            let cond_ts = emit_expr(cond)?;
            let then_ts = transform_for_body(then_, for_label)?;
            if let Some(else_stmts) = else_ {
                let else_ts = transform_for_body(else_stmts, for_label)?;
                Ok(quote! { if #cond_ts { #(#then_ts)* } else { #(#else_ts)* } })
            } else {
                Ok(quote! { if #cond_ts { #(#then_ts)* } })
            }
        }
        IrStmt::Block(inner) => {
            let inner_ts = transform_for_body(inner, for_label)?;
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
            if let Some(init_expr) = init {
                // When the RHS constructs an anonymous inner class (name starts
                // with `__Anon_`), or an inner/local class that has been hoisted
                // and renamed, emit the concrete struct type rather than the
                // Java-declared type so that Rust accepts the value.
                let effective_ty_ts = if let IrExpr::New { class: new_cls, .. } = init_expr {
                    if new_cls.starts_with("__Anon_") {
                        // Anonymous class: use the concrete anon struct name.
                        let cid = ident(new_cls);
                        quote! { #cid }
                    } else {
                        // Inner/local class: check if the DECLARED type needs
                        // renaming (e.g. `Counter` → `InnerClass$Counter`).
                        let renamed_ty = if let IrType::Class(ref type_name) = ty {
                            INNER_CLASS_MAP
                                .with(|m| m.borrow().get(type_name.as_str()).cloned())
                                .map(IrType::Class)
                                .unwrap_or_else(|| ty.clone())
                        } else {
                            ty.clone()
                        };
                        emit_type(&renamed_ty)
                    }
                } else {
                    emit_type(ty)
                };
                let val = emit_expr(init_expr)?;
                // When the declared type is an abstract I/O base class and the
                // init is a concrete constructor, bridge with `.into()` so that
                // e.g. `InputStream is = new FileInputStream(...)` compiles as
                // `let mut is: JInputStream = JFileInputStream::new(...).into();`.
                let needs_io_into = matches!(
                    ty,
                    IrType::Class(ref c)
                    if matches!(c.as_str(), "InputStream" | "OutputStream" | "Reader" | "Writer")
                ) && matches!(init_expr, IrExpr::New { .. });
                if needs_io_into {
                    Ok(quote! { let mut #n: #effective_ty_ts = (#val).into(); })
                } else {
                    Ok(quote! { let mut #n: #effective_ty_ts = #val; })
                }
            } else {
                let t = emit_type(ty);
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
            // Pattern instanceof: `if (expr instanceof Type name)` injects a
            // binding variable at the start of the then-block.
            // Evaluate `inst_expr` into a temp to avoid double evaluation.
            if let IrExpr::InstanceOf {
                expr: inst_expr,
                check_type,
                binding: Some(binding_name),
            } = cond
            {
                let tmp_name = ident(INSTANCEOF_TMP);
                let inner_expr = emit_expr(inst_expr)?;
                let cond_no_binding = IrExpr::InstanceOf {
                    expr: Box::new(IrExpr::Var {
                        name: INSTANCEOF_TMP.to_string(),
                        ty: inst_expr.ty().clone(),
                    }),
                    check_type: check_type.clone(),
                    binding: None,
                };
                let cond_ts = emit_expr(&cond_no_binding)?;
                let bname = ident(binding_name);
                let bty = emit_type(check_type);
                let binding_decl = quote! { let mut #bname: #bty = #tmp_name.clone(); };
                let then_ts = emit_stmts(then_)?;
                if let Some(else_stmts) = else_ {
                    let else_ts = emit_stmts(else_stmts)?;
                    return Ok(quote! {
                        { let #tmp_name = #inner_expr; if #cond_ts { #binding_decl #(#then_ts)* } else { #(#else_ts)* } }
                    });
                } else {
                    return Ok(quote! {
                        { let #tmp_name = #inner_expr; if #cond_ts { #binding_decl #(#then_ts)* } }
                    });
                }
            }
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
        IrStmt::While { cond, body, label } => {
            let cond_ts = emit_expr(cond)?;
            let body_ts = emit_stmts(body)?;
            if let Some(lbl) = label {
                let lt = label_lifetime(lbl);
                Ok(quote! { #lt: while #cond_ts { #(#body_ts)* } })
            } else {
                Ok(quote! { while #cond_ts { #(#body_ts)* } })
            }
        }
        IrStmt::DoWhile { body, cond, label } => {
            let body_ts = emit_stmts(body)?;
            let cond_ts = emit_expr(cond)?;
            if let Some(lbl) = label {
                let lt = label_lifetime(lbl);
                Ok(quote! {
                    #lt: loop {
                        #(#body_ts)*
                        if !(#cond_ts) { break; }
                    }
                })
            } else {
                Ok(quote! {
                    loop {
                        #(#body_ts)*
                        if !(#cond_ts) { break; }
                    }
                })
            }
        }
        IrStmt::For {
            init,
            cond,
            update,
            body,
            label,
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
            // If the body contains a bare `continue` (or labeled `continue` targeting
            // this loop), a simple while-loop desugaring is wrong: the update would be
            // skipped. Use labeled loops so that `continue` (→ `break 'for_body`) still
            // runs the update before the next iteration.
            if for_body_has_continue(body, label.as_deref()) {
                let body_ts = transform_for_body(body, label.as_deref())?;
                if let Some(lbl) = label {
                    let outer_lt = label_lifetime(lbl);
                    Ok(quote! {
                        {
                            #init_ts
                            #outer_lt: loop {
                                if !(#cond_ts) { break #outer_lt; }
                                'for_body: loop {
                                    #(#body_ts)*
                                    break 'for_body;
                                }
                                #(#update_ts)*
                            }
                        }
                    })
                } else {
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
                }
            } else {
                let body_ts = emit_stmts(body)?;
                if let Some(lbl) = label {
                    let outer_lt = label_lifetime(lbl);
                    Ok(quote! {
                        {
                            #init_ts
                            #outer_lt: while #cond_ts {
                                #(#body_ts)*
                                #(#update_ts)*
                            }
                        }
                    })
                } else {
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
        }
        IrStmt::ForEach {
            var,
            var_ty,
            iterable,
            body,
            label,
        } => {
            let v = ident(var);
            let ty = emit_type(var_ty);
            let iter_ts = emit_expr(iterable)?;
            let body_ts = emit_stmts(body)?;
            if let Some(lbl) = label {
                let lt = label_lifetime(lbl);
                Ok(quote! {
                    #lt: for #v in #iter_ts.iter() {
                        let #v: #ty = #v.clone();
                        #(#body_ts)*
                    }
                })
            } else {
                Ok(quote! {
                    for #v in #iter_ts.iter() {
                        let #v: #ty = #v.clone();
                        #(#body_ts)*
                    }
                })
            }
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
        IrStmt::Break(None) => Ok(quote! { break; }),
        IrStmt::Break(Some(lbl)) => {
            let lt = label_lifetime(lbl);
            Ok(quote! { break #lt; })
        }
        IrStmt::Continue(None) => Ok(quote! { continue; }),
        IrStmt::Continue(Some(lbl)) => {
            let lt = label_lifetime(lbl);
            Ok(quote! { continue #lt; })
        }
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
            let catch_chain = {
                let hierarchy = EXCEPTION_HIERARCHY.with(|eh| eh.borrow().clone());
                emit_catch_chain(catches, &hierarchy)?
            };
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
        IrStmt::Synchronized { monitor, body } => {
            // Determine whether the monitor is `this` / `self` (per-object lock)
            // or an arbitrary object.
            let is_self_monitor = matches!(
                monitor,
                IrExpr::Var { name, .. } if name == "this" || name == "self"
            );
            if is_self_monitor {
                // `synchronized(this)` → use the per-object __monitor field,
                // consistent with Java's single per-object monitor semantics.
                // Extract the exact variable name ("this" or "self") so that
                // `this.wait()` / `this.notify()` inside the body route through
                // the block's __sync_cond rather than calling a nonexistent method.
                let monitor_var_name: Option<String> = if let IrExpr::Var { name, .. } = monitor {
                    Some(name.clone())
                } else {
                    None
                };
                SYNC_MONITOR_EXPR.with(|m| *m.borrow_mut() = monitor_var_name);
                let body_ts = emit_stmts(body)?;
                SYNC_MONITOR_EXPR.with(|m| *m.borrow_mut() = None);
                Ok(quote! {
                    {
                        let __sync_arc = self.__monitor.pair();
                        let (__sync_lock, __sync_cond) = &*__sync_arc;
                        let mut __sync_guard = __sync_lock.lock().unwrap();
                        let _ = &__sync_guard;
                        { #(#body_ts)* }
                        drop(__sync_guard);
                    }
                })
            } else {
                // Arbitrary-object monitor.
                // Track the monitor variable name so that `obj.wait()` /
                // `obj.notify()` inside the body can route to the right condvar.
                let monitor_var_name: Option<String> = if let IrExpr::Var { name, .. } = monitor {
                    Some(name.clone())
                } else {
                    None
                };
                SYNC_MONITOR_EXPR.with(|m| *m.borrow_mut() = monitor_var_name.clone());
                let mon_ts = emit_expr(monitor)?;
                let body_ts = emit_stmts(body)?;
                // Clear after body is emitted so the slot is not reused
                // accidentally.
                SYNC_MONITOR_EXPR.with(|m| *m.borrow_mut() = None);

                // Use obj.__monitor when the monitor expression is a user-defined
                // class (which has an injected __monitor field).  Fall back to the
                // process-global __sync_block_monitor() for runtime/built-in types
                // (String, collections, arrays, exceptions) that don't have that
                // field.
                let has_monitor_field = matches!(
                    monitor.ty(),
                    IrType::Class(name)
                        if is_user_class(name) && !is_exception_class(name)
                );
                if has_monitor_field {
                    Ok(quote! {
                        {
                            let __sync_arc = (#mon_ts).__monitor.pair();
                            let (__sync_lock, __sync_cond) = &*__sync_arc;
                            let mut __sync_guard = __sync_lock.lock().unwrap();
                            let _ = &__sync_guard;
                            { #(#body_ts)* }
                            drop(__sync_guard);
                        }
                    })
                } else {
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
///
/// The `exception_hierarchy` map provides `child → parent` info for user-defined
/// exception classes so that `catch (AppException e)` catches `DetailedException`
/// (which extends AppException).
fn emit_catch_chain(
    catches: &[ir::stmt::CatchClause],
    exception_hierarchy: &std::collections::HashMap<String, String>,
) -> Result<TokenStream, CodegenError> {
    // Build a helper: given a class name, return all ancestor exception names
    // (including the class itself) that would satisfy `class instanceof check`.
    let ancestors_of = |name: &str| -> std::collections::HashSet<String> {
        let mut set = std::collections::HashSet::new();
        set.insert(name.to_owned());
        let mut cur = name.to_owned();
        let mut has_runtime_exception = name == "RuntimeException";
        while let Some(parent) = exception_hierarchy.get(&cur) {
            if parent == "RuntimeException" {
                has_runtime_exception = true;
            }
            set.insert(parent.clone());
            cur = parent.clone();
        }
        // Always include the standard base names.
        set.insert("Exception".to_owned());
        set.insert("Throwable".to_owned());
        if has_runtime_exception {
            set.insert("RuntimeException".to_owned());
        }
        set
    };

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
            // For each catch type, build a check that also matches user exception subclasses.
            // If ExType is a user-defined exception, we also check all its subtypes
            // (i.e. any ex class whose ancestor chain includes ExType).
            let checks: Vec<TokenStream> = catch
                .exception_types
                .iter()
                .flat_map(|t| {
                    // Collect all user-defined exception classes that are subtypes of t.
                    let subtypes: Vec<String> = exception_hierarchy
                        .iter()
                        .filter(|(child, _)| ancestors_of(child.as_str()).contains(t.as_str()))
                        .map(|(child, _)| child.clone())
                        .collect();
                    let mut all_names = vec![t.clone()];
                    all_names.extend(subtypes);
                    all_names
                        .into_iter()
                        .map(|name| {
                            let name_str = name.as_str().to_owned();
                            quote! { __ex.is_instance_of(#name_str) }
                        })
                        .collect::<Vec<_>>()
                })
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
            // OuterClass.this.field = x → self.__outer.borrow_mut().field = x
            if let IrExpr::FieldAccess {
                receiver: deep_recv,
                field_name: this_kw,
                ..
            } = receiver.as_ref()
            {
                if this_kw == "this" {
                    if let IrExpr::Var {
                        name: outer_class_name,
                        ..
                    } = deep_recv.as_ref()
                    {
                        let current = CURRENT_CLASS_NAME.with(|c| c.borrow().clone());
                        let our_outer =
                            INNER_CLASS_OUTERS.with(|m| m.borrow().get(&current).cloned());
                        if our_outer.as_deref() == Some(outer_class_name.as_str()) {
                            let fname = ident(field_name);
                            return Ok(quote! { self.__outer.borrow_mut().#fname });
                        }
                    }
                }
            }
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
            // Check if this variable refers to a mutable static primitive field (Atomic*).
            let static_mangled =
                STATIC_ATOMIC_FIELDS.with(|sf| sf.borrow().get(name.as_str()).cloned());
            if let Some(mangled) = static_mangled {
                let mid = ident(&mangled);
                return Ok(quote! { #mid.load(::std::sync::atomic::Ordering::SeqCst) });
            }
            // Check if this variable refers to a mutable static reference field (OnceLock).
            // Reads use `get_or_init` so that a field which has not yet been written
            // (e.g., before the static initializer has run) returns the type's default
            // value rather than panicking — analogous to Java's null / zero default.
            let once_lock_mangled =
                STATIC_ONCE_LOCK_FIELDS.with(|sf| sf.borrow().get(name.as_str()).cloned());
            if let Some(mangled) = once_lock_mangled {
                let mid = ident(&mangled);
                return Ok(quote! {
                    #mid.get_or_init(|| ::std::default::Default::default()).clone()
                });
            }
            // Check if this variable is a captured outer-scope var in an anonymous class.
            let is_captured = ANON_CAPTURES.with(|ac| ac.borrow().contains(name));
            if is_captured {
                let cap_field = ident(&format!("__cap_{name}"));
                return Ok(quote! { self.#cap_field.clone() });
            }
            let id = ident(name);
            Ok(quote! { #id })
        }

        IrExpr::FieldAccess {
            receiver,
            field_name,
            ty,
        } => {
            // OuterClass.this.field → self.__outer.borrow().field
            // Detect the nested FieldAccess pattern: receiver is itself a
            // FieldAccess with field_name == "this" whose receiver is Var(outer_name).
            if let IrExpr::FieldAccess {
                receiver: deep_recv,
                field_name: this_kw,
                ..
            } = receiver.as_ref()
            {
                if this_kw == "this" {
                    if let IrExpr::Var {
                        name: outer_class_name,
                        ..
                    } = deep_recv.as_ref()
                    {
                        let current = CURRENT_CLASS_NAME.with(|c| c.borrow().clone());
                        let our_outer =
                            INNER_CLASS_OUTERS.with(|m| m.borrow().get(&current).cloned());
                        if our_outer.as_deref() == Some(outer_class_name.as_str()) {
                            let fname = ident(field_name);
                            let outer_field = match ty {
                                IrType::String
                                | IrType::Class(_)
                                | IrType::TypeVar(_)
                                | IrType::Generic { .. }
                                | IrType::Array(_) => {
                                    quote! { self.__outer.borrow().#fname.clone() }
                                }
                                _ => quote! { self.__outer.borrow().#fname },
                            };
                            return Ok(outer_field);
                        }
                    }
                }
            }

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
                    // Integer / Long / Double / Float static constants.
                    "Integer" => {
                        return match field_name.as_str() {
                            "MAX_VALUE" => Ok(quote! { i32::MAX }),
                            "MIN_VALUE" => Ok(quote! { i32::MIN }),
                            "SIZE" => Ok(quote! { 32i32 }),
                            "BYTES" => Ok(quote! { 4i32 }),
                            _ => Err(CodegenError::Unsupported(format!(
                                "Unsupported Integer static field: {field_name}"
                            ))),
                        };
                    }
                    "Long" => {
                        return match field_name.as_str() {
                            "MAX_VALUE" => Ok(quote! { i64::MAX }),
                            "MIN_VALUE" => Ok(quote! { i64::MIN }),
                            _ => Err(CodegenError::Unsupported(format!(
                                "Unsupported Long static field: {field_name}"
                            ))),
                        };
                    }
                    "Double" => {
                        return match field_name.as_str() {
                            "MAX_VALUE" => Ok(quote! { f64::MAX }),
                            "MIN_VALUE" => Ok(quote! { f64::MIN_POSITIVE }),
                            "NaN" => Ok(quote! { f64::NAN }),
                            "POSITIVE_INFINITY" => Ok(quote! { f64::INFINITY }),
                            "NEGATIVE_INFINITY" => Ok(quote! { f64::NEG_INFINITY }),
                            _ => Err(CodegenError::Unsupported(format!(
                                "Unsupported Double static field: {field_name}"
                            ))),
                        };
                    }
                    "Float" => {
                        return match field_name.as_str() {
                            "MAX_VALUE" => Ok(quote! { f32::MAX }),
                            "MIN_VALUE" => Ok(quote! { f32::MIN_POSITIVE }),
                            _ => Err(CodegenError::Unsupported(format!(
                                "Unsupported Float static field: {field_name}"
                            ))),
                        };
                    }
                    // Math constants accessed as field accesses.
                    "Math" => {
                        return match field_name.as_str() {
                            "PI" => Ok(quote! { std::f64::consts::PI }),
                            "E" => Ok(quote! { std::f64::consts::E }),
                            "TAU" => Ok(quote! { std::f64::consts::TAU }),
                            _ => Err(CodegenError::Unsupported(format!(
                                "Unsupported Math static field: {field_name}"
                            ))),
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
            // Cross-class static field read: ClassName.field → mangled_static.load()
            if let IrExpr::Var {
                name: class_name, ..
            } = receiver.as_ref()
            {
                let key = format!("{}::{}", class_name, field_name);
                let global_atomic = GLOBAL_STATIC_ATOMIC.with(|m| m.borrow().get(&key).cloned());
                if let Some(mangled) = global_atomic {
                    let mid = ident(&mangled);
                    return Ok(quote! { #mid.load(::std::sync::atomic::Ordering::SeqCst) });
                }
                let global_once = GLOBAL_STATIC_ONCE_LOCK.with(|m| m.borrow().get(&key).cloned());
                if let Some(mangled) = global_once {
                    let mid = ident(&mangled);
                    return Ok(quote! {
                        #mid.get_or_init(|| ::std::default::Default::default()).clone()
                    });
                }
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
            let mut args_ts: Vec<TokenStream> =
                args.iter().map(emit_expr).collect::<Result<_, _>>()?;

            // OuterClass.this.method(args) → self.__outer.borrow_mut().method(args)
            // Detect: receiver == Some(FieldAccess { receiver: Var(outer_name), field_name: "this" })
            if let Some(recv_expr) = receiver {
                if let IrExpr::FieldAccess {
                    receiver: deep_recv,
                    field_name: this_kw,
                    ..
                } = recv_expr.as_ref()
                {
                    if this_kw == "this" {
                        if let IrExpr::Var {
                            name: outer_class_name,
                            ..
                        } = deep_recv.as_ref()
                        {
                            let current = CURRENT_CLASS_NAME.with(|c| c.borrow().clone());
                            let our_outer =
                                INNER_CLASS_OUTERS.with(|m| m.borrow().get(&current).cloned());
                            if our_outer.as_deref() == Some(outer_class_name.as_str()) {
                                let mname = ident(method_name);
                                return Ok(
                                    quote! { self.__outer.borrow_mut().#mname(#(#args_ts),*) },
                                );
                            }
                        }
                    }
                }
            }

            // Varargs wrapping: if this method has a varargs parameter, bundle
            // the trailing arguments on the call site into a JArray::from_vec.
            // Look up by "ReceiverClass::method_name" first; fall back to
            // "CurrentClass::method_name" for unresolved or static calls.
            {
                let va_info = {
                    // Try to resolve receiver class from its IR type.
                    let receiver_class: Option<String> = receiver.as_ref().and_then(|r| {
                        if let IrType::Class(name) = r.ty() {
                            Some(name.clone())
                        } else {
                            None
                        }
                    });
                    let current_class = CURRENT_CLASS_NAME.with(|cn| cn.borrow().clone());
                    VARARGS_METHODS.with(|vm| {
                        let map = vm.borrow();
                        // 1. Try resolved receiver class
                        if let Some(cls) = &receiver_class {
                            let key = format!("{}::{}", cls, method_name);
                            if let Some(v) = map.get(key.as_str()).copied() {
                                return Some(v);
                            }
                        }
                        // 2. Try current class (covers static calls and
                        //    cases where receiver type is not resolved)
                        let key = format!("{}::{}", current_class, method_name);
                        map.get(key.as_str()).copied()
                    })
                };
                if let Some(non_va_count) = va_info {
                    // Only wrap if we have more than just a single JArray arg
                    // (otherwise assume an array is already being passed through).
                    let should_wrap = !(args_ts.len() == non_va_count + 1
                        && matches!(
                            args.get(non_va_count).map(|a| a.ty()),
                            Some(IrType::Array(_))
                        ));
                    if should_wrap {
                        let regular: Vec<TokenStream> =
                            args_ts.drain(..non_va_count.min(args_ts.len())).collect();
                        let va: Vec<TokenStream> = std::mem::take(&mut args_ts);
                        let mut new_args = regular;
                        new_args.push(quote! { JArray::from_vec(vec![#(#va),*]) });
                        args_ts = new_args;
                    }
                }
            }

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

                    // ForkJoinPool.commonPool() static call
                    if name == "ForkJoinPool" {
                        return match method_name.as_str() {
                            "commonPool" => Ok(quote! { JForkJoinPool::commonPool() }),
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JForkJoinPool::#m(#(#args_ts),*) })
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
                            "E" => Ok(quote! { std::f64::consts::E }),
                            "signum" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as f64).signum() })
                            }
                            "hypot" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { (#a as f64).hypot(#b as f64) })
                            }
                            "atan2" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { (#a as f64).atan2(#b as f64) })
                            }
                            "asin" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as f64).asin() })
                            }
                            "acos" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as f64).acos() })
                            }
                            "atan" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as f64).atan() })
                            }
                            "sinh" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as f64).sinh() })
                            }
                            "cosh" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as f64).cosh() })
                            }
                            "tanh" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as f64).tanh() })
                            }
                            "toDegrees" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as f64).to_degrees() })
                            }
                            "toRadians" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as f64).to_radians() })
                            }
                            "cbrt" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as f64).cbrt() })
                            }
                            "copySign" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { (#a as f64).copysign(#b as f64) })
                            }
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
                    // ZonedDateTime static methods
                    if name == "ZonedDateTime" {
                        return match method_name.as_str() {
                            "of" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { JZonedDateTime::of(#a, #b) })
                            }
                            "now" => {
                                if args_ts.is_empty() {
                                    Ok(quote! { JZonedDateTime::now() })
                                } else {
                                    let a = &args_ts[0];
                                    Ok(quote! { JZonedDateTime::now_zone(&#a) })
                                }
                            }
                            "parse" => {
                                let a = &args_ts[0];
                                Ok(quote! { JZonedDateTime::parse(&#a) })
                            }
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JZonedDateTime::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // ZoneId static methods
                    if name == "ZoneId" {
                        return match method_name.as_str() {
                            "of" => {
                                let a = &args_ts[0];
                                Ok(quote! { JZoneId::of(&#a) })
                            }
                            "systemDefault" => Ok(quote! { JZoneId::systemDefault() }),
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JZoneId::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // Clock static methods
                    if name == "Clock" {
                        return match method_name.as_str() {
                            "systemUTC" => Ok(quote! { JClock::systemUTC() }),
                            "systemDefaultZone" => Ok(quote! { JClock::systemDefaultZone() }),
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JClock::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // HttpClient static methods
                    if name == "HttpClient" {
                        return match method_name.as_str() {
                            "newHttpClient" => Ok(quote! { JHttpClient::new() }),
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JHttpClient::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // HttpRequest static methods (builder factory)
                    if name == "HttpRequest" {
                        return match method_name.as_str() {
                            "newBuilder" => Ok(quote! { JHttpRequestBuilder::new() }),
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JHttpRequest::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // URI.create(String) → JURL::new(s)
                    if name == "URI" {
                        return match method_name.as_str() {
                            "create" => {
                                let a = &args_ts[0];
                                Ok(quote! { JURL::new(#a) })
                            }
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JURL::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // Integer.parseInt / Integer.valueOf / Integer.toBinaryString / etc.
                    if name == "Integer" {
                        return match method_name.as_str() {
                            "parseInt" | "valueOf" => {
                                if args_ts.len() == 2 {
                                    // parseInt(s, radix)
                                    let a = &args_ts[0];
                                    let b = &args_ts[1];
                                    Ok(quote! {{
                                        let __s_owned = format!("{}", #a);
                                        let __s = __s_owned.as_str();
                                        let __radix_i32 = #b as i32;
                                        if !(2..=36).contains(&__radix_i32) {
                                            panic!(
                                                "java.lang.NumberFormatException: For input string: \"{}\" under radix {}",
                                                __s,
                                                __radix_i32
                                            );
                                        }
                                        let __radix = __radix_i32 as u32;
                                        i32::from_str_radix(__s, __radix).unwrap_or_else(|_| {
                                            panic!(
                                                "java.lang.NumberFormatException: For input string: \"{}\" under radix {}",
                                                __s,
                                                __radix_i32
                                            )
                                        })
                                    }})
                                } else {
                                    let a = &args_ts[0];
                                    Ok(quote! {{
                                        let __s_owned = format!("{}", #a);
                                        let __s = __s_owned.as_str();
                                        __s.parse::<i32>().unwrap_or_else(|_| {
                                            panic!(
                                                "java.lang.NumberFormatException: For input string: \"{}\"",
                                                __s
                                            )
                                        })
                                    }})
                                }
                            }
                            "toString" => {
                                let a = &args_ts[0];
                                Ok(quote! { JString::from(format!("{}", #a as i32).as_str()) })
                            }
                            "toBinaryString" => {
                                let a = &args_ts[0];
                                Ok(quote! { JString::from(format!("{:b}", #a as i32).as_str()) })
                            }
                            "toHexString" => {
                                let a = &args_ts[0];
                                Ok(quote! { JString::from(format!("{:x}", #a as i32).as_str()) })
                            }
                            "toOctalString" => {
                                let a = &args_ts[0];
                                Ok(quote! { JString::from(format!("{:o}", #a as i32).as_str()) })
                            }
                            "bitCount" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as i32).count_ones() as i32 })
                            }
                            "highestOneBit" => {
                                let a = &args_ts[0];
                                Ok(
                                    quote! { { let __n = #a as i32; if __n == 0 { 0i32 } else { 1i32 << (31 - __n.leading_zeros()) } } },
                                )
                            }
                            "numberOfLeadingZeros" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as i32).leading_zeros() as i32 })
                            }
                            "numberOfTrailingZeros" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as i32).trailing_zeros() as i32 })
                            }
                            "compare" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! {
                                    match (#a as i32).cmp(&(#b as i32)) {
                                        std::cmp::Ordering::Less => -1i32,
                                        std::cmp::Ordering::Equal => 0i32,
                                        std::cmp::Ordering::Greater => 1i32,
                                    }
                                })
                            }
                            "signum" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as i32).signum() })
                            }
                            "max" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { (#a as i32).max(#b as i32) })
                            }
                            "min" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { (#a as i32).min(#b as i32) })
                            }
                            "sum" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { (#a as i32).wrapping_add(#b as i32) })
                            }
                            _ => Ok(quote! { 0i32 }),
                        };
                    }
                    // Long.parseLong / Long.toBinaryString / etc.
                    if name == "Long" {
                        return match method_name.as_str() {
                            "parseLong" | "valueOf" => {
                                let a = &args_ts[0];
                                Ok(quote! {{
                                    let __s_owned = format!("{}", #a);
                                    let __s = __s_owned.as_str();
                                    __s.parse::<i64>().unwrap_or_else(|_| {
                                        panic!(
                                            "java.lang.NumberFormatException: For input string: \"{}\"",
                                            __s
                                        )
                                    })
                                }})
                            }
                            "toString" => {
                                let a = &args_ts[0];
                                Ok(quote! { JString::from(format!("{}", #a as i64).as_str()) })
                            }
                            "toBinaryString" => {
                                let a = &args_ts[0];
                                Ok(quote! { JString::from(format!("{:b}", #a as i64).as_str()) })
                            }
                            "toHexString" => {
                                let a = &args_ts[0];
                                Ok(quote! { JString::from(format!("{:x}", #a as i64).as_str()) })
                            }
                            "toOctalString" => {
                                let a = &args_ts[0];
                                Ok(quote! { JString::from(format!("{:o}", #a as i64).as_str()) })
                            }
                            "bitCount" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as i64).count_ones() as i32 })
                            }
                            "compare" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! {
                                    match (#a as i64).cmp(&(#b as i64)) {
                                        std::cmp::Ordering::Less => -1i32,
                                        std::cmp::Ordering::Equal => 0i32,
                                        std::cmp::Ordering::Greater => 1i32,
                                    }
                                })
                            }
                            "max" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { (#a as i64).max(#b as i64) })
                            }
                            "min" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { (#a as i64).min(#b as i64) })
                            }
                            _ => Ok(quote! { 0i64 }),
                        };
                    }
                    // Double.parseDouble / Double.isNaN / etc.
                    if name == "Double" {
                        return match method_name.as_str() {
                            "parseDouble" | "valueOf" => {
                                let a = &args_ts[0];
                                Ok(quote! {{
                                    let __s_owned = format!("{}", #a);
                                    let __s = __s_owned.as_str();
                                    __s.parse::<f64>().unwrap_or_else(|_| {
                                        panic!(
                                            "java.lang.NumberFormatException: For input string: \"{}\"",
                                            __s
                                        )
                                    })
                                }})
                            }
                            "toString" => {
                                let a = &args_ts[0];
                                Ok(quote! { JString::from(format!("{}", #a as f64).as_str()) })
                            }
                            "isNaN" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as f64).is_nan() })
                            }
                            "isInfinite" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a as f64).is_infinite() })
                            }
                            "compare" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! {{
                                    let __a = #a as f64;
                                    let __b = #b as f64;
                                    if __a < __b {
                                        -1i32
                                    } else if __a > __b {
                                        1i32
                                    } else if __a.is_nan() {
                                        if __b.is_nan() { 0i32 } else { 1i32 }
                                    } else if __b.is_nan() {
                                        -1i32
                                    } else if __a == 0.0f64 && __b == 0.0f64 {
                                        if __a.is_sign_negative() == __b.is_sign_negative() {
                                            0i32
                                        } else if __a.is_sign_negative() {
                                            -1i32
                                        } else {
                                            1i32
                                        }
                                    } else {
                                        0i32
                                    }
                                }})
                            }
                            "max" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { (#a as f64).max(#b as f64) })
                            }
                            "min" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { (#a as f64).min(#b as f64) })
                            }
                            _ => Ok(quote! { 0.0f64 }),
                        };
                    }
                    // Character.isDigit / Character.isLetter / etc.
                    if name == "Character" {
                        return match method_name.as_str() {
                            "isDigit" => {
                                let c = &args_ts[0];
                                Ok(quote! { (#c).is_numeric() })
                            }
                            "isLetter" | "isAlphabetic" => {
                                let c = &args_ts[0];
                                Ok(quote! { (#c).is_alphabetic() })
                            }
                            "isLetterOrDigit" => {
                                let c = &args_ts[0];
                                Ok(quote! { (#c).is_alphanumeric() })
                            }
                            "isWhitespace" | "isSpaceChar" => {
                                let c = &args_ts[0];
                                Ok(quote! { (#c).is_whitespace() })
                            }
                            "isUpperCase" => {
                                let c = &args_ts[0];
                                Ok(quote! { (#c).is_uppercase() })
                            }
                            "isLowerCase" => {
                                let c = &args_ts[0];
                                Ok(quote! { (#c).is_lowercase() })
                            }
                            "toUpperCase" => {
                                let c = &args_ts[0];
                                Ok(quote! { (#c).to_uppercase().next().unwrap_or(#c) })
                            }
                            "toLowerCase" => {
                                let c = &args_ts[0];
                                Ok(quote! { (#c).to_lowercase().next().unwrap_or(#c) })
                            }
                            "getNumericValue" => {
                                let c = &args_ts[0];
                                Ok(quote! { (#c).to_digit(10).map(|d| d as i32).unwrap_or(-1) })
                            }
                            "digit" => {
                                let c = &args_ts[0];
                                let radix = &args_ts[1];
                                Ok(
                                    quote! { (#c).to_digit(#radix as u32).map(|d| d as i32).unwrap_or(-1) },
                                )
                            }
                            "forDigit" => {
                                let d = &args_ts[0];
                                let radix = &args_ts[1];
                                Ok(
                                    quote! { char::from_digit(#d as u32, #radix as u32).unwrap_or('\0') },
                                )
                            }
                            "toString" => {
                                let c = &args_ts[0];
                                Ok(quote! { JString::from(format!("{}", #c).as_str()) })
                            }
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { #m(#(#args_ts),*) })
                            }
                        };
                    }
                    // Objects.requireNonNull / Objects.isNull / Objects.equals / etc.
                    if name == "Objects" {
                        let a_is_null_literal = args
                            .first()
                            .map(|arg| matches!(arg.ty(), IrType::Null))
                            .unwrap_or(false);
                        let a_is_nullable = args
                            .first()
                            .map(|arg| matches!(arg.ty(), IrType::Nullable(_) | IrType::Null))
                            .unwrap_or(false);
                        let b_is_nullable = args
                            .get(1)
                            .map(|arg| matches!(arg.ty(), IrType::Nullable(_) | IrType::Null))
                            .unwrap_or(false);
                        return match method_name.as_str() {
                            "requireNonNull" => {
                                let a = &args_ts[0];
                                if a_is_null_literal {
                                    Ok(quote! { panic!("java.lang.NullPointerException") })
                                } else if a_is_nullable {
                                    Ok(quote! {
                                        (#a).unwrap_or_else(|| panic!("java.lang.NullPointerException"))
                                    })
                                } else {
                                    Ok(quote! { #a })
                                }
                            }
                            "requireNonNullElse" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                if a_is_null_literal && b_is_nullable {
                                    Ok(quote! {
                                        (#b).unwrap_or_else(|| panic!("java.lang.NullPointerException"))
                                    })
                                } else if a_is_null_literal {
                                    Ok(quote! { #b })
                                } else if a_is_nullable && b_is_nullable {
                                    Ok(quote! {
                                        (#a).or(#b).unwrap_or_else(|| panic!("java.lang.NullPointerException"))
                                    })
                                } else if a_is_nullable {
                                    Ok(quote! { (#a).unwrap_or(#b) })
                                } else {
                                    Ok(quote! { #a })
                                }
                            }
                            "isNull" => {
                                let a = &args_ts[0];
                                if a_is_null_literal {
                                    Ok(quote! { true })
                                } else if a_is_nullable {
                                    Ok(quote! { (#a).is_none() })
                                } else {
                                    Ok(quote! { false })
                                }
                            }
                            "nonNull" => {
                                let a = &args_ts[0];
                                if a_is_null_literal {
                                    Ok(quote! { false })
                                } else if a_is_nullable {
                                    Ok(quote! { (#a).is_some() })
                                } else {
                                    Ok(quote! { true })
                                }
                            }
                            "equals" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { (#a == #b) })
                            }
                            "deepEquals" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { (#a == #b) })
                            }
                            "hash" | "hashCode" => {
                                // Simplified: hash all args together naively.
                                Ok(quote! { {
                                    let mut __h = 1i32;
                                    #(let __h = __h.wrapping_mul(31).wrapping_add(format!("{:?}", &#args_ts).len() as i32);)*
                                    __h
                                } })
                            }
                            "toString" if args_ts.len() == 1 => {
                                let a = &args_ts[0];
                                if a_is_null_literal {
                                    Ok(quote! { JString::from("null") })
                                } else if a_is_nullable {
                                    Ok(quote! {
                                        match #a {
                                            Some(__v) => JString::from(format!("{}", __v).as_str()),
                                            None => JString::from("null"),
                                        }
                                    })
                                } else {
                                    Ok(quote! { JString::from(format!("{}", #a).as_str()) })
                                }
                            }
                            "toString" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                if a_is_null_literal {
                                    Ok(quote! { JString::from(format!("{}", #b).as_str()) })
                                } else if a_is_nullable {
                                    Ok(quote! {
                                        match #a {
                                            Some(__v) => JString::from(format!("{}", __v).as_str()),
                                            None => JString::from(format!("{}", #b).as_str()),
                                        }
                                    })
                                } else {
                                    Ok(quote! { JString::from(format!("{}", #a).as_str()) })
                                }
                            }
                            "compare" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                let cmp = &args_ts[2];
                                Ok(quote! { #cmp(&#a, &#b) })
                            }
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { #m(#(#args_ts),*) })
                            }
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
                    // Collections.sort / Collections.reverse / Collections.min / etc.
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
                            "min" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a).min_element() })
                            }
                            "max" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a).max_element() })
                            }
                            "frequency" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { (#a).frequency(#b) })
                            }
                            "nCopies" => {
                                let n = &args_ts[0];
                                let v = &args_ts[1];
                                Ok(quote! { JList::n_copies(#n, #v) })
                            }
                            "fill" => {
                                let a = &args_ts[0];
                                let v = &args_ts[1];
                                Ok(quote! { (#a).fill_all(#v) })
                            }
                            "swap" => {
                                let a = &args_ts[0];
                                let i = &args_ts[1];
                                let j = &args_ts[2];
                                Ok(quote! { (#a).swap(#i, #j) })
                            }
                            "disjoint" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { (#a).disjoint(&#b) })
                            }
                            "binarySearch" => {
                                let a = &args_ts[0];
                                let k = &args_ts[1];
                                Ok(quote! { (#a).binary_search_val(#k) })
                            }
                            "shuffle" => {
                                // shuffle is a no-op in deterministic translation
                                let a = &args_ts[0];
                                Ok(quote! { { let _ = &#a; } })
                            }
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { java_compat::collections_util::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // Arrays.asList / Arrays.sort / Arrays.fill / etc.
                    if name == "Arrays" {
                        return match method_name.as_str() {
                            "asList" => Ok(quote! { {
                                let mut __list = JList::new();
                                #( __list.add(#args_ts); )*
                                __list
                            } }),
                            "sort" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a).sort_in_place() })
                            }
                            "fill" => {
                                let a = &args_ts[0];
                                let v = &args_ts[1];
                                Ok(quote! { (#a).fill(#v) })
                            }
                            "stream" => {
                                let a = &args_ts[0];
                                Ok(quote! { JStream::from_array(&#a) })
                            }
                            "toString" => {
                                let a = &args_ts[0];
                                Ok(quote! { (#a).to_display_string() })
                            }
                            "copyOfRange" => {
                                let a = &args_ts[0];
                                let from = &args_ts[1];
                                let to = &args_ts[2];
                                Ok(quote! { (#a).copy_of_range(#from, #to) })
                            }
                            "copyOf" => {
                                let a = &args_ts[0];
                                let n = &args_ts[1];
                                Ok(quote! { (#a).copy_of_length(#n) })
                            }
                            "binarySearch" => {
                                let a = &args_ts[0];
                                let k = &args_ts[1];
                                Ok(quote! { (#a).binary_search_val(#k) })
                            }
                            "equals" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { (#a).elements_equal(&#b) })
                            }
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { #m(#(#args_ts),*) })
                            }
                        };
                    }
                    // Stream.of(...) / Stream.empty()
                    if name == "Stream" {
                        return match method_name.as_str() {
                            "of" => Ok(quote! { JStream::new(vec![#(#args_ts),*]) }),
                            "empty" => Ok(quote! { JStream::new(vec![]) }),
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JStream::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // IntStream.range / IntStream.rangeClosed / IntStream.of
                    if name == "IntStream" {
                        return match method_name.as_str() {
                            "range" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { JStream::<i32>::int_range(#a, #b) })
                            }
                            "rangeClosed" => {
                                let a = &args_ts[0];
                                let b = &args_ts[1];
                                Ok(quote! { JStream::<i32>::int_range_closed(#a, #b) })
                            }
                            "of" => Ok(quote! { JStream::new(vec![#(#args_ts),*]) }),
                            _ => {
                                let m = ident(method_name);
                                Ok(quote! { JStream::<i32>::#m(#(#args_ts),*) })
                            }
                        };
                    }
                    // EnumSet.noneOf(...) / EnumSet.of(...) / EnumSet.allOf(...)
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
                    // ResourceBundle.getBundle(name)
                    if name == "ResourceBundle" && method_name == "getBundle" {
                        let a = args_ts
                            .first()
                            .cloned()
                            .unwrap_or_else(|| quote! { JString::from("") });
                        return Ok(quote! { JResourceBundle::get_bundle(#a) });
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

                // `.collect(Collectors.*)` dispatch
                if method_name == "collect" {
                    if let Some(arg) = args.first() {
                        let recv_ts = emit_expr(recv)?;
                        if is_collectors_to_list(arg) {
                            return Ok(quote! { (#recv_ts).collect_to_list() });
                        }
                        if is_collectors_method(arg, "toSet") {
                            return Ok(quote! { (#recv_ts).collect_to_set() });
                        }
                        if is_collectors_method(arg, "toUnmodifiableList") {
                            return Ok(quote! { (#recv_ts).collect_to_list() });
                        }
                        if is_collectors_method(arg, "counting") {
                            return Ok(quote! { (#recv_ts).count() });
                        }
                        // Collectors.joining() variants
                        if let Some(join_args_result) = collectors_joining_args(arg) {
                            let join_args = join_args_result?;
                            match join_args.len() {
                                0 => {
                                    return Ok(
                                        quote! { (#recv_ts).collect_joining(JString::from("")) },
                                    );
                                }
                                1 => {
                                    let sep = &join_args[0];
                                    return Ok(quote! { (#recv_ts).collect_joining(#sep) });
                                }
                                3 => {
                                    let sep = &join_args[0];
                                    let pre = &join_args[1];
                                    let suf = &join_args[2];
                                    return Ok(
                                        quote! { (#recv_ts).collect_joining_full(#sep, #pre, #suf) },
                                    );
                                }
                                n => {
                                    return Err(CodegenError::Unsupported(format!(
                                        "Collectors.joining() does not support {n} arguments; only 0, 1, or 3 arguments are supported"
                                    )));
                                }
                            }
                        }
                        // Collectors.toMap(keyFn, valFn)
                        if let Some(map_args_result) = collectors_two_fn_args(arg, "toMap") {
                            let map_args = map_args_result?;
                            let kf = &map_args[0];
                            let vf = &map_args[1];
                            return Ok(quote! { (#recv_ts).collect_to_map(#kf, #vf) });
                        }
                        // Collectors.groupingBy(classifier)
                        if let Some(gb_args_result) = collectors_one_fn_arg(arg, "groupingBy") {
                            let gb_args = gb_args_result?;
                            let clf = &gb_args[0];
                            return Ok(quote! { (#recv_ts).collect_grouping_by(#clf) });
                        }
                    }
                }

                let recv_ts = emit_expr(recv)?;

                // ForkJoinPool.invoke(task) → (task).compute()
                // When the argument is a simple variable reference, call compute()
                // directly via auto-borrow (&mut self) to avoid consuming the task
                // via a move.  For complex expressions a local binding is still
                // needed.
                if method_name == "invoke"
                    && args_ts.len() == 1
                    && type_name_matches(recv.ty(), "ForkJoinPool")
                {
                    let task = &args_ts[0];
                    if matches!(args.first(), Some(IrExpr::Var { .. })) {
                        return Ok(quote! { (#task).compute() });
                    }
                    return Ok(quote! { { let mut __fjp_t = #task; __fjp_t.compute() } });
                }

                // Handle method overloads that need special dispatch.
                if method_name == "substring" && args_ts.len() == 2 {
                    let a = &args_ts[0];
                    let b = &args_ts[1];
                    return Ok(quote! { (#recv_ts).substring_range(#a, #b) });
                }

                // getBytes() / getBytes("UTF-8") → getBytes() (charset ignored)
                if method_name == "getBytes" {
                    return Ok(quote! { (#recv_ts).getBytes() });
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

                // Properties instance methods
                if type_name_matches(recv.ty(), "Properties") {
                    match method_name.as_str() {
                        "getProperty" if args_ts.len() == 1 => {
                            let k = &args_ts[0];
                            return Ok(quote! { (#recv_ts).getProperty(&#k) });
                        }
                        "getProperty" if args_ts.len() == 2 => {
                            let k = &args_ts[0];
                            let d = &args_ts[1];
                            return Ok(quote! { (#recv_ts).getProperty_default(&#k, &#d) });
                        }
                        "setProperty" if args_ts.len() == 2 => {
                            let k = &args_ts[0];
                            let v = &args_ts[1];
                            return Ok(quote! { (#recv_ts).setProperty(&#k, &#v) });
                        }
                        "containsKey" if args_ts.len() == 1 => {
                            let k = &args_ts[0];
                            return Ok(quote! { (#recv_ts).containsKey(&#k) });
                        }
                        "load" if args_ts.len() == 1 => {
                            // load(new StringReader(s)) or load(reader) — extract string content
                            // Try to detect new StringReader(...) wrapping
                            let inner = args.first().unwrap();
                            let content_ts = if let IrExpr::New {
                                class,
                                args: inner_args,
                                ..
                            } = inner
                            {
                                if class == "StringReader" && !inner_args.is_empty() {
                                    emit_expr(&inner_args[0])?
                                } else {
                                    args_ts[0].clone()
                                }
                            } else {
                                args_ts[0].clone()
                            };
                            return Ok(quote! { (#recv_ts).load_string(&#content_ts) });
                        }
                        _ => {}
                    }
                }

                // Timer instance methods — schedule / cancel / purge
                if type_name_matches(recv.ty(), "Timer") {
                    match method_name.as_str() {
                        "cancel" => {
                            return Ok(quote! { (#recv_ts).cancel() });
                        }
                        "purge" => {
                            return Ok(quote! { (#recv_ts).purge() });
                        }
                        "schedule" if args_ts.len() == 2 => {
                            // schedule(task, delayMs) — one-shot
                            let task = &args_ts[0];
                            let delay = &args_ts[1];
                            return Ok(quote! { (#recv_ts).schedule_fn_once({
                                let mut __t = #task;
                                Box::new(move || { (__t).run(); })
                            }, #delay) });
                        }
                        "schedule" if args_ts.len() == 3 => {
                            // schedule(task, delayMs, periodMs) — repeating
                            let task = &args_ts[0];
                            let delay = &args_ts[1];
                            let period = &args_ts[2];
                            return Ok(quote! { (#recv_ts).schedule_fn({
                                let mut __t = #task;
                                Box::new(move || { (__t).run(); })
                            }, #delay, #period) });
                        }
                        _ => {}
                    }
                }

                // ZonedDateTime instance methods
                if type_name_matches(recv.ty(), "ZonedDateTime") {
                    match method_name.as_str() {
                        "isBefore" | "isAfter" | "isEqual" if args_ts.len() == 1 => {
                            let a = &args_ts[0];
                            let m = ident(method_name);
                            return Ok(quote! { (#recv_ts).#m(&#a) });
                        }
                        "format" if args_ts.len() == 1 => {
                            let a = &args_ts[0];
                            return Ok(quote! { (#recv_ts).format(&#a) });
                        }
                        "withZoneSameInstant" if args_ts.len() == 1 => {
                            let a = &args_ts[0];
                            return Ok(quote! { (#recv_ts).withZoneSameInstant(&#a) });
                        }
                        _ => {}
                    }
                }

                // Clock instance methods
                if type_name_matches(recv.ty(), "Clock") {
                    match method_name.as_str() {
                        "instant" => return Ok(quote! { (#recv_ts).instant() }),
                        "millis" => return Ok(quote! { (#recv_ts).millis() }),
                        "getZone" => return Ok(quote! { (#recv_ts).getZone() }),
                        _ => {}
                    }
                }

                // HttpRequestBuilder chain methods — pass-through (generic dispatch handles them)
                // HttpRequest instance methods
                if type_name_matches(recv.ty(), "HttpRequest") {
                    match method_name.as_str() {
                        "method" => return Ok(quote! { (#recv_ts).method() }),
                        "uri" => return Ok(quote! { (#recv_ts).uri() }),
                        _ => {}
                    }
                }

                // HttpClient.send(request, handler)
                if type_name_matches(recv.ty(), "HttpClient") && method_name == "send" {
                    let req = &args_ts[0];
                    return Ok(quote! { (#recv_ts).send(#req) });
                }

                // HttpResponse instance methods
                if type_name_matches(recv.ty(), "HttpResponse") {
                    match method_name.as_str() {
                        "statusCode" => return Ok(quote! { (#recv_ts).statusCode() }),
                        "body" => return Ok(quote! { (#recv_ts).body() }),
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

                // `compareTo` on a generic type variable (T: PartialOrd + Ord) →
                // delegate to Rust's `Ord::cmp()`, casting the Ordering to i32.
                // The receiver type is IrType::Class("T") (parser uses Class for all
                // non-primitive names), so we check against active type param names.
                if method_name == "compareTo" && args_ts.len() == 1 {
                    let recv_is_type_param = match recv.ty() {
                        IrType::Class(name) => {
                            CLASS_TYPE_PARAMS.with(|ctp| ctp.borrow().contains(name.as_str()))
                                || METHOD_TYPE_PARAMS
                                    .with(|mtp| mtp.borrow().contains(name.as_str()))
                        }
                        IrType::TypeVar(_) => true,
                        _ => false,
                    };
                    if recv_is_type_param {
                        let arg = &args_ts[0];
                        return Ok(quote! {
                            match (#recv_ts).cmp(&(#arg)) {
                                ::std::cmp::Ordering::Less => -1_i32,
                                ::std::cmp::Ordering::Equal => 0_i32,
                                ::std::cmp::Ordering::Greater => 1_i32,
                            }
                        });
                    }
                }

                // Rename Java method names to their Rust runtime equivalents.
                let method = match method_name.as_str() {
                    "await" => ident("await_"),
                    "mod" => ident("mod_"),
                    "charAt" => ident("char_at"),
                    "indexOf" => ident("index_of"),
                    "isEmpty" => ident("isEmpty"),
                    // Modern String API: map Java names to runtime method names.
                    "lines" => ident("lines_stream"),
                    "chars" => ident("chars_stream"),
                    "toCharArray" => ident("to_char_array"),
                    // Stream terminal ops with alternate Rust names (none needed — pass through).
                    // mapToInt/mapToLong/mapToDouble are type-changing maps; alias to map().
                    "mapToInt" | "mapToLong" | "mapToDouble" => ident("map"),
                    _ => ident(method_name),
                };
                // Per-object wait/notify: obj.wait() / obj.notify() / obj.notifyAll()
                // where `obj` is the monitor variable bound by the enclosing
                // synchronized(obj) block.  Route through the block's __sync_cond
                // instead of calling a (non-existent) instance method.
                if let IrExpr::Var {
                    name: recv_name, ..
                } = recv.as_ref()
                {
                    let sync_mon = SYNC_MONITOR_EXPR.with(|m| m.borrow().clone());
                    if let Some(ref mon_name) = sync_mon {
                        if recv_name == mon_name {
                            match method_name.as_str() {
                                "wait" => {
                                    return Ok(quote! {
                                        __sync_guard = __sync_cond.wait(__sync_guard).unwrap()
                                    });
                                }
                                "notifyAll" => {
                                    return Ok(quote! { __sync_cond.notify_all() });
                                }
                                "notify" => {
                                    return Ok(quote! { __sync_cond.notify_one() });
                                }
                                _ => {}
                            }
                        }
                    }
                }
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
                            Some(IrType::Class(ref c)) if c == "StringReader" => {
                                Ok(quote! { JBufferedReader::from_string_reader(#a) })
                            }
                            Some(IrType::Class(ref c)) if c == "Reader" => {
                                Ok(quote! { (#a).into_buffered_reader() })
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
                        let arg_ty = args.first().map(|e| e.ty());
                        match arg_ty {
                            Some(IrType::Class(ref c)) if c == "Writer" => {
                                Ok(quote! { (#a).into_buffered_writer() })
                            }
                            _ => Ok(quote! { JBufferedWriter::from_writer(#a) }),
                        }
                    }
                }
                "PrintWriter" => {
                    if args_ts.is_empty() {
                        Ok(quote! { JPrintWriter::default() })
                    } else {
                        let a = &args_ts[0];
                        // Check if arg is a FileWriter/File/StringWriter type or a String path
                        let arg_ty = args.first().map(|e| e.ty());
                        match arg_ty {
                            Some(IrType::Class(ref c)) if c == "FileWriter" => {
                                Ok(quote! { JPrintWriter::from_writer(#a) })
                            }
                            Some(IrType::Class(ref c)) if c == "File" => {
                                Ok(quote! { JPrintWriter::from_file(&#a) })
                            }
                            Some(IrType::Class(ref c)) if c == "StringWriter" => {
                                Ok(quote! { JPrintWriter::from_string_writer(&#a) })
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
                "Properties" => Ok(quote! { JProperties::new() }),
                "Timer" => Ok(quote! { JTimer::new() }),
                // Stage 13: StampedLock and ForkJoinPool constructors
                "StampedLock" => Ok(quote! { JStampedLock::new() }),
                "ForkJoinPool" => Ok(quote! { JForkJoinPool::new() }),
                "StringWriter" => Ok(quote! { JStringWriter::new() }),
                "StringReader" => {
                    let a = args_ts
                        .first()
                        .cloned()
                        .unwrap_or_else(|| quote! { JString::from("") });
                    Ok(quote! { JStringReader::new(#a) })
                }
                "ByteArrayOutputStream" => Ok(quote! { JByteArrayOutputStream::new() }),
                "ByteArrayInputStream" => {
                    let a = args_ts
                        .first()
                        .cloned()
                        .unwrap_or_else(|| quote! { JArray::<i8>::new_default(0) });
                    Ok(quote! { JByteArrayInputStream::new(#a) })
                }
                "ResourceBundle" | "PropertyResourceBundle" => {
                    if let Some(a) = args_ts.first() {
                        let arg_ty = args.first().map(|e| e.ty());
                        match arg_ty {
                            Some(IrType::Class(ref c))
                                if c == "InputStream" || c == "ByteArrayInputStream" =>
                            {
                                Ok(quote! { JResourceBundle::from_input_stream(#a) })
                            }
                            _ => Ok(quote! { JResourceBundle::get_bundle(#a) }),
                        }
                    } else {
                        Ok(quote! { JResourceBundle::default() })
                    }
                }
                _ => {
                    // Check if this refers to an inner/local class that was
                    // hoisted: "Counter" → "InnerClass$Counter", etc.
                    let mangled = INNER_CLASS_MAP
                        .with(|m| m.borrow().get(class.as_str()).cloned())
                        .unwrap_or_else(|| class.clone());
                    let cls_ident = ident(&mangled);
                    // For non-static inner classes inject the outer back-reference.
                    let is_inner = INNER_CLASS_OUTERS.with(|m| m.borrow().contains_key(&mangled));
                    if is_inner && !IN_STATIC_METHOD.with(|c| c.get()) {
                        Ok(quote! {
                            #cls_ident::new(
                                ::std::rc::Rc::new(::std::cell::RefCell::new(self.clone())),
                                #(#args_ts),*
                            )
                        })
                    } else if is_inner {
                        // Static context (shouldn't happen in valid Java, but
                        // fall back to a default outer to keep code compiling).
                        Ok(quote! {
                            #cls_ident::new(
                                ::std::rc::Rc::new(::std::cell::RefCell::new(Default::default())),
                                #(#args_ts),*
                            )
                        })
                    } else {
                        // Anonymous class: append captured variable clones.
                        let cap_args: Vec<TokenStream> = ANON_CAPTURE_MAP
                            .with(|m| m.borrow().get(class.as_str()).cloned())
                            .unwrap_or_default()
                            .iter()
                            .map(|cap_name| {
                                let cid = ident(cap_name);
                                quote! { #cid.clone() }
                            })
                            .collect();
                        let mut ctor_args = args_ts.clone();
                        ctor_args.extend(cap_args);
                        Ok(quote! { #cls_ident::new(#(#ctor_args),*) })
                    }
                }
            }
        }

        IrExpr::NewArray { elem_ty, len, .. } => {
            let len_ts = emit_expr(len)?;
            let elem_ts = emit_type(elem_ty);
            Ok(quote! { JArray::<#elem_ts>::new_default(#len_ts) })
        }

        IrExpr::NewArrayMultiDim { elem_ty, dims, .. } => {
            // Build nested allocation from the innermost dimension outward.
            // For `new int[r][c]`:
            //   JArray::<JArray<i32>>::new_with(r, |_| JArray::<i32>::new_default(c))
            let n = dims.len();
            if n == 0 {
                return Ok(quote! { JArray::<()>::new_default(0) });
            }
            let last_dim_ts = emit_expr(&dims[n - 1])?;
            let elem_ts = emit_type(elem_ty);
            let mut inner_expr = quote! { JArray::<#elem_ts>::new_default(#last_dim_ts) };
            let mut current_elem_ty = elem_ty.clone();
            for i in (0..n - 1).rev() {
                let dim_ts = emit_expr(&dims[i])?;
                let wrapped_ty = IrType::Array(Box::new(current_elem_ty.clone()));
                let wrapped_ts = emit_type(&wrapped_ty);
                inner_expr = quote! { JArray::<#wrapped_ts>::new_with(#dim_ts, |_| #inner_expr) };
                current_elem_ty = wrapped_ty;
            }
            Ok(inner_expr)
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
            // Check if the operand is a mutable static field; if so use atomics.
            if let IrExpr::Var { name, .. } = operand.as_ref() {
                let static_mangled =
                    STATIC_ATOMIC_FIELDS.with(|sf| sf.borrow().get(name.as_str()).cloned());
                if let Some(mangled) = static_mangled {
                    let mid = ident(&mangled);
                    let seqcst = quote! { ::std::sync::atomic::Ordering::SeqCst };
                    return match op {
                        UnOp::PostInc => Ok(quote! {
                            #mid.fetch_add(1, #seqcst)
                        }),
                        UnOp::PostDec => Ok(quote! {
                            #mid.fetch_sub(1, #seqcst)
                        }),
                        UnOp::PreInc => Ok(quote! { {
                            #mid.fetch_add(1, #seqcst);
                            #mid.load(#seqcst)
                        }}),
                        UnOp::PreDec => Ok(quote! { {
                            #mid.fetch_sub(1, #seqcst);
                            #mid.load(#seqcst)
                        }}),
                        _ => {
                            let operand_ts = emit_expr(operand)?;
                            match op {
                                UnOp::Neg => Ok(quote! { (-#operand_ts) }),
                                UnOp::Not => Ok(quote! { (!#operand_ts) }),
                                UnOp::BitNot => Ok(quote! { (!#operand_ts) }),
                                _ => unreachable!(),
                            }
                        }
                    };
                }
            }
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
            // Mutable static primitive field write: emit atomic store.
            if let IrExpr::Var { name, .. } = lhs.as_ref() {
                let static_mangled =
                    STATIC_ATOMIC_FIELDS.with(|sf| sf.borrow().get(name.as_str()).cloned());
                if let Some(mangled) = static_mangled {
                    let mid = ident(&mangled);
                    let val = emit_expr(rhs)?;
                    return Ok(quote! {
                        #mid.store(#val, ::std::sync::atomic::Ordering::SeqCst)
                    });
                }
                // Mutable static reference field write: emit OnceLock::set.
                // The `let _` is safe here: this code path is reached exclusively
                // from within `__run_static_init`, which is called through a
                // `std::sync::Once` guard — so `set` is invoked at most once per
                // field and will always succeed on that first call.
                let once_lock_mangled =
                    STATIC_ONCE_LOCK_FIELDS.with(|sf| sf.borrow().get(name.as_str()).cloned());
                if let Some(mangled) = once_lock_mangled {
                    let mid = ident(&mangled);
                    let val = emit_expr(rhs)?;
                    return Ok(quote! { let _ = #mid.set(#val) });
                }
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
            // Abstract I/O assignment: wrap with .into() when assigning a
            // concrete I/O constructor to a variable typed as the abstract base.
            let lhs_is_abstract_io = matches!(
                lhs.ty(),
                IrType::Class(ref c)
                if matches!(c.as_str(), "InputStream" | "OutputStream" | "Reader" | "Writer")
            ) && matches!(rhs.as_ref(), IrExpr::New { .. });
            if lhs_is_abstract_io {
                Ok(quote! { #l = (#r).into() })
            } else {
                Ok(quote! { #l = #r })
            }
        }

        IrExpr::CompoundAssign { op, lhs, rhs, .. } => {
            // Mutable static field compound-assign: emit fetch_add/fetch_sub etc.
            if let IrExpr::Var { name, .. } = lhs.as_ref() {
                let static_mangled =
                    STATIC_ATOMIC_FIELDS.with(|sf| sf.borrow().get(name.as_str()).cloned());
                if let Some(mangled) = static_mangled {
                    let mid = ident(&mangled);
                    let r = emit_expr(rhs)?;
                    let seqcst = quote! { ::std::sync::atomic::Ordering::SeqCst };
                    return match op {
                        BinOp::Add | BinOp::Concat => Ok(quote! { #mid.fetch_add(#r, #seqcst) }),
                        BinOp::Sub => Ok(quote! { #mid.fetch_sub(#r, #seqcst) }),
                        BinOp::BitAnd => Ok(quote! { #mid.fetch_and(#r, #seqcst) }),
                        BinOp::BitOr => Ok(quote! { #mid.fetch_or(#r, #seqcst) }),
                        BinOp::BitXor => Ok(quote! { #mid.fetch_xor(#r, #seqcst) }),
                        _ => {
                            // Fallback: load → op → store
                            let op_ts = emit_binop(op);
                            Ok(quote! {
                                #mid.store(
                                    #mid.load(#seqcst) #op_ts #r,
                                    #seqcst,
                                )
                            })
                        }
                    };
                }
            }
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
            match target {
                // Primitive casts: emit `as T`
                IrType::Bool
                | IrType::Byte
                | IrType::Short
                | IrType::Int
                | IrType::Long
                | IrType::Float
                | IrType::Double
                | IrType::Char => {
                    let ty = emit_type(target);
                    Ok(quote! { (#inner as #ty) })
                }
                // Reference/class casts are currently erased to identity here:
                // we do not emit a runtime-checked downcast, and instead rely
                // on the declared IR types lining up so the generated Rust
                // type-checks.
                _ => Ok(quote! { { #inner } }),
            }
        }

        IrExpr::InstanceOf {
            expr, check_type, ..
        } => {
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

        IrExpr::ClassLiteral { class_name } => Ok(quote! { JClass::new(#class_name) }),

        IrExpr::MethodRef {
            class_name,
            target,
            method_name,
            ..
        } => {
            let mident = ident(method_name);
            if method_name == "new" {
                let cident = ident(class_name.as_deref().unwrap_or("Unknown"));
                Ok(quote! { |__x0| #cident::new(__x0) })
            } else if let Some(cls) = class_name {
                let cident = ident(cls);
                Ok(quote! { |__x0| #cident::#mident(__x0) })
            } else if let Some(recv) = target {
                // Special case: System.out::println → |__x0| println!("{}", __x0)
                //               System.err::println → |__x0| eprintln!("{}", __x0)
                if let IrExpr::FieldAccess {
                    receiver,
                    field_name,
                    ..
                } = recv.as_ref()
                {
                    if let IrExpr::Var { name, .. } = receiver.as_ref() {
                        if name == "System"
                            && (field_name == "out" || field_name == "err")
                            && method_name == "println"
                        {
                            if field_name == "err" {
                                return Ok(quote! { |__x0| eprintln!("{}", __x0) });
                            } else {
                                return Ok(quote! { |__x0| println!("{}", __x0) });
                            }
                        }
                    }
                }
                // Handle `this::method` — need `self.clone()` as receiver
                let recv_ts = if matches!(
                    recv.as_ref(),
                    IrExpr::Var { name, .. } if name == "self" || name == "__self__"
                ) {
                    quote! { self.clone() }
                } else {
                    emit_expr(recv)?
                };
                // `mut` is required because generated instance methods take `&mut self`.
                Ok(quote! { { let mut __ref = #recv_ts; move |__x0| __ref.#mident(__x0) } })
            } else {
                Err(CodegenError::Unsupported(
                    "method reference without class or target".into(),
                ))
            }
        }

        IrExpr::SwitchExpr {
            expr,
            arms,
            default,
            ..
        } => {
            let scrutinee = emit_expr(expr)?;
            let arm_tokens = arms
                .iter()
                .map(|(pat, body)| {
                    let p = emit_expr(pat)?;
                    let b = emit_expr(body)?;
                    Ok(quote! { #p => #b, })
                })
                .collect::<Result<Vec<_>, CodegenError>>()?;
            let default_arm = if let Some(d) = default {
                let d = emit_expr(d)?;
                quote! { _ => #d, }
            } else {
                quote! { _ => unreachable!(), }
            };
            Ok(quote! { match #scrutinee { #(#arm_tokens)* #default_arm } })
        }

        IrExpr::BlockExpr { stmts, expr, .. } => {
            let stmts_ts = emit_stmts(stmts)?;
            let val = emit_expr(expr)?;
            Ok(quote! { { #(#stmts_ts)* #val } })
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
                "InputStream" => quote! { JInputStream },
                "OutputStream" => quote! { JOutputStream },
                "Reader" => quote! { JReader },
                "Writer" => quote! { JWriter },
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
                "Properties" => quote! { JProperties },
                "Timer" => quote! { JTimer },
                "TimerTask" => quote! { JTimerTask },
                "ZonedDateTime" => quote! { JZonedDateTime },
                "ZoneId" => quote! { JZoneId },
                "Clock" => quote! { JClock },
                "HttpClient" => quote! { JHttpClient },
                "HttpRequest" => quote! { JHttpRequest },
                "HttpResponse" => quote! { JHttpResponse },
                "URI" => quote! { JURL },
                "Object" => quote! { JavaObject },
                // Reflection / class literals
                "Class" | "JClass" => quote! { JClass },
                // Abstract I/O base types
                "StringWriter" => quote! { JStringWriter },
                "StringReader" => quote! { JStringReader },
                "ByteArrayOutputStream" => quote! { JByteArrayOutputStream },
                "ByteArrayInputStream" => quote! { JByteArrayInputStream },
                // ResourceBundle
                "ResourceBundle" | "PropertyResourceBundle" => quote! { JResourceBundle },
                // Stage 13 additions
                "StampedLock" => quote! { JStampedLock },
                "ForkJoinPool" => quote! { JForkJoinPool },
                "RecursiveTask" | "RecursiveAction" => quote! { () },
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
            // Special-case HttpResponse<T> → JHttpResponse (runtime type is not generic).
            if matches!(base.as_ref(), IrType::Class(c) if c == "HttpResponse") {
                return quote! { JHttpResponse };
            }
            // Class<?> / Class<T> → JClass (the runtime type is not generic).
            if matches!(base.as_ref(), IrType::Class(c) if c == "Class") {
                return quote! { JClass };
            }
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

/// Sanitise a Java loop label and construct a Rust `syn::Lifetime` from it.
/// Java labels may contain characters (e.g. `$`) that are illegal in Rust
/// lifetime identifiers; we map every character that is not alphanumeric or
/// `_` to `_`, mirroring the sanitisation done by `ident()`.
fn label_lifetime(lbl: &str) -> syn::Lifetime {
    let sanitised: String = lbl
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    syn::Lifetime::new(&format!("'{}", sanitised), proc_macro2::Span::call_site())
}

/// Check if an expression is `Collectors.toList()` (a MethodCall on `Collectors` with name `toList`).
fn is_collectors_to_list(expr: &IrExpr) -> bool {
    is_collectors_method(expr, "toList")
}

/// Check if an expression is `Collectors.<method_name>(...)`.
fn is_collectors_method(expr: &IrExpr, target: &str) -> bool {
    if let IrExpr::MethodCall {
        receiver: Some(recv),
        method_name,
        ..
    } = expr
    {
        if method_name == target {
            if let IrExpr::Var { name, .. } = recv.as_ref() {
                return name == "Collectors";
            }
        }
    }
    false
}

/// If `expr` is `Collectors.joining(...)`, return the (already-emitted) arg token streams.
fn collectors_joining_args(expr: &IrExpr) -> Option<Result<Vec<TokenStream>, CodegenError>> {
    if let IrExpr::MethodCall {
        receiver: Some(recv),
        method_name,
        args,
        ..
    } = expr
    {
        if method_name == "joining" {
            if let IrExpr::Var { name, .. } = recv.as_ref() {
                if name == "Collectors" {
                    return Some(args.iter().map(emit_expr).collect::<Result<Vec<_>, _>>());
                }
            }
        }
    }
    None
}

/// If `expr` is `Collectors.<method>(fn1, fn2)`, return emitted arg token streams.
fn collectors_two_fn_args(
    expr: &IrExpr,
    method: &str,
) -> Option<Result<Vec<TokenStream>, CodegenError>> {
    if let IrExpr::MethodCall {
        receiver: Some(recv),
        method_name,
        args,
        ..
    } = expr
    {
        if method_name == method && args.len() == 2 {
            if let IrExpr::Var { name, .. } = recv.as_ref() {
                if name == "Collectors" {
                    return Some(args.iter().map(emit_expr).collect::<Result<Vec<_>, _>>());
                }
            }
        }
    }
    None
}

/// If `expr` is `Collectors.<method>(fn1)`, return emitted arg token streams.
fn collectors_one_fn_arg(
    expr: &IrExpr,
    method: &str,
) -> Option<Result<Vec<TokenStream>, CodegenError>> {
    if let IrExpr::MethodCall {
        receiver: Some(recv),
        method_name,
        args,
        ..
    } = expr
    {
        if method_name == method && args.len() == 1 {
            if let IrExpr::Var { name, .. } = recv.as_ref() {
                if name == "Collectors" {
                    return Some(args.iter().map(emit_expr).collect::<Result<Vec<_>, _>>());
                }
            }
        }
    }
    None
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

fn as_i32_literal(expr: &IrExpr) -> Option<i32> {
    if let IrExpr::LitInt(n) = expr {
        Some(*n as i32)
    } else {
        None
    }
}

fn as_i64_literal(expr: &IrExpr) -> Option<i64> {
    match expr {
        IrExpr::LitLong(n) => Some(*n),
        IrExpr::LitInt(n) => Some(*n),
        _ => None,
    }
}

fn as_bool_literal(expr: &IrExpr) -> Option<bool> {
    if let IrExpr::LitBool(b) = expr {
        Some(*b)
    } else {
        None
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
                is_default: false,
            }],
            constructors: vec![],
            is_record: false,
            captures: vec![],
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
            label: None,
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
            label: None,
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
            label: None,
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
            label: None,
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
            is_default: false,
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
                is_default: false,
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
            binding: None,
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
            is_default: false,
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
                is_default: false,
            }],
            constructors: vec![],
            is_record: false,
            captures: vec![],
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
                is_default: false,
            }],
            constructors: vec![],
            is_record: false,
            captures: vec![],
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
            is_record: false,
            captures: vec![],
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
                is_default: false,
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
                    is_default: false,
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
                    is_default: false,
                },
            ],
            constructors: vec![],
            is_record: false,
            captures: vec![],
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
            is_default: false,
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
            is_default: false,
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
            is_record: false,
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
            captures: vec![],
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
                is_default: false,
            }],
            is_record: false,
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
            captures: vec![],
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
            label: None,
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
            binding: None,
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
            is_record: false,
            constructors: vec![ir::decl::IrConstructor {
                visibility: Visibility::Public,
                params: vec![],
                body: vec![],
                throws: vec![],
            }],
            captures: vec![],
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
                is_default: false,
            }],
            constructors: vec![],
            is_record: false,
            captures: vec![],
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
            is_record: false,
            constructors: vec![ir::decl::IrConstructor {
                visibility: Visibility::Public,
                params: vec![],
                body: vec![],
                throws: vec![],
            }],
            captures: vec![],
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
            label: None,
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
                is_default: false,
            }],
            constructors: vec![],
            is_record: false,
            captures: vec![],
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
                    is_default: false,
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
                    is_default: false,
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
                is_default: false,
            }],
            constructors: vec![],
            is_record: false,
            captures: vec![],
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
            is_record: false,
            captures: vec![],
        };
        module.decls.push(IrDecl::Class(cls));
        let code = gen(&module);
        assert!(
            code.contains("const MAX"),
            "should emit const for static final field"
        );
    }
}
