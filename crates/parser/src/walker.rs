//! Java CST → `ir::IrModule` lowering pass.
//!
//! `parse_source` is the single public entry point. It:
//! 1. Drives the tree-sitter parser to produce a concrete syntax tree.
//! 2. Validates that the tree is error-free.
//! 3. Walks the CST and emits an [`ir::IrModule`].

use ir::decl::{IrClass, IrConstructor, IrField, IrInterface, IrMethod, IrParam, Visibility};
use ir::expr::{BinOp, UnOp};
use ir::stmt::{CatchClause, SwitchCase};
use ir::{IrDecl, IrExpr, IrModule, IrStmt, IrType};
use tree_sitter::{Language, Node, Parser};

use crate::{from_node, ParseError};

// ─── Public entry point ──────────────────────────────────────────────────────

/// Parse `source` as Java and lower it to an [`IrModule`].
///
/// Returns `Err` if the source is not valid Java or contains unsupported
/// constructs for the current stage.
pub fn parse_source(source: &str) -> Result<IrModule, ParseError> {
    let tree = parse_tree(source)?;
    let walker = Walker {
        source: source.as_bytes(),
    };
    walker.lower_program(tree.root_node())
}

// ─── Raw tree parsing ─────────────────────────────────────────────────────────

fn parse_tree(source: &str) -> Result<tree_sitter::Tree, ParseError> {
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

// ─── Walker ───────────────────────────────────────────────────────────────────

struct Walker<'src> {
    source: &'src [u8],
}

impl<'src> Walker<'src> {
    fn text(&self, node: Node) -> Result<&str, ParseError> {
        std::str::from_utf8(&self.source[node.start_byte()..node.end_byte()])
            .map_err(ParseError::Utf8)
    }

    // ── Program ──────────────────────────────────────────────────────────────

    fn lower_program(&self, root: Node) -> Result<IrModule, ParseError> {
        let mut module = IrModule::new("");
        let mut cursor = root.walk();
        for child in root.named_children(&mut cursor) {
            match child.kind() {
                "package_declaration" => {
                    if let Some(n) = child.named_child(0) {
                        module.package = self.text(n)?.to_owned();
                    }
                }
                "import_declaration" => {
                    if let Some(n) = child.named_child(0) {
                        module.imports.push(self.text(n)?.to_owned());
                    }
                }
                "class_declaration" => {
                    module.decls.push(IrDecl::Class(self.lower_class_decl(child)?));
                }
                "interface_declaration" => {
                    module
                        .decls
                        .push(IrDecl::Interface(self.lower_interface_decl(child)?));
                }
                _ => {}
            }
        }
        Ok(module)
    }

    // ── Class ─────────────────────────────────────────────────────────────────

    fn lower_class_decl(&self, node: Node) -> Result<IrClass, ParseError> {
        let name = node
            .child_by_field_name("name")
            .ok_or_else(|| ParseError::Unsupported("class without name".into()))?;
        let name = self.text(name)?.to_owned();

        let (visibility, is_abstract, is_final) = self.extract_modifiers(node);
        let superclass = node
            .child_by_field_name("superclass")
            .and_then(|n| n.named_child(0))
            .map(|n| self.text(n).map(|s| s.to_owned()))
            .transpose()?;

        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut constructors = Vec::new();

        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.named_children(&mut cursor) {
                match child.kind() {
                    "method_declaration" => methods.push(self.lower_method(child)?),
                    "constructor_declaration" => {
                        constructors.push(self.lower_constructor(child, &name)?)
                    }
                    "field_declaration" => fields.extend(self.lower_field_decl(child)?),
                    _ => {}
                }
            }
        }

        Ok(IrClass {
            name,
            visibility,
            is_abstract,
            is_final,
            type_params: vec![],
            superclass,
            interfaces: vec![],
            fields,
            methods,
            constructors,
        })
    }

    fn lower_interface_decl(&self, node: Node) -> Result<IrInterface, ParseError> {
        let name = node
            .child_by_field_name("name")
            .ok_or_else(|| ParseError::Unsupported("interface without name".into()))?;
        let name = self.text(name)?.to_owned();
        let (visibility, _, _) = self.extract_modifiers(node);

        let mut methods = Vec::new();
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.named_children(&mut cursor) {
                if child.kind() == "method_declaration" {
                    methods.push(self.lower_method(child)?);
                }
            }
        }

        Ok(IrInterface {
            name,
            visibility,
            type_params: vec![],
            extends: vec![],
            methods,
        })
    }

    // ── Modifiers ─────────────────────────────────────────────────────────────

    fn extract_modifiers(&self, node: Node) -> (Visibility, bool, bool) {
        let mut visibility = Visibility::PackagePrivate;
        let mut is_abstract = false;
        let mut is_final = false;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                let mut mcursor = child.walk();
                for m in child.children(&mut mcursor) {
                    match m.kind() {
                        "public" => visibility = Visibility::Public,
                        "protected" => visibility = Visibility::Protected,
                        "private" => visibility = Visibility::Private,
                        "abstract" => is_abstract = true,
                        "final" => is_final = true,
                        _ => {}
                    }
                }
            }
        }
        (visibility, is_abstract, is_final)
    }

    fn is_static(&self, node: Node) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                let mut mcursor = child.walk();
                for m in child.children(&mut mcursor) {
                    if m.kind() == "static" {
                        return true;
                    }
                }
            }
        }
        false
    }

    // ── Methods & constructors ────────────────────────────────────────────────

    fn lower_method(&self, node: Node) -> Result<IrMethod, ParseError> {
        let name_node = node
            .child_by_field_name("name")
            .ok_or_else(|| ParseError::Unsupported("method without name".into()))?;
        let name = self.text(name_node)?.to_owned();

        let type_node = node
            .child_by_field_name("type")
            .ok_or_else(|| ParseError::Unsupported("method without return type".into()))?;
        let return_ty = self.lower_type(type_node)?;

        let (visibility, is_abstract, is_final) = self.extract_modifiers(node);
        let is_static = self.is_static(node);

        let params = node
            .child_by_field_name("parameters")
            .map(|n| self.lower_formal_params(n))
            .transpose()?
            .unwrap_or_default();

        let body = node
            .child_by_field_name("body")
            .map(|n| self.lower_block(n))
            .transpose()?;

        Ok(IrMethod {
            name,
            visibility,
            is_static,
            is_abstract,
            is_final,
            type_params: vec![],
            params,
            return_ty,
            body,
            throws: vec![],
        })
    }

    fn lower_constructor(&self, node: Node, class_name: &str) -> Result<IrConstructor, ParseError> {
        let (visibility, _, _) = self.extract_modifiers(node);

        let params = node
            .child_by_field_name("parameters")
            .map(|n| self.lower_formal_params(n))
            .transpose()?
            .unwrap_or_default();

        let body = node
            .child_by_field_name("body")
            .map(|n| self.lower_block(n))
            .transpose()?
            .unwrap_or_default();

        let _ = class_name; // used by callers for context
        Ok(IrConstructor {
            visibility,
            params,
            body,
            throws: vec![],
        })
    }

    fn lower_field_decl(&self, node: Node) -> Result<Vec<IrField>, ParseError> {
        let type_node = node
            .child_by_field_name("type")
            .ok_or_else(|| ParseError::Unsupported("field without type".into()))?;
        let ty = self.lower_type(type_node)?;
        let (visibility, _, is_final) = self.extract_modifiers(node);
        let is_static = self.is_static(node);

        let mut fields = Vec::new();
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() == "variable_declarator" {
                let name_node = child
                    .child_by_field_name("name")
                    .ok_or_else(|| ParseError::Unsupported("variable declarator without name".into()))?;
                let name = self.resolve_declarator_id(name_node)?;
                let init = child
                    .child_by_field_name("value")
                    .map(|n| self.lower_expr(n))
                    .transpose()?;
                fields.push(IrField {
                    name,
                    ty: ty.clone(),
                    visibility,
                    is_static,
                    is_final,
                    init,
                });
            }
        }
        Ok(fields)
    }

    fn lower_formal_params(&self, node: Node) -> Result<Vec<IrParam>, ParseError> {
        let mut params = Vec::new();
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            match child.kind() {
                "formal_parameter" | "spread_parameter" => {
                    let type_node = child
                        .child_by_field_name("type")
                        .ok_or_else(|| ParseError::Unsupported("param without type".into()))?;
                    let ty = self.lower_type(type_node)?;

                    let name_node = child
                        .child_by_field_name("name")
                        .ok_or_else(|| ParseError::Unsupported("param without name".into()))?;
                    let name = self.resolve_declarator_id(name_node)?;
                    let is_varargs = child.kind() == "spread_parameter";
                    params.push(IrParam {
                        name,
                        ty,
                        is_varargs,
                    });
                }
                _ => {}
            }
        }
        Ok(params)
    }

    /// Resolve a `variable_declarator_id` or plain `identifier` to a name string.
    fn resolve_declarator_id(&self, node: Node) -> Result<String, ParseError> {
        if node.kind() == "variable_declarator_id" {
            // e.g. `int[] arr` — first named child is the identifier
            let id = node
                .named_child(0)
                .ok_or_else(|| ParseError::Unsupported("empty variable_declarator_id".into()))?;
            Ok(self.text(id)?.to_owned())
        } else {
            Ok(self.text(node)?.to_owned())
        }
    }

    // ── Types ─────────────────────────────────────────────────────────────────

    fn lower_type(&self, node: Node) -> Result<IrType, ParseError> {
        match node.kind() {
            "integral_type" | "floating_point_type" | "boolean_type" | "void_type" => {
                let text = self.text(node)?;
                Ok(from_node::primitive_keyword_to_ir_type(text).unwrap_or(IrType::Unknown))
            }
            "type_identifier" => {
                let text = self.text(node)?;
                Ok(match text {
                    "String" | "java.lang.String" => IrType::String,
                    _ => IrType::Class(text.to_owned()),
                })
            }
            "array_type" => {
                let elem_node = node
                    .child_by_field_name("element")
                    .ok_or_else(|| ParseError::Unsupported("array_type without element".into()))?;
                Ok(IrType::Array(Box::new(self.lower_type(elem_node)?)))
            }
            "generic_type" => {
                if let Some(name_node) = node.named_child(0) {
                    let text = self.text(name_node)?;
                    Ok(IrType::Class(text.to_owned()))
                } else {
                    Ok(IrType::Unknown)
                }
            }
            "scoped_type_identifier" => Ok(IrType::Class(self.text(node)?.to_owned())),
            _ => {
                let text = self.text(node)?;
                Ok(from_node::node_kind_to_ir_type(node.kind(), text))
            }
        }
    }

    // ── Blocks & statements ───────────────────────────────────────────────────

    fn lower_block(&self, node: Node) -> Result<Vec<IrStmt>, ParseError> {
        let mut stmts = Vec::new();
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if let Some(stmt) = self.lower_stmt_opt(child)? {
                stmts.push(stmt);
            }
        }
        Ok(stmts)
    }

    fn lower_stmt_to_block(&self, node: Node) -> Result<Vec<IrStmt>, ParseError> {
        if node.kind() == "block" {
            self.lower_block(node)
        } else {
            Ok(self.lower_stmt_opt(node)?.into_iter().collect())
        }
    }

    fn lower_stmt_opt(&self, node: Node) -> Result<Option<IrStmt>, ParseError> {
        let stmt = match node.kind() {
            "local_variable_declaration" => {
                let type_node = node
                    .child_by_field_name("type")
                    .ok_or_else(|| ParseError::Unsupported("local var without type".into()))?;
                let ty = self.lower_type(type_node)?;
                let mut stmts = Vec::new();
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    if child.kind() == "variable_declarator" {
                        let name_node = child.child_by_field_name("name").ok_or_else(|| {
                            ParseError::Unsupported("variable declarator without name".into())
                        })?;
                        let name = self.resolve_declarator_id(name_node)?;
                        let init = child
                            .child_by_field_name("value")
                            .map(|n| self.lower_expr(n))
                            .transpose()?;
                        stmts.push(IrStmt::LocalVar {
                            name,
                            ty: ty.clone(),
                            init,
                        });
                    }
                }
                match stmts.len() {
                    0 => return Ok(None),
                    1 => stmts.pop().unwrap(),
                    _ => IrStmt::Block(stmts),
                }
            }

            "expression_statement" => {
                let expr_node = node.named_child(0).ok_or_else(|| {
                    ParseError::Unsupported("empty expression statement".into())
                })?;
                IrStmt::Expr(self.lower_expr(expr_node)?)
            }

            "if_statement" => {
                let cond_node = node
                    .child_by_field_name("condition")
                    .ok_or_else(|| ParseError::Unsupported("if without condition".into()))?;
                let cond = self.lower_paren_expr(cond_node)?;
                let then_node = node
                    .child_by_field_name("consequence")
                    .ok_or_else(|| ParseError::Unsupported("if without consequence".into()))?;
                let then_ = self.lower_stmt_to_block(then_node)?;
                let else_ = node
                    .child_by_field_name("alternative")
                    .map(|n| {
                        // `alternative` may be an `else_clause` node wrapping the statement
                        let stmt_node = if n.kind() == "else_clause" {
                            n.named_child(0).unwrap_or(n)
                        } else {
                            n
                        };
                        self.lower_stmt_to_block(stmt_node)
                    })
                    .transpose()?;
                IrStmt::If { cond, then_, else_ }
            }

            "while_statement" => {
                let cond = self.lower_paren_expr(
                    node.child_by_field_name("condition")
                        .ok_or_else(|| ParseError::Unsupported("while without condition".into()))?,
                )?;
                let body = self.lower_stmt_to_block(
                    node.child_by_field_name("body")
                        .ok_or_else(|| ParseError::Unsupported("while without body".into()))?,
                )?;
                IrStmt::While { cond, body }
            }

            "do_statement" => {
                let body = self.lower_stmt_to_block(
                    node.child_by_field_name("body")
                        .ok_or_else(|| ParseError::Unsupported("do-while without body".into()))?,
                )?;
                let cond = self.lower_paren_expr(
                    node.child_by_field_name("condition").ok_or_else(|| {
                        ParseError::Unsupported("do-while without condition".into())
                    })?,
                )?;
                IrStmt::DoWhile { body, cond }
            }

            "for_statement" => self.lower_for_stmt(node)?,

            "enhanced_for_statement" => {
                let var_ty = self.lower_type(
                    node.child_by_field_name("type")
                        .ok_or_else(|| ParseError::Unsupported("for-each without type".into()))?,
                )?;
                let var = self.text(
                    node.child_by_field_name("name")
                        .ok_or_else(|| ParseError::Unsupported("for-each without name".into()))?,
                )?
                .to_owned();
                let iterable = self.lower_expr(
                    node.child_by_field_name("value")
                        .ok_or_else(|| ParseError::Unsupported("for-each without value".into()))?,
                )?;
                let body = self.lower_stmt_to_block(
                    node.child_by_field_name("body")
                        .ok_or_else(|| ParseError::Unsupported("for-each without body".into()))?,
                )?;
                IrStmt::ForEach {
                    var,
                    var_ty,
                    iterable,
                    body,
                }
            }

            "switch_statement" | "switch_expression" => self.lower_switch(node)?,

            "return_statement" => {
                IrStmt::Return(node.named_child(0).map(|n| self.lower_expr(n)).transpose()?)
            }

            "break_statement" => IrStmt::Break(
                node.named_child(0)
                    .map(|n| self.text(n).map(|s| s.to_owned()))
                    .transpose()?,
            ),

            "continue_statement" => IrStmt::Continue(
                node.named_child(0)
                    .map(|n| self.text(n).map(|s| s.to_owned()))
                    .transpose()?,
            ),

            "throw_statement" => {
                let expr_node = node
                    .named_child(0)
                    .ok_or_else(|| ParseError::Unsupported("throw without expression".into()))?;
                IrStmt::Throw(self.lower_expr(expr_node)?)
            }

            "try_statement" | "try_with_resources_statement" => self.lower_try(node)?,

            "block" => IrStmt::Block(self.lower_block(node)?),

            "labeled_statement" => {
                // label: stmt — just lower the body
                if let Some(body_node) = node.named_child(1) {
                    if let Some(s) = self.lower_stmt_opt(body_node)? {
                        s
                    } else {
                        return Ok(None);
                    }
                } else {
                    return Ok(None);
                }
            }

            // Skip: assert, empty_statement, line_comment, etc.
            _ => return Ok(None),
        };
        Ok(Some(stmt))
    }

    // ── Statement helpers ─────────────────────────────────────────────────────

    fn lower_paren_expr(&self, node: Node) -> Result<IrExpr, ParseError> {
        if node.kind() == "parenthesized_expression" {
            let inner = node.named_child(0).ok_or_else(|| {
                ParseError::Unsupported("empty parenthesized expression".into())
            })?;
            self.lower_expr(inner)
        } else {
            self.lower_expr(node)
        }
    }

    fn lower_for_stmt(&self, node: Node) -> Result<IrStmt, ParseError> {
        let init = node
            .child_by_field_name("init")
            .map(|n| self.lower_stmt_opt(n))
            .transpose()?
            .flatten()
            .map(Box::new);

        let cond = node
            .child_by_field_name("condition")
            .map(|n| self.lower_expr(n))
            .transpose()?;

        let mut update = Vec::new();
        let mut cursor = node.walk();
        for child in node.children_by_field_name("update", &mut cursor) {
            update.push(self.lower_expr(child)?);
        }

        let body = self.lower_stmt_to_block(
            node.child_by_field_name("body")
                .ok_or_else(|| ParseError::Unsupported("for without body".into()))?,
        )?;

        Ok(IrStmt::For {
            init,
            cond,
            update,
            body,
        })
    }

    fn lower_switch(&self, node: Node) -> Result<IrStmt, ParseError> {
        let cond_node = node
            .child_by_field_name("condition")
            .ok_or_else(|| ParseError::Unsupported("switch without condition".into()))?;
        let expr = self.lower_paren_expr(cond_node)?;

        let mut cases: Vec<SwitchCase> = Vec::new();
        let mut default: Option<Vec<IrStmt>> = None;

        if let Some(body_node) = node.child_by_field_name("body") {
            let mut cursor = body_node.walk();
            for group in body_node.named_children(&mut cursor) {
                if group.kind() != "switch_block_statement_group" {
                    continue;
                }

                let mut case_values: Vec<IrExpr> = Vec::new();
                let mut is_default = false;
                let mut stmts: Vec<IrStmt> = Vec::new();
                let mut past_labels = false;

                let mut gcursor = group.walk();
                for child in group.named_children(&mut gcursor) {
                    if !past_labels && child.kind() == "switch_label" {
                        if let Some(val_node) = child.child_by_field_name("value") {
                            case_values.push(self.lower_expr(val_node)?);
                        } else {
                            is_default = true;
                        }
                    } else {
                        past_labels = true;
                        if let Some(s) = self.lower_stmt_opt(child)? {
                            stmts.push(s);
                        }
                    }
                }

                if is_default {
                    default = Some(stmts);
                } else {
                    for val in case_values {
                        cases.push(SwitchCase {
                            value: val,
                            body: stmts.clone(),
                        });
                    }
                }
            }
        }

        Ok(IrStmt::Switch {
            expr,
            cases,
            default,
        })
    }

    fn lower_try(&self, node: Node) -> Result<IrStmt, ParseError> {
        let body = self.lower_block(
            node.child_by_field_name("body")
                .ok_or_else(|| ParseError::Unsupported("try without body".into()))?,
        )?;

        let mut catches = Vec::new();
        let mut finally = None;

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            match child.kind() {
                "catch_clause" => {
                    let mut exception_types = Vec::new();
                    let mut var_name = "e".to_owned();

                    if let Some(param) = child.child_by_field_name("parameter") {
                        // catch_formal_parameter: type name
                        // The name is the last named child; types are before it
                        let nc = param.named_child_count();
                        if nc > 0 {
                            let name_node = param.named_child(nc - 1).unwrap();
                            var_name = self.resolve_declarator_id(name_node)?;
                            let mut pcursor = param.walk();
                            for c in param.named_children(&mut pcursor) {
                                match c.kind() {
                                    "catch_type" | "type_identifier" => {
                                        // catch_type may contain multiple types (multi-catch)
                                        if c.kind() == "catch_type" {
                                            let mut tcursor = c.walk();
                                            for t in c.named_children(&mut tcursor) {
                                                exception_types.push(self.text(t)?.to_owned());
                                            }
                                        } else {
                                            exception_types.push(self.text(c)?.to_owned());
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    if exception_types.is_empty() {
                        exception_types.push("Exception".into());
                    }

                    let catch_body = child
                        .child_by_field_name("body")
                        .map(|b| self.lower_block(b))
                        .transpose()?
                        .unwrap_or_default();

                    catches.push(CatchClause {
                        exception_types,
                        var: var_name,
                        body: catch_body,
                    });
                }
                "finally_clause" => {
                    if let Some(body_node) = child.named_child(0) {
                        finally = Some(self.lower_block(body_node)?);
                    }
                }
                _ => {}
            }
        }

        Ok(IrStmt::TryCatch {
            body,
            catches,
            finally,
        })
    }

    // ── Expressions ───────────────────────────────────────────────────────────

    fn lower_expr(&self, node: Node) -> Result<IrExpr, ParseError> {
        match node.kind() {
            // ── Literals ──────────────────────────────────────────────────────
            "decimal_integer_literal"
            | "hex_integer_literal"
            | "octal_integer_literal"
            | "binary_integer_literal" => {
                let raw = self.text(node)?;
                let is_long = raw.ends_with('l') || raw.ends_with('L');
                let clean = raw
                    .trim_end_matches(['l', 'L'])
                    .replace('_', "");
                let val: i64 = if clean.starts_with("0x") || clean.starts_with("0X") {
                    i64::from_str_radix(&clean[2..], 16).unwrap_or(0)
                } else if clean.starts_with("0b") || clean.starts_with("0B") {
                    i64::from_str_radix(&clean[2..], 2).unwrap_or(0)
                } else if clean.starts_with('0') && clean.len() > 1 {
                    i64::from_str_radix(&clean[1..], 8).unwrap_or(0)
                } else {
                    clean.parse().unwrap_or(0)
                };
                Ok(if is_long {
                    IrExpr::LitLong(val)
                } else {
                    IrExpr::LitInt(val)
                })
            }

            "decimal_floating_point_literal" | "hex_floating_point_literal" => {
                let raw = self.text(node)?;
                let is_float = raw.ends_with('f') || raw.ends_with('F');
                let clean = raw
                    .trim_end_matches(['f', 'F', 'd', 'D'])
                    .replace('_', "");
                let val = clean.parse::<f64>().unwrap_or(0.0);
                Ok(if is_float {
                    IrExpr::LitFloat(val)
                } else {
                    IrExpr::LitDouble(val)
                })
            }

            "true" => Ok(IrExpr::LitBool(true)),
            "false" => Ok(IrExpr::LitBool(false)),
            "null_literal" => Ok(IrExpr::LitNull),

            "string_literal" => {
                let raw = self.text(node)?;
                let inner = &raw[1..raw.len() - 1]; // strip surrounding quotes
                Ok(IrExpr::LitString(unescape_java_string(inner)))
            }

            "character_literal" => {
                let raw = self.text(node)?;
                let inner = &raw[1..raw.len() - 1];
                Ok(IrExpr::LitChar(unescape_java_char(inner)))
            }

            // ── Identifier & wrappers ─────────────────────────────────────────
            "identifier" => Ok(IrExpr::Var {
                name: self.text(node)?.to_owned(),
                ty: IrType::Unknown,
            }),

            "this" => Ok(IrExpr::Var {
                name: "this".into(),
                ty: IrType::Unknown,
            }),

            "super" => Ok(IrExpr::Var {
                name: "super".into(),
                ty: IrType::Unknown,
            }),

            "parenthesized_expression" => self.lower_paren_expr(node),

            // ── Binary & unary operators ──────────────────────────────────────
            "binary_expression" => {
                let left = node
                    .child_by_field_name("left")
                    .ok_or_else(|| ParseError::Unsupported("binary_expression without left".into()))?;
                let right = node.child_by_field_name("right").ok_or_else(|| {
                    ParseError::Unsupported("binary_expression without right".into())
                })?;
                let op_node = node.child_by_field_name("operator").ok_or_else(|| {
                    ParseError::Unsupported("binary_expression without operator".into())
                })?;
                let op = match self.text(op_node)? {
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
                    other => {
                        return Err(ParseError::Unsupported(format!("unknown binary op: {other}")))
                    }
                };
                Ok(IrExpr::BinOp {
                    op,
                    lhs: Box::new(self.lower_expr(left)?),
                    rhs: Box::new(self.lower_expr(right)?),
                    ty: IrType::Unknown,
                })
            }

            "unary_expression" => {
                let operand_node = node
                    .child_by_field_name("operand")
                    .ok_or_else(|| ParseError::Unsupported("unary without operand".into()))?;
                let op_node = node
                    .child_by_field_name("operator")
                    .ok_or_else(|| ParseError::Unsupported("unary without operator".into()))?;
                let op_text = self.text(op_node)?;
                if op_text == "+" {
                    return self.lower_expr(operand_node); // unary + is a no-op
                }
                let op = match op_text {
                    "-" => UnOp::Neg,
                    "!" => UnOp::Not,
                    "~" => UnOp::BitNot,
                    other => {
                        return Err(ParseError::Unsupported(format!("unknown unary op: {other}")))
                    }
                };
                Ok(IrExpr::UnOp {
                    op,
                    operand: Box::new(self.lower_expr(operand_node)?),
                    ty: IrType::Unknown,
                })
            }

            "update_expression" => {
                // Prefix if first child (including anonymous) is the operator
                let is_prefix = node
                    .child(0)
                    .map(|n| n.kind() == "++" || n.kind() == "--")
                    .unwrap_or(false);
                // The expression being updated is the only named child
                let operand_node = node
                    .named_child(0)
                    .ok_or_else(|| ParseError::Unsupported("update without operand".into()))?;
                let op_kind = {
                    let mut found = "++";
                    let mut cur = node.walk();
                    for c in node.children(&mut cur) {
                        if c.kind() == "++" || c.kind() == "--" {
                            found = c.kind();
                            break;
                        }
                    }
                    found
                };
                let op = match (is_prefix, op_kind) {
                    (true, "++") => UnOp::PreInc,
                    (true, "--") => UnOp::PreDec,
                    (false, "++") => UnOp::PostInc,
                    _ => UnOp::PostDec,
                };
                Ok(IrExpr::UnOp {
                    op,
                    operand: Box::new(self.lower_expr(operand_node)?),
                    ty: IrType::Unknown,
                })
            }

            "ternary_expression" => Ok(IrExpr::Ternary {
                cond: Box::new(self.lower_expr(
                    node.child_by_field_name("condition").ok_or_else(|| {
                        ParseError::Unsupported("ternary without condition".into())
                    })?,
                )?),
                then_: Box::new(self.lower_expr(
                    node.child_by_field_name("consequence").ok_or_else(|| {
                        ParseError::Unsupported("ternary without consequence".into())
                    })?,
                )?),
                else_: Box::new(self.lower_expr(
                    node.child_by_field_name("alternative").ok_or_else(|| {
                        ParseError::Unsupported("ternary without alternative".into())
                    })?,
                )?),
                ty: IrType::Unknown,
            }),

            "assignment_expression" => {
                let left = node
                    .child_by_field_name("left")
                    .ok_or_else(|| ParseError::Unsupported("assignment without left".into()))?;
                let right = node
                    .child_by_field_name("right")
                    .ok_or_else(|| ParseError::Unsupported("assignment without right".into()))?;
                let op_text = self.text(
                    node.child_by_field_name("operator")
                        .ok_or_else(|| ParseError::Unsupported("assignment without operator".into()))?,
                )?;
                if op_text == "=" {
                    Ok(IrExpr::Assign {
                        lhs: Box::new(self.lower_expr(left)?),
                        rhs: Box::new(self.lower_expr(right)?),
                        ty: IrType::Unknown,
                    })
                } else {
                    let bin_op = match op_text {
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
                        other => {
                            return Err(ParseError::Unsupported(format!(
                                "unknown compound assign: {other}"
                            )))
                        }
                    };
                    Ok(IrExpr::CompoundAssign {
                        op: bin_op,
                        lhs: Box::new(self.lower_expr(left)?),
                        rhs: Box::new(self.lower_expr(right)?),
                        ty: IrType::Unknown,
                    })
                }
            }

            // ── Calls & construction ──────────────────────────────────────────
            "method_invocation" => {
                let name_node = node.child_by_field_name("name").ok_or_else(|| {
                    ParseError::Unsupported("method_invocation without name".into())
                })?;
                let method_name = self.text(name_node)?.to_owned();
                let receiver = node
                    .child_by_field_name("object")
                    .map(|n| self.lower_expr(n))
                    .transpose()?
                    .map(Box::new);
                let args = node
                    .child_by_field_name("arguments")
                    .map(|n| self.lower_argument_list(n))
                    .transpose()?
                    .unwrap_or_default();
                Ok(IrExpr::MethodCall {
                    receiver,
                    method_name,
                    args,
                    ty: IrType::Unknown,
                })
            }

            "object_creation_expression" => {
                let type_node = node
                    .child_by_field_name("type")
                    .ok_or_else(|| ParseError::Unsupported("new without type".into()))?;
                let class = self.text(type_node)?.to_owned();
                let args = node
                    .child_by_field_name("arguments")
                    .map(|n| self.lower_argument_list(n))
                    .transpose()?
                    .unwrap_or_default();
                Ok(IrExpr::New {
                    ty: IrType::Class(class.clone()),
                    class,
                    args,
                })
            }

            "array_creation_expression" => {
                let type_node = node
                    .child_by_field_name("type")
                    .ok_or_else(|| ParseError::Unsupported("array creation without type".into()))?;
                let elem_ty = self.lower_type(type_node)?;

                // Grab the first dimensions_expr child for the length
                let mut len_expr = IrExpr::LitInt(0);
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    if child.kind() == "dimensions_expr" {
                        if let Some(e) = child.named_child(0) {
                            len_expr = self.lower_expr(e)?;
                        }
                        break;
                    }
                }

                let arr_ty = IrType::Array(Box::new(elem_ty.clone()));
                Ok(IrExpr::NewArray {
                    elem_ty,
                    len: Box::new(len_expr),
                    ty: arr_ty,
                })
            }

            // ── Field & array access ──────────────────────────────────────────
            "field_access" => {
                let object = node
                    .child_by_field_name("object")
                    .ok_or_else(|| ParseError::Unsupported("field_access without object".into()))?;
                let field = node
                    .child_by_field_name("field")
                    .ok_or_else(|| ParseError::Unsupported("field_access without field".into()))?;
                Ok(IrExpr::FieldAccess {
                    receiver: Box::new(self.lower_expr(object)?),
                    field_name: self.text(field)?.to_owned(),
                    ty: IrType::Unknown,
                })
            }

            "array_access" => {
                let array = node
                    .child_by_field_name("array")
                    .ok_or_else(|| ParseError::Unsupported("array_access without array".into()))?;
                let index = node
                    .child_by_field_name("index")
                    .ok_or_else(|| ParseError::Unsupported("array_access without index".into()))?;
                Ok(IrExpr::ArrayAccess {
                    array: Box::new(self.lower_expr(array)?),
                    index: Box::new(self.lower_expr(index)?),
                    ty: IrType::Unknown,
                })
            }

            // ── Cast & instanceof ─────────────────────────────────────────────
            "cast_expression" => {
                let type_node = node
                    .child_by_field_name("type")
                    .ok_or_else(|| ParseError::Unsupported("cast without type".into()))?;
                let target = self.lower_type(type_node)?;
                let expr_node = node
                    .child_by_field_name("value")
                    .ok_or_else(|| ParseError::Unsupported("cast without value".into()))?;
                Ok(IrExpr::Cast {
                    target,
                    expr: Box::new(self.lower_expr(expr_node)?),
                })
            }

            "instanceof_expression" => {
                let left = node
                    .child_by_field_name("left")
                    .ok_or_else(|| ParseError::Unsupported("instanceof without left".into()))?;
                let right = node
                    .child_by_field_name("right")
                    .ok_or_else(|| ParseError::Unsupported("instanceof without right".into()))?;
                Ok(IrExpr::InstanceOf {
                    expr: Box::new(self.lower_expr(left)?),
                    check_type: self.lower_type(right)?,
                })
            }

            other => Err(ParseError::Unsupported(format!(
                "unsupported expression kind: {other}"
            ))),
        }
    }

    fn lower_argument_list(&self, node: Node) -> Result<Vec<IrExpr>, ParseError> {
        let mut args = Vec::new();
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            args.push(self.lower_expr(child)?);
        }
        Ok(args)
    }
}

// ─── String unescaping ────────────────────────────────────────────────────────

fn unescape_java_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('\'') => result.push('\''),
                Some('0') => result.push('\0'),
                Some(c) => {
                    result.push('\\');
                    result.push(c);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
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
            _ => s.chars().next().unwrap_or('?'),
        }
    } else {
        s.chars().next().unwrap_or('?')
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const HELLO_WORLD: &str = include_str!("../../tests/java/HelloWorld.java");

    #[test]
    fn smoke_parse_hello_world() {
        let module = parse_source(HELLO_WORLD).expect("HelloWorld.java must parse without errors");
        assert_eq!(module.decls.len(), 1, "should have exactly one class");
        if let IrDecl::Class(cls) = &module.decls[0] {
            assert_eq!(cls.name, "HelloWorld");
            assert_eq!(cls.methods.len(), 1);
            assert_eq!(cls.methods[0].name, "main");
            assert!(cls.methods[0].is_static);
        } else {
            panic!("expected a class declaration");
        }
    }

    #[test]
    fn rejects_invalid_java() {
        let result = parse_source("this is not java @@@@");
        assert!(result.is_err(), "invalid Java should produce a parse error");
    }

    #[test]
    fn parses_arithmetic_method() {
        let src = r#"
            public class Calc {
                public static int add(int a, int b) {
                    return a + b;
                }
            }
        "#;
        let module = parse_source(src).unwrap();
        if let IrDecl::Class(cls) = &module.decls[0] {
            assert_eq!(cls.name, "Calc");
            assert_eq!(cls.methods[0].name, "add");
            assert_eq!(cls.methods[0].params.len(), 2);
        }
    }

    #[test]
    fn parses_if_while() {
        let src = r#"
            public class Loops {
                public static void run() {
                    int i = 0;
                    while (i < 10) {
                        if (i == 5) { break; }
                        i++;
                    }
                }
            }
        "#;
        let module = parse_source(src).unwrap();
        assert_eq!(module.decls.len(), 1);
    }
}
