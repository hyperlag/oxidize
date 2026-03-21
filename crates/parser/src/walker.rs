//! Top-level parse entry point.

use tree_sitter::{Language, Parser};

use crate::ParseError;

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
        // Find the first ERROR node and report its byte offset.
        let error_node = find_first_error(tree.root_node());
        let offset = error_node.map(|n| n.start_byte()).unwrap_or(0);
        return Err(ParseError::SyntaxError {
            offset,
            message: "tree-sitter reported a parse error".into(),
        });
    }

    Ok(tree)
}

fn find_first_error(node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
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
}
