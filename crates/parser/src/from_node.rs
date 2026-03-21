//! `From` / `TryFrom` conversions between tree-sitter node kinds and IR nodes.
//!
//! These are the boilerplate conversions generated (and reviewed) with Copilot
//! assistance as specified in the Stage 0 Copilot task.
//!
//! At Stage 0, only the type-keyword mappings are implemented since the full
//! AST walker lives in Stage 1. Additional converters are added incrementally
//! as the walker is extended.

use ir::IrType;

/// Map a Java primitive-type keyword to the corresponding [`IrType`].
///
/// Returns `None` if the keyword is not a recognised primitive.
pub fn primitive_keyword_to_ir_type(keyword: &str) -> Option<IrType> {
    match keyword {
        "boolean_type" | "boolean" => Some(IrType::Bool),
        "byte_type" | "byte" => Some(IrType::Byte),
        "short_type" | "short" => Some(IrType::Short),
        "int_type" | "int" => Some(IrType::Int),
        "long_type" | "long" => Some(IrType::Long),
        "float_type" | "float" => Some(IrType::Float),
        "double_type" | "double" => Some(IrType::Double),
        "char_type" | "char" => Some(IrType::Char),
        "void_type" | "void" => Some(IrType::Void),
        _ => None,
    }
}

/// Convert a tree-sitter node kind string for a type reference into an
/// [`IrType`].  Only handles primitives and `String` at Stage 0.
pub fn node_kind_to_ir_type(kind: &str, text: &str) -> IrType {
    if let Some(prim) = primitive_keyword_to_ir_type(kind) {
        return prim;
    }
    match text {
        "String" | "java.lang.String" => IrType::String,
        _ => IrType::Class(text.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_primitives_round_trip() {
        let cases = [
            ("boolean", IrType::Bool),
            ("byte", IrType::Byte),
            ("short", IrType::Short),
            ("int", IrType::Int),
            ("long", IrType::Long),
            ("float", IrType::Float),
            ("double", IrType::Double),
            ("char", IrType::Char),
            ("void", IrType::Void),
        ];
        for (kw, expected) in &cases {
            assert_eq!(primitive_keyword_to_ir_type(kw).as_ref(), Some(expected));
        }
    }

    #[test]
    fn unknown_keyword_returns_none() {
        assert!(primitive_keyword_to_ir_type("Object").is_none());
    }

    #[test]
    fn string_type_maps_correctly() {
        assert_eq!(
            node_kind_to_ir_type("type_identifier", "String"),
            IrType::String
        );
    }

    #[test]
    fn unknown_class_maps_to_class_variant() {
        assert_eq!(
            node_kind_to_ir_type("type_identifier", "MyClass"),
            IrType::Class("MyClass".into())
        );
    }
}
