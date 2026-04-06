//! Top-level parse entry point and CST → IR lowering.

use std::cell::Cell;
use std::cell::RefCell;

use ir::{
    decl::{
        IrClass, IrConstructor, IrEnum, IrEnumConstant, IrField, IrInterface, IrMethod, IrParam,
        Visibility,
    },
    expr::{BinOp, UnOp},
    stmt::{CatchClause, SwitchCase},
    IrDecl, IrExpr, IrModule, IrStmt, IrType, IrTypeParam, WildcardBound,
};
use tree_sitter::{Language, Node, Parser};

use crate::{
    from_node::{node_kind_to_ir_type, primitive_keyword_to_ir_type},
    ParseError,
};

// ─── thread-local state for anonymous/inner/local class hoisting ─────────────

// Counter for generating unique anonymous-class names.
thread_local! {
    static ANON_COUNTER: Cell<u32> = const { Cell::new(0) };
}

// Classes to hoist to module level (anonymous, inner, local).
thread_local! {
    static HOISTED_DECLS: RefCell<Vec<IrDecl>> = const { RefCell::new(Vec::new()) };
}

// Stack of enclosing class names (for mangling inner/local class names).
thread_local! {
    static OUTER_CLASS_STACK: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn current_outer_class() -> String {
    OUTER_CLASS_STACK.with(|s| s.borrow().last().cloned().unwrap_or_default())
}

/// Returns the full enclosing-class prefix by joining all stack entries with
/// `$`, e.g. `"Outer$Inner"` when inside an inner class of `Outer`.
fn full_outer_class_prefix() -> String {
    OUTER_CLASS_STACK.with(|s| s.borrow().join("$"))
}

fn push_outer_class(name: &str) {
    OUTER_CLASS_STACK.with(|s| s.borrow_mut().push(name.to_owned()));
}

fn pop_outer_class() {
    OUTER_CLASS_STACK.with(|s| {
        s.borrow_mut().pop();
    });
}

/// RAII guard that calls `pop_outer_class()` when dropped, ensuring the stack
/// stays balanced even when `lower_class` returns early via `?`.
struct OuterClassGuard;

impl Drop for OuterClassGuard {
    fn drop(&mut self) {
        pop_outer_class();
    }
}

fn hoist_decl(decl: IrDecl) {
    HOISTED_DECLS.with(|h| h.borrow_mut().push(decl));
}

fn next_anon_name() -> String {
    let n = ANON_COUNTER.with(|c| {
        let v = c.get();
        c.set(v + 1);
        v
    });
    format!("__Anon_{n}")
}

/// Parse `source` as Java and return the tree-sitter tree.
///
/// Returns `Err` if tree-sitter reports any error nodes in the root of the
/// tree, i.e. the source is not valid Java.
pub fn parse_source(source: &str) -> Result<tree_sitter::Tree, ParseError> {
    let mut parser = Parser::new();
    let language: Language = tree_sitter_java::language();
    parser
        .set_language(&language)
        .expect("tree-sitter-java grammar version mismatch");

    let tree = parser
        .parse(source, None)
        .expect("tree-sitter parse returned None (should be infallible)");

    if tree.root_node().has_error() {
        let error_node = find_first_error(tree.root_node());
        let offset = error_node.map(|n| n.start_byte()).unwrap_or(0);
        return Err(ParseError::SyntaxError {
            offset,
            message: "tree-sitter reported a parse error".into(),
        });
    }

    Ok(tree)
}

/// Parse `source` as Java and lower it to an [`IrModule`].
pub fn parse_to_ir(source: &str) -> Result<IrModule, ParseError> {
    // Reset thread-locals so re-entrant calls (tests) start clean.
    ANON_COUNTER.with(|c| c.set(0));
    HOISTED_DECLS.with(|h| h.borrow_mut().clear());
    OUTER_CLASS_STACK.with(|s| s.borrow_mut().clear());

    let tree = parse_source(source)?;
    let root = tree.root_node();
    let mut module = lower_program(root, source.as_bytes())?;

    // Append any classes hoisted during the walk (anonymous/inner/local).
    HOISTED_DECLS.with(|h| module.decls.extend(h.borrow_mut().drain(..)));

    Ok(module)
}

fn find_first_error(node: Node<'_>) -> Option<Node<'_>> {
    if node.is_error() || node.is_missing() {
        return Some(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(err) = find_first_error(child) {
            return Some(err);
        }
    }
    None
}

// ─── helpers ────────────────────────────────────────────────────────────────

fn text<'a>(node: Node<'_>, src: &'a [u8]) -> &'a str {
    std::str::from_utf8(&src[node.start_byte()..node.end_byte()]).unwrap_or("")
}

fn child_by_field<'a>(node: Node<'a>, field: &str) -> Option<Node<'a>> {
    node.child_by_field_name(field)
}

fn named_children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

fn children_by_field_name<'a>(node: Node<'a>, field: &str) -> Vec<Node<'a>> {
    let mut cursor = node.walk();
    node.children_by_field_name(field, &mut cursor).collect()
}

/// Extract generic type parameters (with bounds) from a `type_parameters` parent node.
///
/// Tree-sitter-java structure:
/// ```text
/// type_parameters: < type_parameter [ type_identifier, type_bound? ] , ... >
/// type_bound: extends type1 & type2 & ...
/// ```
fn lower_type_params(node: Node<'_>, src: &[u8]) -> Vec<IrTypeParam> {
    child_by_field(node, "type_parameters")
        .map(|tp_node| {
            named_children(tp_node)
                .into_iter()
                .filter(|n| n.kind() == "type_parameter")
                .filter_map(|tp| {
                    let children = named_children(tp);
                    // First type_identifier child is the name
                    let name = children
                        .iter()
                        .find(|n| n.kind() == "type_identifier")
                        .map(|n| text(*n, src).to_owned())?;
                    // Optional type_bound child contains the bounds
                    let bounds: Vec<IrType> = children
                        .iter()
                        .find(|n| n.kind() == "type_bound")
                        .map(|bound_node| {
                            named_children(*bound_node)
                                .into_iter()
                                .filter(|n| {
                                    n.kind() == "type_identifier" || n.kind() == "generic_type"
                                })
                                .map(|n| lower_type(n, src))
                                .collect()
                        })
                        .unwrap_or_default();
                    Some(IrTypeParam { name, bounds })
                })
                .collect()
        })
        .unwrap_or_default()
}

// ─── program ────────────────────────────────────────────────────────────────

fn lower_program(node: Node<'_>, src: &[u8]) -> Result<IrModule, ParseError> {
    let mut module = IrModule::new("");
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "package_declaration" => {
                if let Some(name_node) = child.named_child(0) {
                    module.package = text(name_node, src).to_owned();
                }
            }
            "import_declaration" => {
                // Collect import text (skip keyword tokens)
                let import_text = text(child, src)
                    .trim_start_matches("import")
                    .trim_end_matches(';')
                    .trim()
                    .to_owned();
                module.imports.push(import_text);
            }
            "class_declaration" => {
                let (cls, inner_enums) = lower_class(child, src)?;
                let cls_name = cls.name.clone();
                module.decls.push(IrDecl::Class(cls));
                for mut enm in inner_enums {
                    // Mangle nested enum names with the enclosing class name
                    // (e.g. `Outer$Day`) to avoid collisions when multiple
                    // classes define nested enums with the same simple name.
                    let prefix = format!("{}$", cls_name);
                    if !enm.name.starts_with(&prefix) {
                        enm.name = format!("{}{}", prefix, enm.name);
                    }
                    module.decls.push(IrDecl::Enum(enm));
                }
            }
            "interface_declaration" => {
                module
                    .decls
                    .push(IrDecl::Interface(lower_interface(child, src)?));
            }
            "enum_declaration" => {
                module.decls.push(IrDecl::Enum(lower_enum(child, src)?));
            }
            "record_declaration" => {
                module.decls.push(IrDecl::Class(lower_record(child, src)?));
            }
            _ => {} // skip unknown top-level nodes
        }
    }
    Ok(module)
}

// ─── record ─────────────────────────────────────────────────────────────────

fn lower_record(node: Node<'_>, src: &[u8]) -> Result<IrClass, ParseError> {
    let name = child_by_field(node, "name")
        .map(|n| text(n, src).to_owned())
        .unwrap_or_default();
    let visibility = extract_visibility(node, src);
    let type_params = lower_type_params(node, src);

    // Record components share the same grammar as formal_parameters.
    let params: Vec<IrParam> = child_by_field(node, "parameters")
        .map(|params_node| lower_params(params_node, src))
        .unwrap_or_default();

    // Each component becomes a public final instance field.
    let fields: Vec<IrField> = params
        .iter()
        .map(|p| IrField {
            name: p.name.clone(),
            ty: p.ty.clone(),
            visibility: Visibility::Public,
            is_static: false,
            is_final: true,
            is_volatile: false,
            init: None,
        })
        .collect();

    // Canonical constructor: assigns each component to the corresponding field.
    let constructor_body: Vec<IrStmt> = params
        .iter()
        .map(|p| {
            IrStmt::Expr(IrExpr::Assign {
                lhs: Box::new(IrExpr::FieldAccess {
                    receiver: Box::new(IrExpr::Var {
                        name: "self".to_owned(),
                        ty: IrType::Unknown,
                    }),
                    field_name: p.name.clone(),
                    ty: p.ty.clone(),
                }),
                rhs: Box::new(IrExpr::Var {
                    name: p.name.clone(),
                    ty: p.ty.clone(),
                }),
                ty: p.ty.clone(),
            })
        })
        .collect();

    let canonical_ctor = IrConstructor {
        visibility: Visibility::Public,
        params: params.clone(),
        body: constructor_body,
        throws: vec![],
    };

    // Interfaces the record implements (from `implements` clause).
    let interfaces: Vec<String> = child_by_field(node, "interfaces")
        .map(|ifaces_node| {
            let mut c = ifaces_node.walk();
            ifaces_node
                .named_children(&mut c)
                .flat_map(|n| {
                    if n.kind() == "type_identifier" || n.kind() == "generic_type" {
                        vec![n]
                    } else {
                        named_children(n)
                    }
                })
                .filter(|n| n.kind() == "type_identifier" || n.kind() == "generic_type")
                .map(|n| text(n, src).to_owned())
                .collect()
        })
        .unwrap_or_default();

    // Extra methods declared in the record body.
    let mut methods: Vec<IrMethod> = Vec::new();
    if let Some(body) = child_by_field(node, "body") {
        let mut cur = body.walk();
        for child in body.named_children(&mut cur) {
            if child.kind() == "method_declaration" {
                if let Ok(m) = lower_method(child, src) {
                    methods.push(m);
                }
            }
        }
    }

    Ok(IrClass {
        name,
        visibility,
        is_abstract: false,
        is_final: true,
        is_record: true,
        type_params,
        superclass: None,
        interfaces,
        fields,
        methods,
        constructors: vec![canonical_ctor],
    })
}

// ─── class ──────────────────────────────────────────────────────────────────

fn lower_class(node: Node<'_>, src: &[u8]) -> Result<(IrClass, Vec<IrEnum>), ParseError> {
    let name = child_by_field(node, "name")
        .map(|n| text(n, src).to_owned())
        .unwrap_or_default();

    // Track the class-name stack so inner/local classes can build mangled names.
    // Use an RAII guard so pop always runs even if an error is returned via `?`.
    push_outer_class(&name);
    let _outer_guard = OuterClassGuard;

    let visibility = extract_visibility(node, src);

    let mut is_abstract = false;
    let mut is_final = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mods = text(child, src);
            if mods.contains("abstract") {
                is_abstract = true;
            }
            if mods.contains("final") {
                is_final = true;
            }
        }
    }

    let superclass = child_by_field(node, "superclass")
        .map(|n| text(n, src).trim_start_matches("extends").trim().to_owned());

    // Generic type parameters with bounds: `class Box<T extends Comparable<T>>`
    let type_params = lower_type_params(node, src);

    let interfaces: Vec<String> = child_by_field(node, "interfaces")
        .map(|ifaces_node| {
            // tree-sitter-java: super_interfaces → interface_type_list → type_identifier
            // We flatten one extra level to handle the intermediate wrapper node.
            named_children(ifaces_node)
                .into_iter()
                .flat_map(|n| {
                    if n.kind() == "type_identifier" || n.kind() == "generic_type" {
                        vec![n]
                    } else {
                        // unwrap interface_type_list or similar container
                        named_children(n)
                    }
                })
                .filter(|n| n.kind() == "type_identifier" || n.kind() == "generic_type")
                .map(|n| text(n, src).to_owned())
                .collect()
        })
        .unwrap_or_default();

    let mut fields = Vec::new();
    let mut methods = Vec::new();
    let mut constructors = Vec::new();
    let mut inner_enums = Vec::new();

    if let Some(body) = child_by_field(node, "body") {
        let mut cur = body.walk();
        for child in body.named_children(&mut cur) {
            match child.kind() {
                "field_declaration" => {
                    fields.extend(lower_field(child, src)?);
                }
                "method_declaration" => {
                    methods.push(lower_method(child, src)?);
                }
                "constructor_declaration" => {
                    constructors.push(lower_constructor(child, src)?);
                }
                "enum_declaration" => {
                    inner_enums.push(lower_enum(child, src)?);
                }
                "class_declaration" => {
                    // Non-static inner class: hoist to module level with
                    // mangled name "{Outer}${Inner}", using the full enclosing-
                    // class stack so deeply nested classes get names like
                    // "Outer$Inner$Nested" rather than just "Inner$Nested".
                    let outer = full_outer_class_prefix();
                    let (mut inner_cls, inner_sub_enums) = lower_class(child, src)?;
                    let orig_inner_name = inner_cls.name.clone();
                    let inner_prefix = format!("{}${}", outer, orig_inner_name);
                    inner_cls.name = inner_prefix.clone();
                    // Also mangle any inner-of-inner enums, preserving any
                    // nested suffixes that were already lowered relative to
                    // the original immediate inner-class name.
                    let nested_prefix = format!("{}$", orig_inner_name);
                    for mut e in inner_sub_enums {
                        if let Some(rest) = e.name.strip_prefix(&nested_prefix) {
                            e.name = format!("{}${}", inner_prefix, rest);
                        } else {
                            e.name = format!("{}${}", inner_prefix, e.name);
                        }
                        inner_enums.push(e);
                    }
                    hoist_decl(IrDecl::Class(inner_cls));
                }
                "static_initializer" => {
                    // Represent as a synthetic `__clinit__` static method so
                    // the codegen can emit an Once-guarded init function.
                    // tree-sitter-java: static_initializer has no named fields;
                    // its body is the anonymous `block` named child.
                    let mut c = child.walk();
                    let block = child.named_children(&mut c).find(|n| n.kind() == "block");
                    let body_stmts = block
                        .map(|b| lower_block(b, src))
                        .transpose()?
                        .unwrap_or_default();
                    methods.push(IrMethod {
                        name: "__clinit__".into(),
                        visibility: Visibility::Public,
                        is_static: true,
                        is_abstract: false,
                        is_final: false,
                        is_synchronized: false,
                        type_params: vec![],
                        params: vec![],
                        return_ty: IrType::Void,
                        body: Some(body_stmts),
                        throws: vec![],
                    });
                }
                _ => {}
            }
        }
    }

    Ok((
        IrClass {
            name,
            visibility,
            is_abstract,
            is_final,
            type_params,
            superclass,
            interfaces,
            fields,
            methods,
            constructors,
            is_record: false,
        },
        inner_enums,
    ))
}

// ─── interface ──────────────────────────────────────────────────────────────

fn lower_interface(node: Node<'_>, src: &[u8]) -> Result<IrInterface, ParseError> {
    let name = child_by_field(node, "name")
        .map(|n| text(n, src).to_owned())
        .unwrap_or_default();
    let visibility = extract_visibility(node, src);

    // extends clause (interface can extend multiple interfaces)
    let extends: Vec<String> = child_by_field(node, "extends_interfaces")
        .map(|ext_node| {
            named_children(ext_node)
                .into_iter()
                .filter(|n| n.kind() == "type_identifier" || n.kind() == "generic_type")
                .map(|n| text(n, src).to_owned())
                .collect()
        })
        .unwrap_or_default();

    let mut methods = Vec::new();
    if let Some(body) = child_by_field(node, "body") {
        let mut cur = body.walk();
        for child in body.named_children(&mut cur) {
            if child.kind() == "method_declaration" {
                methods.push(lower_method(child, src)?);
            }
        }
    }

    // Generic type parameters with bounds
    let type_params = lower_type_params(node, src);

    Ok(IrInterface {
        name,
        visibility,
        type_params,
        extends,
        methods,
    })
}

// ─── enum ───────────────────────────────────────────────────────────────────

fn lower_enum(node: Node<'_>, src: &[u8]) -> Result<IrEnum, ParseError> {
    let name = child_by_field(node, "name")
        .map(|n| text(n, src).to_owned())
        .unwrap_or_default();
    let visibility = extract_visibility(node, src);

    let interfaces: Vec<String> = child_by_field(node, "interfaces")
        .map(|ifaces_node| {
            named_children(ifaces_node)
                .into_iter()
                .flat_map(|n| {
                    if n.kind() == "type_identifier" || n.kind() == "generic_type" {
                        vec![n]
                    } else {
                        named_children(n)
                    }
                })
                .filter(|n| n.kind() == "type_identifier" || n.kind() == "generic_type")
                .map(|n| text(n, src).to_owned())
                .collect()
        })
        .unwrap_or_default();

    let mut constants = Vec::new();
    let mut fields = Vec::new();
    let mut methods = Vec::new();
    let mut constructor = None;

    if let Some(body) = child_by_field(node, "body") {
        let mut cur = body.walk();
        for child in body.named_children(&mut cur) {
            match child.kind() {
                "enum_constant" => {
                    let const_name = named_children(child)
                        .into_iter()
                        .find(|n| n.kind() == "identifier")
                        .map(|n| text(n, src).to_owned())
                        .unwrap_or_default();
                    let args = named_children(child)
                        .into_iter()
                        .find(|n| n.kind() == "argument_list")
                        .map(|al| {
                            named_children(al)
                                .into_iter()
                                .map(|a| lower_expr(a, src))
                                .collect::<Result<Vec<_>, _>>()
                        })
                        .transpose()?
                        .unwrap_or_default();
                    // Per-constant class body: methods inside `ADD { ... }`
                    let const_body = named_children(child)
                        .into_iter()
                        .find(|n| n.kind() == "class_body")
                        .map(|cb| {
                            named_children(cb)
                                .into_iter()
                                .filter(|n| n.kind() == "method_declaration")
                                .map(|n| lower_method(n, src))
                                .collect::<Result<Vec<_>, _>>()
                        })
                        .transpose()?
                        .unwrap_or_default();
                    constants.push(IrEnumConstant {
                        name: const_name,
                        args,
                        body: const_body,
                    });
                }
                "enum_body_declarations" => {
                    let mut cur2 = child.walk();
                    for decl in child.named_children(&mut cur2) {
                        match decl.kind() {
                            "field_declaration" => {
                                fields.extend(lower_field(decl, src)?);
                            }
                            "method_declaration" => {
                                methods.push(lower_method(decl, src)?);
                            }
                            "constructor_declaration" => {
                                if constructor.is_some() {
                                    return Err(ParseError::Unsupported(
                                        "multiple enum constructors are not supported".to_string(),
                                    ));
                                }
                                constructor = Some(lower_constructor(decl, src)?);
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Ok(IrEnum {
        name,
        visibility,
        interfaces,
        constants,
        fields,
        methods,
        constructor,
    })
}

fn extract_visibility(node: Node<'_>, src: &[u8]) -> Visibility {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mods = text(child, src);
            if mods.contains("public") {
                return Visibility::Public;
            } else if mods.contains("protected") {
                return Visibility::Protected;
            } else if mods.contains("private") {
                return Visibility::Private;
            }
        }
    }
    Visibility::PackagePrivate
}

// ─── field ──────────────────────────────────────────────────────────────────

fn lower_field(node: Node<'_>, src: &[u8]) -> Result<Vec<IrField>, ParseError> {
    let vis = extract_visibility(node, src);
    let mods_text = named_children(node)
        .iter()
        .filter(|n| n.kind() == "modifiers")
        .map(|n| text(*n, src))
        .next()
        .unwrap_or("")
        .to_owned();
    let is_static = mods_text.contains("static");
    let is_final = mods_text.contains("final");
    let is_volatile = mods_text.contains("volatile");

    let ty = child_by_field(node, "type")
        .map(|n| lower_type(n, src))
        .unwrap_or(IrType::Unknown);

    // A field declaration can declare multiple variables: `int x = 1, y = 2;`
    let mut fields = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            let name = child_by_field(child, "name")
                .map(|n| text(n, src).to_owned())
                .unwrap_or_default();
            let init = child_by_field(child, "value")
                .map(|n| lower_expr(n, src))
                .transpose()?;
            // Volatile fields use atomic types so concurrent reads/writes are safe.
            let field_ty = if is_volatile {
                match &ty {
                    IrType::Int => IrType::Atomic(Box::new(IrType::Int)),
                    IrType::Long => IrType::Atomic(Box::new(IrType::Long)),
                    IrType::Bool => IrType::Atomic(Box::new(IrType::Bool)),
                    other => other.clone(),
                }
            } else {
                ty.clone()
            };
            fields.push(IrField {
                name,
                ty: field_ty,
                visibility: vis,
                is_static,
                is_final,
                is_volatile,
                init,
            });
        }
    }
    Ok(fields)
}

// ─── method ─────────────────────────────────────────────────────────────────

fn lower_method(node: Node<'_>, src: &[u8]) -> Result<IrMethod, ParseError> {
    let name = child_by_field(node, "name")
        .map(|n| text(n, src).to_owned())
        .unwrap_or_default();
    let vis = extract_visibility(node, src);

    let mods_text = named_children(node)
        .iter()
        .filter(|n| n.kind() == "modifiers")
        .map(|n| text(*n, src))
        .next()
        .unwrap_or("")
        .to_owned();
    let is_static = mods_text.contains("static");
    let is_abstract = mods_text.contains("abstract");
    let is_final = mods_text.contains("final");
    let is_synchronized = mods_text.contains("synchronized");

    let return_ty = child_by_field(node, "type")
        .map(|n| lower_type(n, src))
        .unwrap_or(IrType::Void);

    let params = child_by_field(node, "parameters")
        .map(|params_node| lower_params(params_node, src))
        .unwrap_or_default();

    let body = child_by_field(node, "body")
        .map(|b| lower_block(b, src))
        .transpose()?;

    let throws = child_by_field(node, "throws")
        .map(|t| {
            named_children(t)
                .iter()
                .map(|n| text(*n, src).to_owned())
                .collect()
        })
        .unwrap_or_default();

    // Generic type parameters with bounds: `<T extends Comparable<T>> T identity(T x)`
    let type_params = lower_type_params(node, src);

    Ok(IrMethod {
        name,
        visibility: vis,
        is_static,
        is_abstract,
        is_final,
        is_synchronized,
        type_params,
        params,
        return_ty,
        body,
        throws,
    })
}

fn lower_constructor(node: Node<'_>, src: &[u8]) -> Result<IrConstructor, ParseError> {
    let vis = extract_visibility(node, src);
    let params = child_by_field(node, "parameters")
        .map(|p| lower_params(p, src))
        .unwrap_or_default();
    let body = child_by_field(node, "body")
        .map(|b| lower_block(b, src))
        .transpose()?
        .unwrap_or_default();
    let throws = child_by_field(node, "throws")
        .map(|t| {
            named_children(t)
                .iter()
                .map(|n| text(*n, src).to_owned())
                .collect()
        })
        .unwrap_or_default();
    Ok(IrConstructor {
        visibility: vis,
        params,
        body,
        throws,
    })
}

fn lower_params(node: Node<'_>, src: &[u8]) -> Vec<IrParam> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|n| n.kind() == "formal_parameter" || n.kind() == "spread_parameter")
        .map(|p| {
            let is_varargs = p.kind() == "spread_parameter";
            // spread_parameter has no named "type" field; find the type by looking
            // for the first child that is not `modifiers` or `variable_declarator`.
            // Store the type as IrType::Array(elem) so that field-accesses like
            // `nums.length` resolve correctly through the normal Array path.
            let ty = if is_varargs {
                let mut cur2 = p.walk();
                let ty_node = p
                    .named_children(&mut cur2)
                    .find(|n| n.kind() != "modifiers" && n.kind() != "variable_declarator");
                let elem_ty = ty_node
                    .map(|n| lower_type(n, src))
                    .unwrap_or(IrType::Unknown);
                IrType::Array(Box::new(elem_ty))
            } else {
                child_by_field(p, "type")
                    .map(|n| lower_type(n, src))
                    .unwrap_or(IrType::Unknown)
            };
            // spread_parameter has no direct "name" field; its name is inside a
            // variable_declarator child.  formal_parameter DOES have "name" directly.
            let name = if is_varargs {
                let mut cur2 = p.walk();
                let varargs_name = p
                    .named_children(&mut cur2)
                    .find(|n| n.kind() == "variable_declarator")
                    .and_then(|vd| child_by_field(vd, "name"))
                    .map(|n| text(n, src).to_owned())
                    .unwrap_or_default();
                varargs_name
            } else {
                child_by_field(p, "name")
                    .map(|n| text(n, src).to_owned())
                    .unwrap_or_default()
            };
            IrParam {
                name,
                ty,
                is_varargs,
            }
        })
        .collect()
}

// ─── type ────────────────────────────────────────────────────────────────────

fn lower_type(node: Node<'_>, src: &[u8]) -> IrType {
    match node.kind() {
        "void_type" => IrType::Void,
        "boolean_type" => IrType::Bool,
        "byte_type" => IrType::Byte,
        "short_type" => IrType::Short,
        "int_type" => IrType::Int,
        "long_type" => IrType::Long,
        "float_type" => IrType::Float,
        "double_type" => IrType::Double,
        "char_type" => IrType::Char,
        // tree-sitter-java groups primitives under these parent node kinds
        "integral_type" | "floating_point_type" => {
            let t = text(node, src);
            primitive_keyword_to_ir_type(t).unwrap_or(IrType::Unknown)
        }
        "array_type" => {
            let elem = child_by_field(node, "element")
                .map(|n| lower_type(n, src))
                .unwrap_or(IrType::Unknown);
            // Count the number of `[]` pairs in the `dimensions` field to
            // handle multi-dimensional types like `int[][]`.
            let dim_count = child_by_field(node, "dimensions")
                .map(|d| {
                    let dim_text = text(d, src);
                    dim_text.chars().filter(|&c| c == '[').count()
                })
                .unwrap_or(1);
            let mut ty = elem;
            for _ in 0..dim_count {
                ty = IrType::Array(Box::new(ty));
            }
            ty
        }
        "type_identifier" => {
            let t = text(node, src);
            node_kind_to_ir_type(node.kind(), t)
        }
        "generic_type" => {
            // e.g. List<String>
            let mut cursor = node.walk();
            let children: Vec<_> = node.named_children(&mut cursor).collect();
            let base = children
                .first()
                .map(|n| lower_type(*n, src))
                .unwrap_or(IrType::Unknown);
            let args = children
                .get(1)
                .map(|type_args| {
                    let mut c = type_args.walk();
                    type_args
                        .named_children(&mut c)
                        .map(|n| lower_type(n, src))
                        .collect()
                })
                .unwrap_or_default();
            IrType::Generic {
                base: Box::new(base),
                args,
            }
        }
        "wildcard" => {
            // `?`, `? extends X`, or `? super X`
            let children = named_children(node);
            // Look for extends/super keywords to determine bound kind
            let mut cursor = node.walk();
            let all_children: Vec<_> = node.children(&mut cursor).collect();
            let has_extends = all_children.iter().any(|c| c.kind() == "extends");
            let has_super = all_children.iter().any(|c| c.kind() == "super");
            let bound_type = children
                .iter()
                .find(|n| n.kind() == "type_identifier" || n.kind() == "generic_type")
                .map(|n| lower_type(*n, src));
            let bound = if has_extends {
                bound_type.map(|t| WildcardBound::Upper(Box::new(t)))
            } else if has_super {
                bound_type.map(|t| WildcardBound::Lower(Box::new(t)))
            } else {
                None
            };
            IrType::Wildcard { bound }
        }
        _ => {
            let t = text(node, src);
            node_kind_to_ir_type(node.kind(), t)
        }
    }
}

// ─── block / statements ─────────────────────────────────────────────────────

fn lower_block(node: Node<'_>, src: &[u8]) -> Result<Vec<IrStmt>, ParseError> {
    let mut stmts = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        stmts.extend(lower_stmt(child, src)?);
    }
    Ok(stmts)
}

fn lower_stmt(node: Node<'_>, src: &[u8]) -> Result<Vec<IrStmt>, ParseError> {
    match node.kind() {
        "local_variable_declaration" => {
            let ty = child_by_field(node, "type")
                .map(|n| lower_type(n, src))
                .unwrap_or(IrType::Unknown);
            let mut stmts = Vec::new();
            for decl in children_by_field_name(node, "declarator") {
                let name = child_by_field(decl, "name")
                    .map(|n| text(n, src).to_owned())
                    .unwrap_or_default();
                let init = child_by_field(decl, "value")
                    .map(|n| lower_expr(n, src))
                    .transpose()?;
                stmts.push(IrStmt::LocalVar {
                    name,
                    ty: ty.clone(),
                    init,
                });
            }
            Ok(stmts)
        }
        "if_statement" => {
            let cond = child_by_field(node, "condition")
                .map(|n| {
                    // tree-sitter wraps the condition in a parenthesized_expression
                    let inner = if n.kind() == "parenthesized_expression" {
                        n.named_child(0).unwrap_or(n)
                    } else {
                        n
                    };
                    lower_expr(inner, src)
                })
                .transpose()?
                .unwrap_or(IrExpr::LitBool(true));

            let consequence = child_by_field(node, "consequence")
                .map(|n| lower_stmt(n, src))
                .transpose()?
                .unwrap_or_default();

            let alternative = child_by_field(node, "alternative")
                .map(|n| lower_stmt(n, src))
                .transpose()?;

            Ok(vec![IrStmt::If {
                cond,
                then_: consequence,
                else_: alternative,
            }])
        }
        "while_statement" => {
            let cond = lower_paren_condition(node, src)?;
            let body = child_by_field(node, "body")
                .map(|n| lower_stmt(n, src))
                .transpose()?
                .unwrap_or_default();
            Ok(vec![IrStmt::While {
                cond,
                body,
                label: None,
            }])
        }
        "do_statement" => {
            let body = child_by_field(node, "body")
                .map(|n| lower_stmt(n, src))
                .transpose()?
                .unwrap_or_default();
            let cond = lower_paren_condition(node, src)?;
            Ok(vec![IrStmt::DoWhile {
                body,
                cond,
                label: None,
            }])
        }
        "labeled_statement" => {
            // Java: `lbl: for (...) { ... }` — attach the label to the inner loop.
            // tree-sitter-java does not use a named field for labeled_statement;
            // the label identifier is the first named child (statement_label alias).
            let kids = named_children(node);
            let lbl = kids
                .first()
                .map(|n| text(*n, src).to_owned())
                .ok_or_else(|| ParseError::Unsupported("labeled_statement missing label".into()))?;
            // The body statement is the second named child.
            let inner_node = kids
                .into_iter()
                .nth(1)
                .ok_or_else(|| ParseError::Unsupported("labeled_statement missing body".into()))?;
            let mut stmts = lower_stmt(inner_node, src)?;
            if let Some(
                IrStmt::While { label, .. }
                | IrStmt::DoWhile { label, .. }
                | IrStmt::For { label, .. }
                | IrStmt::ForEach { label, .. },
            ) = stmts.first_mut()
            {
                *label = Some(lbl);
            }
            Ok(stmts)
        }
        "for_statement" => {
            // init is either a local_variable_declaration or expression_statement
            let init = child_by_field(node, "init")
                .map(|n| lower_stmt(n, src))
                .transpose()?
                .unwrap_or_default()
                .into_iter()
                .next()
                .map(Box::new);

            let cond = child_by_field(node, "condition")
                .map(|n| lower_expr(n, src))
                .transpose()?;

            let update = children_by_field_name(node, "update")
                .into_iter()
                .map(|n| lower_expr(n, src))
                .collect::<Result<Vec<_>, _>>()?;

            let body = child_by_field(node, "body")
                .map(|n| lower_stmt(n, src))
                .transpose()?
                .unwrap_or_default();

            Ok(vec![IrStmt::For {
                init,
                cond,
                update,
                body,
                label: None,
            }])
        }
        "enhanced_for_statement" => {
            let var_ty = child_by_field(node, "type")
                .map(|n| lower_type(n, src))
                .unwrap_or(IrType::Unknown);
            let var = child_by_field(node, "name")
                .map(|n| text(n, src).to_owned())
                .unwrap_or_default();
            let iterable = child_by_field(node, "value")
                .map(|n| lower_expr(n, src))
                .transpose()?
                .unwrap_or(IrExpr::LitNull);
            let body = child_by_field(node, "body")
                .map(|n| lower_stmt(n, src))
                .transpose()?
                .unwrap_or_default();
            Ok(vec![IrStmt::ForEach {
                var,
                var_ty,
                iterable,
                body,
                label: None,
            }])
        }
        "switch_statement" | "switch_expression" => {
            let expr = child_by_field(node, "condition")
                .map(|n| {
                    let inner = if n.kind() == "parenthesized_expression" {
                        n.named_child(0).unwrap_or(n)
                    } else {
                        n
                    };
                    lower_expr(inner, src)
                })
                .transpose()?
                .unwrap_or(IrExpr::LitNull);

            let body_node = child_by_field(node, "body").unwrap_or(node);
            let mut cases = Vec::new();
            let mut default: Option<Vec<IrStmt>> = None;

            // Collect all switch_label + statement nodes, flattening
            // switch_block_statement_group wrappers (tree-sitter-java 0.21).
            let mut flat_children = Vec::new();
            let mut cur = body_node.walk();
            for child in body_node.named_children(&mut cur) {
                if child.kind() == "switch_block_statement_group" {
                    let mut inner_cur = child.walk();
                    for inner in child.named_children(&mut inner_cur) {
                        flat_children.push((inner.kind().to_owned(), inner.id(), inner));
                    }
                } else {
                    flat_children.push((child.kind().to_owned(), child.id(), child));
                }
            }

            let mut current_values: Vec<IrExpr> = Vec::new();
            let mut current_stmts: Vec<IrStmt> = Vec::new();
            let mut is_default = false;

            for (kind, _id, child) in &flat_children {
                match kind.as_str() {
                    "switch_label" => {
                        // Flush the previous case group when we have accumulated
                        // statements.  Crucially we capture the default body
                        // *before* draining/clearing the group so that patterns
                        // like `case A: default: <stmts>` are handled correctly.
                        if !current_stmts.is_empty() {
                            if is_default {
                                default = Some(current_stmts.clone());
                            }
                            for val in current_values.drain(..) {
                                cases.push(SwitchCase {
                                    value: val,
                                    body: current_stmts.clone(),
                                });
                            }
                            current_stmts.clear();
                            is_default = false;
                        }
                        let label_text = text(*child, src);
                        if label_text.contains("default") {
                            is_default = true;
                        } else {
                            // `case <expr> :`
                            for val_node in named_children(*child) {
                                if let Ok(e) = lower_expr(val_node, src) {
                                    current_values.push(e);
                                }
                            }
                        }
                    }
                    _ => {
                        // statement inside current case group
                        current_stmts.extend(lower_stmt(*child, src)?);
                    }
                }
            }
            // flush last group
            if !current_stmts.is_empty() {
                if is_default {
                    default = Some(current_stmts.clone());
                }
                if !current_values.is_empty() {
                    for val in current_values {
                        cases.push(SwitchCase {
                            value: val,
                            body: current_stmts.clone(),
                        });
                    }
                } else if !is_default {
                    // Statements with no case label and no default keyword —
                    // this should not normally occur in valid Java, but we
                    // treat them as the default body as a best-effort fallback.
                    default = Some(current_stmts);
                }
            }

            Ok(vec![IrStmt::Switch {
                expr,
                cases,
                default,
            }])
        }
        "return_statement" => {
            let expr = node
                .named_child(0)
                .map(|n| lower_expr(n, src))
                .transpose()?;
            Ok(vec![IrStmt::Return(expr)])
        }
        "break_statement" => {
            let label = node.named_child(0).map(|n| text(n, src).to_owned());
            Ok(vec![IrStmt::Break(label)])
        }
        "continue_statement" => {
            let label = node.named_child(0).map(|n| text(n, src).to_owned());
            Ok(vec![IrStmt::Continue(label)])
        }
        "synchronized_statement" => {
            // synchronized ( monitor_expr ) { body }
            let monitor = child_by_field(node, "lock")
                .or_else(|| {
                    // tree-sitter-java uses a parenthesized_expression as the first named child
                    named_children(node)
                        .into_iter()
                        .find(|n| n.kind() == "parenthesized_expression")
                })
                .and_then(|n| {
                    // unwrap parentheses to get the inner expression
                    if n.kind() == "parenthesized_expression" {
                        n.named_child(0)
                            .map(|inner| lower_expr(inner, src))
                            .transpose()
                            .ok()
                            .flatten()
                    } else {
                        lower_expr(n, src).ok()
                    }
                })
                .unwrap_or(IrExpr::LitNull);
            let body = named_children(node)
                .into_iter()
                .find(|n| n.kind() == "block")
                .map(|n| lower_block(n, src))
                .transpose()?
                .unwrap_or_default();
            Ok(vec![IrStmt::Synchronized { monitor, body }])
        }
        "throw_statement" => {
            let expr = node
                .named_child(0)
                .map(|n| lower_expr(n, src))
                .transpose()?
                .unwrap_or(IrExpr::LitNull);
            Ok(vec![IrStmt::Throw(expr)])
        }
        "try_statement" => {
            let body = child_by_field(node, "body")
                .map(|n| lower_block(n, src))
                .transpose()?
                .unwrap_or_default();

            let mut catches = Vec::new();
            let mut finally = None;
            let mut cur = node.walk();
            for child in node.named_children(&mut cur) {
                match child.kind() {
                    "catch_clause" => {
                        // In tree-sitter-java, catch_formal_parameter is a
                        // named child of catch_clause but NOT a named field,
                        // so child_by_field does not work here.
                        let catch_formal = named_children(child)
                            .into_iter()
                            .find(|n| n.kind() == "catch_formal_parameter");
                        let (exception_types, var) = if let Some(cfp) = catch_formal {
                            // "name" IS a named field on catch_formal_parameter.
                            let var = child_by_field(cfp, "name")
                                .or_else(|| {
                                    named_children(cfp)
                                        .into_iter()
                                        .find(|n| n.kind() == "identifier")
                                })
                                .map(|n| text(n, src).to_owned())
                                .unwrap_or_default();
                            let types: Vec<String> = named_children(cfp)
                                .iter()
                                .filter(|n| {
                                    n.kind() == "type_identifier" || n.kind() == "catch_type"
                                })
                                .flat_map(|n| {
                                    if n.kind() == "catch_type" {
                                        named_children(*n)
                                            .iter()
                                            .map(|t| text(*t, src).to_owned())
                                            .collect::<Vec<_>>()
                                    } else {
                                        vec![text(*n, src).to_owned()]
                                    }
                                })
                                .collect();
                            (types, var)
                        } else {
                            (vec![], String::new())
                        };
                        let catch_body = child_by_field(child, "body")
                            .map(|n| lower_block(n, src))
                            .transpose()?
                            .unwrap_or_default();
                        catches.push(CatchClause {
                            exception_types,
                            var,
                            body: catch_body,
                        });
                    }
                    "finally_clause" => {
                        // In tree-sitter-java the block inside finally_clause
                        // is NOT a named field, so search by kind.
                        finally = named_children(child)
                            .into_iter()
                            .find(|n| n.kind() == "block")
                            .map(|n| lower_block(n, src))
                            .transpose()?;
                    }
                    _ => {}
                }
            }

            Ok(vec![IrStmt::TryCatch {
                body,
                catches,
                finally,
            }])
        }
        // try (Resource r = new Resource()) { body } catch (...) { ... }
        // Desugared: let r = new Resource(); try { body } finally { r.close(); }
        "try_with_resources_statement" => {
            let mut result_stmts = Vec::new();
            let mut resource_names: Vec<String> = Vec::new();

            // Parse resource_specification → individual resource declarations
            if let Some(resources_node) = child_by_field(node, "resources") {
                let mut res_cursor = resources_node.walk();
                for res in resources_node.named_children(&mut res_cursor) {
                    if res.kind() != "resource" {
                        continue;
                    }
                    let ty = child_by_field(res, "type")
                        .map(|n| lower_type(n, src))
                        .unwrap_or(IrType::Unknown);
                    let name = child_by_field(res, "name")
                        .map(|n| text(n, src).to_owned())
                        .unwrap_or_default();
                    let init = child_by_field(res, "value")
                        .map(|n| lower_expr(n, src))
                        .transpose()?;
                    if !name.is_empty() {
                        resource_names.push(name.clone());
                        result_stmts.push(IrStmt::LocalVar { name, ty, init });
                    }
                }
            }

            let body = child_by_field(node, "body")
                .map(|n| lower_block(n, src))
                .transpose()?
                .unwrap_or_default();

            let mut catches = Vec::new();
            let mut user_finally: Vec<IrStmt> = Vec::new();
            let mut cur = node.walk();
            for child in node.named_children(&mut cur) {
                match child.kind() {
                    "catch_clause" => {
                        let catch_formal = named_children(child)
                            .into_iter()
                            .find(|n| n.kind() == "catch_formal_parameter");
                        let (exception_types, var) = if let Some(cfp) = catch_formal {
                            let var = child_by_field(cfp, "name")
                                .or_else(|| {
                                    named_children(cfp)
                                        .into_iter()
                                        .find(|n| n.kind() == "identifier")
                                })
                                .map(|n| text(n, src).to_owned())
                                .unwrap_or_default();
                            let types: Vec<String> = named_children(cfp)
                                .iter()
                                .filter(|n| {
                                    n.kind() == "type_identifier" || n.kind() == "catch_type"
                                })
                                .flat_map(|n| {
                                    if n.kind() == "catch_type" {
                                        named_children(*n)
                                            .iter()
                                            .map(|t| text(*t, src).to_owned())
                                            .collect::<Vec<_>>()
                                    } else {
                                        vec![text(*n, src).to_owned()]
                                    }
                                })
                                .collect();
                            (types, var)
                        } else {
                            (vec![], String::new())
                        };
                        let catch_body = child_by_field(child, "body")
                            .map(|n| lower_block(n, src))
                            .transpose()?
                            .unwrap_or_default();
                        catches.push(CatchClause {
                            exception_types,
                            var,
                            body: catch_body,
                        });
                    }
                    "finally_clause" => {
                        user_finally = named_children(child)
                            .into_iter()
                            .find(|n| n.kind() == "block")
                            .map(|n| lower_block(n, src))
                            .transpose()?
                            .unwrap_or_default();
                    }
                    _ => {}
                }
            }

            // Build close() calls in LIFO order, then append user finally stmts
            let mut close_stmts: Vec<IrStmt> = resource_names
                .iter()
                .rev()
                .map(|name| {
                    IrStmt::Expr(IrExpr::MethodCall {
                        receiver: Some(Box::new(IrExpr::Var {
                            name: name.clone(),
                            ty: IrType::Unknown,
                        })),
                        method_name: "close".to_owned(),
                        args: vec![],
                        ty: IrType::Void,
                    })
                })
                .collect();
            close_stmts.extend(user_finally);
            let finally = if close_stmts.is_empty() {
                None
            } else {
                Some(close_stmts)
            };

            result_stmts.push(IrStmt::TryCatch {
                body,
                catches,
                finally,
            });
            Ok(result_stmts)
        }
        "block" => {
            let inner = lower_block(node, src)?;
            Ok(vec![IrStmt::Block(inner)])
        }
        "expression_statement" => {
            let expr_node = node.named_child(0).unwrap_or(node);
            let expr = lower_expr(expr_node, src)?;
            Ok(vec![IrStmt::Expr(expr)])
        }
        // super(...) or this(...) constructor delegation
        "explicit_constructor_invocation" => {
            // Distinguish super() vs this() by inspecting unnamed children
            let is_super = named_children(node).iter().any(|ch| ch.kind() == "super") || {
                // Also check unnamed children for the "super" keyword token
                let src_text = text(node, src);
                src_text.trim_start().starts_with("super")
            };
            if is_super {
                let args = child_by_field(node, "arguments")
                    .map(|args_node| {
                        let mut c = args_node.walk();
                        args_node
                            .named_children(&mut c)
                            .map(|n| lower_expr(n, src))
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .transpose()?
                    .unwrap_or_default();
                Ok(vec![IrStmt::SuperConstructorCall { args }])
            } else {
                Ok(vec![]) // this(args) — deferred
            }
        }
        // Local class declaration inside a method body: hoist to module level
        // as "{Outer}__loc__{ClassName}" and emit no statements.
        "class_declaration" => {
            let outer = current_outer_class();
            let (mut cls, sub_enums) = lower_class(node, src)?;
            let orig_name = cls.name.clone();
            cls.name = format!("{}__loc__{}", outer, orig_name);
            for mut e in sub_enums {
                e.name = format!("{}${}", cls.name, e.name);
                hoist_decl(IrDecl::Enum(e));
            }
            hoist_decl(IrDecl::Class(cls));
            Ok(vec![])
        }
        // empty statement or not yet handled
        _ => Ok(vec![]),
    }
}

fn lower_paren_condition(node: Node<'_>, src: &[u8]) -> Result<IrExpr, ParseError> {
    child_by_field(node, "condition")
        .map(|n| {
            let inner = if n.kind() == "parenthesized_expression" {
                n.named_child(0).unwrap_or(n)
            } else {
                n
            };
            lower_expr(inner, src)
        })
        .transpose()?
        .ok_or_else(|| ParseError::Unsupported("missing condition".into()))
}

// ─── expressions ─────────────────────────────────────────────────────────────

fn lower_expr(node: Node<'_>, src: &[u8]) -> Result<IrExpr, ParseError> {
    match node.kind() {
        // ── literals ──────────────────────────────────────────────────────
        "true" => Ok(IrExpr::LitBool(true)),
        "false" => Ok(IrExpr::LitBool(false)),
        "null_literal" => Ok(IrExpr::LitNull),
        "decimal_integer_literal"
        | "hex_integer_literal"
        | "octal_integer_literal"
        | "binary_integer_literal" => {
            let raw = text(node, src).to_lowercase();
            if raw.ends_with('l') {
                let n = raw.trim_end_matches('l');
                let val = parse_int_literal(n).unwrap_or(0);
                Ok(IrExpr::LitLong(val))
            } else {
                let val = parse_int_literal(&raw).unwrap_or(0);
                Ok(IrExpr::LitInt(val))
            }
        }
        "decimal_floating_point_literal" => {
            let raw = text(node, src).to_lowercase();
            if raw.ends_with('f') {
                let n: f64 = raw.trim_end_matches('f').parse().unwrap_or(0.0);
                Ok(IrExpr::LitFloat(n))
            } else {
                let n: f64 = raw.trim_end_matches('d').parse().unwrap_or(0.0);
                Ok(IrExpr::LitDouble(n))
            }
        }
        "hex_floating_point_literal" => {
            let raw = text(node, src);
            Ok(IrExpr::LitDouble(parse_hex_float(raw)))
        }
        "string_literal" => {
            let raw = text(node, src);
            if raw.starts_with("\"\"\"") {
                // Java text block – strip delimiters then apply indent-stripping
                let inner = &raw[3..raw.len() - 3]; // remove leading/trailing """
                let content = strip_text_block_indent(inner);
                Ok(IrExpr::LitString(unescape_java_string(&content)))
            } else {
                let unquoted = raw.trim_start_matches('"').trim_end_matches('"');
                Ok(IrExpr::LitString(unescape_java_string(unquoted)))
            }
        }
        "character_literal" => {
            let raw = text(node, src);
            let unquoted = raw.trim_start_matches('\'').trim_end_matches('\'');
            let ch = unescape_java_char(unquoted);
            Ok(IrExpr::LitChar(ch))
        }

        // ── identifiers ───────────────────────────────────────────────────
        "identifier" => {
            let name = text(node, src).to_owned();
            Ok(IrExpr::Var {
                name,
                ty: IrType::Unknown,
            })
        }
        "this" => Ok(IrExpr::Var {
            name: "self".to_owned(),
            ty: IrType::Unknown,
        }),
        "super" => Ok(IrExpr::Var {
            name: "_super".to_owned(),
            ty: IrType::Unknown,
        }),

        // ── parenthesized ─────────────────────────────────────────────────
        "parenthesized_expression" => {
            let inner = node.named_child(0).unwrap_or(node);
            lower_expr(inner, src)
        }

        // ── binary expressions ────────────────────────────────────────────
        "binary_expression" => {
            let lhs_node = child_by_field(node, "left").unwrap_or(node);
            let rhs_node = child_by_field(node, "right").unwrap_or(node);
            let op_text = operator_text(node, src);
            let lhs = lower_expr(lhs_node, src)?;
            let rhs = lower_expr(rhs_node, src)?;
            let op = text_to_binop(&op_text);
            Ok(IrExpr::BinOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                ty: IrType::Unknown,
            })
        }

        // ── unary expressions ─────────────────────────────────────────────
        "unary_expression" => {
            let op_text = operator_text(node, src);
            let operand_node =
                child_by_field(node, "operand").unwrap_or(node.named_child(0).unwrap_or(node));
            let operand = lower_expr(operand_node, src)?;
            let op = match op_text.as_str() {
                "-" => UnOp::Neg,
                "!" => UnOp::Not,
                "~" => UnOp::BitNot,
                _ => UnOp::Neg,
            };
            Ok(IrExpr::UnOp {
                op,
                operand: Box::new(operand),
                ty: IrType::Unknown,
            })
        }
        "update_expression" => {
            let node_text = text(node, src);
            let (op, operand_src) = if node_text.starts_with("++") {
                (UnOp::PreInc, node_text.trim_start_matches("++"))
            } else if node_text.starts_with("--") {
                (UnOp::PreDec, node_text.trim_start_matches("--"))
            } else if node_text.ends_with("++") {
                (UnOp::PostInc, node_text.trim_end_matches("++"))
            } else {
                (UnOp::PostDec, node_text.trim_end_matches("--"))
            };
            let operand_node = node.named_child(0).unwrap_or(node);
            let _ = operand_src; // we use the node instead
            let operand = lower_expr(operand_node, src)?;
            Ok(IrExpr::UnOp {
                op,
                operand: Box::new(operand),
                ty: IrType::Unknown,
            })
        }

        // ── ternary ───────────────────────────────────────────────────────
        "ternary_expression" => {
            let cond = child_by_field(node, "condition")
                .map(|n| lower_expr(n, src))
                .transpose()?
                .unwrap_or(IrExpr::LitBool(true));
            let then_ = child_by_field(node, "consequence")
                .map(|n| lower_expr(n, src))
                .transpose()?
                .unwrap_or(IrExpr::LitNull);
            let else_ = child_by_field(node, "alternative")
                .map(|n| lower_expr(n, src))
                .transpose()?
                .unwrap_or(IrExpr::LitNull);
            Ok(IrExpr::Ternary {
                cond: Box::new(cond),
                then_: Box::new(then_),
                else_: Box::new(else_),
                ty: IrType::Unknown,
            })
        }

        // ── assignment ────────────────────────────────────────────────────
        "assignment_expression" => {
            let lhs_node = child_by_field(node, "left").unwrap_or(node);
            let rhs_node = child_by_field(node, "right").unwrap_or(node);
            let op_text = operator_text(node, src);
            let lhs = lower_expr(lhs_node, src)?;
            let rhs = lower_expr(rhs_node, src)?;
            if op_text == "=" {
                Ok(IrExpr::Assign {
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                    ty: IrType::Unknown,
                })
            } else {
                let op = compound_op(&op_text);
                Ok(IrExpr::CompoundAssign {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                    ty: IrType::Unknown,
                })
            }
        }

        // ── method invocation ─────────────────────────────────────────────
        "method_invocation" => {
            let method_name = child_by_field(node, "name")
                .map(|n| text(n, src).to_owned())
                .unwrap_or_default();
            let receiver = child_by_field(node, "object")
                .map(|n| lower_expr(n, src))
                .transpose()?
                .map(Box::new);
            // For EnumSet factory methods, the first arg is a class literal
            // (e.g. Color.class) used only as a type token — skip it.
            let skip_class_literals = receiver.as_ref().is_some_and(|r| {
                matches!(r.as_ref(), IrExpr::Var { name, .. } if name == "EnumSet")
                    && matches!(method_name.as_str(), "noneOf" | "allOf")
            });
            let args = child_by_field(node, "arguments")
                .map(|args_node| {
                    let mut c = args_node.walk();
                    args_node
                        .named_children(&mut c)
                        .filter(|n| !(skip_class_literals && n.kind() == "class_literal"))
                        .map(|n| lower_expr(n, src))
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();
            Ok(IrExpr::MethodCall {
                receiver,
                method_name,
                args,
                ty: IrType::Unknown,
            })
        }

        // ── object creation ───────────────────────────────────────────────
        "object_creation_expression" => {
            // For `new ArrayList<>()` or `new Wrapper<Integer>()`, the type node
            // is a generic_type.  Extract only the base class name so that
            // IrExpr::New.class is always a plain identifier like "ArrayList".
            let class = child_by_field(node, "type")
                .map(|n| {
                    if n.kind() == "generic_type" {
                        // First named child is the base type_identifier
                        n.named_child(0)
                            .map(|base| text(base, src))
                            .unwrap_or_else(|| text(n, src))
                    } else {
                        text(n, src)
                    }
                    .to_owned()
                })
                .unwrap_or_default();
            // EnumMap and EnumSet constructors take a class literal (e.g. Day.class)
            // as their sole argument — it is a type token and must be skipped.
            let skip_class_literals = matches!(class.as_str(), "EnumMap" | "EnumSet");
            let args = child_by_field(node, "arguments")
                .map(|args_node| {
                    let mut c = args_node.walk();
                    args_node
                        .named_children(&mut c)
                        .filter(|n| !(skip_class_literals && n.kind() == "class_literal"))
                        .map(|n| lower_expr(n, src))
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();

            // Anonymous inner class: has a class_body child with method implementations.
            // Use named_children() helper (which creates the cursor internally)
            // to avoid borrow conflicts with the node reference from find().
            let anon_body_opt = named_children(node)
                .into_iter()
                .find(|n| n.kind() == "class_body");
            if let Some(body_node) = anon_body_opt {
                let anon_name = next_anon_name();
                // Collect method declarations from the anonymous class body.
                let mut anon_methods: Vec<IrMethod> = Vec::new();
                let mut anon_fields: Vec<IrField> = Vec::new();
                let mut anon_ctors: Vec<IrConstructor> = Vec::new();
                let mut anon_cur = body_node.walk();
                for child in body_node.named_children(&mut anon_cur) {
                    match child.kind() {
                        "method_declaration" => {
                            let m = lower_method(child, src)?;
                            anon_methods.push(m);
                        }
                        "field_declaration" => {
                            let fs = lower_field(child, src)?;
                            anon_fields.extend(fs);
                        }
                        "constructor_declaration" => {
                            let ctor = lower_constructor(child, src)?;
                            anon_ctors.push(ctor);
                        }
                        _ => {}
                    }
                }
                // The anonymous class implements the target interface/superclass.
                let anon_cls = IrClass {
                    name: anon_name.clone(),
                    visibility: Visibility::PackagePrivate,
                    is_abstract: false,
                    is_final: true,
                    is_record: false,
                    type_params: vec![],
                    superclass: None,
                    interfaces: vec![class.clone()],
                    fields: anon_fields,
                    methods: anon_methods,
                    constructors: anon_ctors,
                };
                hoist_decl(IrDecl::Class(anon_cls));
                return Ok(IrExpr::New {
                    class: anon_name,
                    args,
                    ty: IrType::Unknown,
                });
            }

            Ok(IrExpr::New {
                class,
                args,
                ty: IrType::Unknown,
            })
        }

        // ── array creation ────────────────────────────────────────────────
        "array_creation_expression" => {
            let elem_ty = child_by_field(node, "type")
                .map(|n| lower_type(n, src))
                .unwrap_or(IrType::Unknown);
            let dim_nodes = children_by_field_name(node, "dimensions");
            // Total bracket count determines the full type depth, including
            // unspecified inner dimensions like `new int[3][]` (depth 2).
            let total_depth = dim_nodes.len().max(1);
            // Collect only the non-empty dimension expressions (those that
            // have explicit sizes, e.g. `[3]` but not `[]`).
            let dims: Vec<IrExpr> = dim_nodes
                .into_iter()
                .filter_map(|dim| dim.named_child(0))
                .map(|n| lower_expr(n, src))
                .collect::<Result<Vec<_>, _>>()?;
            // Build the resulting IrType with the full depth so that partially-
            // specified allocations like `new int[3][]` still produce the
            // correct `JArray<JArray<i32>>` type.
            let mut ty = elem_ty.clone();
            for _ in 0..total_depth {
                ty = IrType::Array(Box::new(ty));
            }
            if dims.len() >= 2 {
                // Multi-dimensional: use NewArrayMultiDim with provided dims.
                Ok(IrExpr::NewArrayMultiDim { elem_ty, dims, ty })
            } else {
                let len = dims.into_iter().next().unwrap_or(IrExpr::LitInt(0));
                Ok(IrExpr::NewArray {
                    elem_ty,
                    len: Box::new(len),
                    ty,
                })
            }
        }

        // ── array access ──────────────────────────────────────────────────
        "array_access" => {
            let array = child_by_field(node, "array")
                .map(|n| lower_expr(n, src))
                .transpose()?
                .unwrap_or(IrExpr::LitNull);
            let index = child_by_field(node, "index")
                .map(|n| lower_expr(n, src))
                .transpose()?
                .unwrap_or(IrExpr::LitInt(0));
            Ok(IrExpr::ArrayAccess {
                array: Box::new(array),
                index: Box::new(index),
                ty: IrType::Unknown,
            })
        }

        // ── field access ──────────────────────────────────────────────────
        "field_access" => {
            let receiver = child_by_field(node, "object")
                .map(|n| lower_expr(n, src))
                .transpose()?
                .unwrap_or(IrExpr::LitNull);
            let field_name = child_by_field(node, "field")
                .map(|n| text(n, src).to_owned())
                .unwrap_or_default();
            Ok(IrExpr::FieldAccess {
                receiver: Box::new(receiver),
                field_name,
                ty: IrType::Unknown,
            })
        }

        // ── cast ──────────────────────────────────────────────────────────
        "cast_expression" => {
            let target = child_by_field(node, "type")
                .map(|n| lower_type(n, src))
                .unwrap_or(IrType::Unknown);
            let expr_node =
                child_by_field(node, "value").unwrap_or(node.named_child(1).unwrap_or(node));
            let expr = lower_expr(expr_node, src)?;
            Ok(IrExpr::Cast {
                target,
                expr: Box::new(expr),
            })
        }

        // ── instanceof ────────────────────────────────────────────────────
        "instanceof_expression" => {
            let expr_node =
                child_by_field(node, "left").unwrap_or(node.named_child(0).unwrap_or(node));
            let check_type = child_by_field(node, "right")
                .map(|n| lower_type(n, src))
                .unwrap_or(IrType::Unknown);
            let binding = child_by_field(node, "name").map(|n| text(n, src).to_owned());
            let expr = lower_expr(expr_node, src)?;
            Ok(IrExpr::InstanceOf {
                expr: Box::new(expr),
                check_type,
                binding,
            })
        }

        // ── lambda expression ─────────────────────────────────────────────
        "lambda_expression" => {
            let params = if let Some(params_node) = child_by_field(node, "parameters") {
                match params_node.kind() {
                    "identifier" => vec![text(params_node, src).to_owned()],
                    "inferred_parameters" => {
                        let mut c = params_node.walk();
                        params_node
                            .named_children(&mut c)
                            .filter(|n| n.kind() == "identifier")
                            .map(|n| text(n, src).to_owned())
                            .collect()
                    }
                    "formal_parameters" => {
                        let mut c = params_node.walk();
                        params_node
                            .named_children(&mut c)
                            .filter(|n| {
                                n.kind() == "formal_parameter" || n.kind() == "spread_parameter"
                            })
                            .filter_map(|p| child_by_field(p, "name"))
                            .map(|n| text(n, src).to_owned())
                            .collect()
                    }
                    _ => vec![],
                }
            } else {
                vec![]
            };

            if let Some(body_node) = child_by_field(node, "body") {
                let (body_expr, body_stmts) = if body_node.kind() == "block" {
                    let mut stmts = lower_block(body_node, src)?;
                    // If the last statement is `return expr;`, pull it out as body
                    let final_expr = if let Some(IrStmt::Return(Some(_))) = stmts.last() {
                        if let IrStmt::Return(Some(e)) = stmts.pop().unwrap() {
                            e
                        } else {
                            unreachable!()
                        }
                    } else {
                        // Void block lambda — no return value
                        IrExpr::Unit
                    };
                    (final_expr, stmts)
                } else {
                    (lower_expr(body_node, src)?, vec![])
                };
                Ok(IrExpr::Lambda {
                    params,
                    body: Box::new(body_expr),
                    body_stmts,
                    ty: IrType::Unknown,
                })
            } else {
                Err(ParseError::Unsupported("lambda without body".into()))
            }
        }

        // ── class literal (e.g. String.class, int.class) ─────────────────
        "class_literal" => {
            // First named child is the type node (type_identifier, integral_type, etc.)
            let class_name = node
                .named_child(0)
                .map(|n| text(n, src).to_owned())
                .unwrap_or_else(|| "Object".to_owned());
            Ok(IrExpr::ClassLiteral { class_name })
        }

        // ── fallback ──────────────────────────────────────────────────────
        other => Err(ParseError::Unsupported(format!(
            "unsupported expression kind: {other}"
        ))),
    }
}

// ─── operator helpers ────────────────────────────────────────────────────────

fn operator_text(node: Node<'_>, src: &[u8]) -> String {
    // The operator is a non-named child with a single character or symbol
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !child.is_named() {
            let t = text(child, src);
            if !t.is_empty() && t.chars().all(|c| !c.is_alphanumeric() && c != '_') {
                return t.to_owned();
            }
        }
    }
    String::new()
}

fn text_to_binop(op: &str) -> BinOp {
    match op {
        "+" => BinOp::Add,
        "-" => BinOp::Sub,
        "*" => BinOp::Mul,
        "/" => BinOp::Div,
        "%" => BinOp::Rem,
        "&" => BinOp::BitAnd,
        "|" => BinOp::BitOr,
        "^" => BinOp::BitXor,
        "<<" => BinOp::Shl,
        ">>" => BinOp::Shr,
        ">>>" => BinOp::UShr,
        "&&" => BinOp::And,
        "||" => BinOp::Or,
        "==" => BinOp::Eq,
        "!=" => BinOp::Ne,
        "<" => BinOp::Lt,
        "<=" => BinOp::Le,
        ">" => BinOp::Gt,
        ">=" => BinOp::Ge,
        _ => BinOp::Add,
    }
}

fn compound_op(op: &str) -> BinOp {
    match op {
        "+=" => BinOp::Add,
        "-=" => BinOp::Sub,
        "*=" => BinOp::Mul,
        "/=" => BinOp::Div,
        "%=" => BinOp::Rem,
        "&=" => BinOp::BitAnd,
        "|=" => BinOp::BitOr,
        "^=" => BinOp::BitXor,
        "<<=" => BinOp::Shl,
        ">>=" => BinOp::Shr,
        ">>>=" => BinOp::UShr,
        _ => BinOp::Add,
    }
}

// ─── literal parsing helpers ─────────────────────────────────────────────────

fn parse_int_literal(s: &str) -> Option<i64> {
    let s = s.replace('_', "");
    if s.starts_with("0x") || s.starts_with("0X") {
        i64::from_str_radix(&s[2..], 16).ok()
    } else if s.starts_with("0b") || s.starts_with("0B") {
        i64::from_str_radix(&s[2..], 2).ok()
    } else if s.starts_with('0') && s.len() > 1 {
        i64::from_str_radix(&s[1..], 8).ok()
    } else {
        s.parse().ok()
    }
}

fn parse_hex_float(s: &str) -> f64 {
    // Basic support; full parsing is deferred
    s.replace("0x", "").replace("0X", "").parse().unwrap_or(0.0)
}

/// Strip common leading whitespace from a Java text block (JEP 378).
/// `inner` is the content between the opening and closing `"""` delimiters,
/// including the leading newline after the opening `"""`.
fn strip_text_block_indent(inner: &str) -> String {
    let lines: Vec<&str> = inner.split('\n').collect();
    if lines.is_empty() {
        return String::new();
    }
    // lines[0] is the remainder after opening """ (must be blank per spec); skip it
    let content_lines = &lines[1..];
    if content_lines.is_empty() {
        return String::new();
    }

    // The last line is the closing-delimiter line if it is whitespace-only
    let last_is_delim = content_lines.last().is_some_and(|l| l.trim().is_empty());

    // Compute common indent: consider non-blank content lines AND the
    // closing-delimiter line (even though it is all whitespace).
    let mut min_indent = usize::MAX;
    for (i, line) in content_lines.iter().enumerate() {
        let is_last = i == content_lines.len() - 1;
        if is_last && last_is_delim {
            min_indent = min_indent.min(line.len());
        } else if !line.trim().is_empty() {
            min_indent = min_indent.min(line.len() - line.trim_start().len());
        }
    }
    if min_indent == usize::MAX {
        min_indent = 0;
    }

    // Strip common indent from each line, normalizing CRLF but preserving trailing spaces
    let mut result_lines: Vec<&str> = Vec::new();
    for line in content_lines {
        if line.len() >= min_indent {
            let mut stripped = &line[min_indent..];
            // Normalize possible CRLF line endings by removing a trailing '\r'
            if stripped.ends_with('\r') {
                stripped = &stripped[..stripped.len() - 1];
            }
            result_lines.push(stripped);
        } else {
            result_lines.push("");
        }
    }

    // If closing """ was on its own line, drop that (now-empty) last element
    // and add a trailing newline to preserve the line break before it.
    if last_is_delim {
        result_lines.pop();
        let mut result = result_lines.join("\n");
        result.push('\n');
        result
    } else {
        result_lines.join("\n")
    }
}

fn unescape_java_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                Some('\'') => out.push('\''),
                Some('0') => out.push('\0'),
                Some(x) => {
                    out.push('\\');
                    out.push(x);
                }
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn unescape_java_char(s: &str) -> char {
    if s.starts_with('\\') {
        match s.chars().nth(1) {
            Some('n') => '\n',
            Some('t') => '\t',
            Some('r') => '\r',
            Some('\\') => '\\',
            Some('\'') => '\'',
            Some('0') => '\0',
            _ => '?',
        }
    } else {
        s.chars().next().unwrap_or('?')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELLO_WORLD: &str = include_str!("../../tests/java/HelloWorld.java");

    #[test]
    fn smoke_parse_hello_world() {
        let tree = parse_source(HELLO_WORLD).expect("HelloWorld.java must parse without errors");
        assert_eq!(tree.root_node().kind(), "program");
    }

    #[test]
    fn rejects_invalid_java() {
        let result = parse_source("this is not java @@@@");
        assert!(result.is_err(), "invalid Java should produce a parse error");
    }

    #[test]
    fn lower_hello_world_to_ir() {
        let module = parse_to_ir(HELLO_WORLD).expect("HelloWorld.java must lower to IR");
        assert_eq!(module.decls.len(), 1);
        if let IrDecl::Class(cls) = &module.decls[0] {
            assert_eq!(cls.name, "HelloWorld");
            assert_eq!(cls.methods.len(), 1);
            assert_eq!(cls.methods[0].name, "main");
        } else {
            panic!("expected a class declaration");
        }
    }
}
