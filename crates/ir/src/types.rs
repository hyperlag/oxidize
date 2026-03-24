//! IR types — the static type system used by the translator.

use serde::{Deserialize, Serialize};

/// The static type of an IR expression or declaration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IrType {
    // ── Primitive types ────────────────────────────────────────────────────
    /// Java `boolean` / Rust `bool`
    Bool,
    /// Java `byte` / Rust `i8`
    Byte,
    /// Java `short` / Rust `i16`
    Short,
    /// Java `int` / Rust `i32`
    Int,
    /// Java `long` / Rust `i64`
    Long,
    /// Java `float` / Rust `f32`
    Float,
    /// Java `double` / Rust `f64`
    Double,
    /// Java `char` / Rust `char`
    Char,
    /// Java `void` / Rust `()`
    Void,

    // ── Reference types ────────────────────────────────────────────────────
    /// `java.lang.String` — mapped to `java_compat::JString`
    String,
    /// Nullable wrapper: `Option<T>` in Rust (`null`-able reference)
    Nullable(Box<IrType>),
    /// An array `T[]` — mapped to `java_compat::JArray<T>`
    Array(Box<IrType>),
    /// A named class or interface, identified by its fully-qualified name.
    Class(String),
    /// A type variable, e.g. `T` from `class Box<T>`.
    TypeVar(String),
    /// A parameterised type, e.g. `List<String>`.
    Generic {
        base: Box<IrType>,
        args: Vec<IrType>,
    },

    // ── Concurrency ────────────────────────────────────────────────────────
    /// A `volatile` primitive field: `AtomicI32`, `AtomicI64`, or `AtomicBool`.
    /// The inner type is the original Java primitive (`Int`, `Long`, or `Bool`).
    Atomic(Box<IrType>),

    // ── Special ────────────────────────────────────────────────────────────
    /// Type has not yet been resolved (placeholder during parsing).
    Unknown,
    /// The `null` literal — a subtype of every reference type.
    Null,
}

impl IrType {
    /// Returns `true` if this type is a Java primitive.
    pub fn is_primitive(&self) -> bool {
        matches!(
            self,
            IrType::Bool
                | IrType::Byte
                | IrType::Short
                | IrType::Int
                | IrType::Long
                | IrType::Float
                | IrType::Double
                | IrType::Char
                | IrType::Void
        )
    }

    /// Returns `true` if this type is a reference (heap-allocated in Java).
    pub fn is_reference(&self) -> bool {
        !self.is_primitive() && !matches!(self, IrType::Unknown | IrType::Null)
    }
}

impl std::fmt::Display for IrType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IrType::Bool => write!(f, "boolean"),
            IrType::Byte => write!(f, "byte"),
            IrType::Short => write!(f, "short"),
            IrType::Int => write!(f, "int"),
            IrType::Long => write!(f, "long"),
            IrType::Float => write!(f, "float"),
            IrType::Double => write!(f, "double"),
            IrType::Char => write!(f, "char"),
            IrType::Void => write!(f, "void"),
            IrType::String => write!(f, "String"),
            IrType::Nullable(inner) => write!(f, "{inner}?"),
            IrType::Array(elem) => write!(f, "{elem}[]"),
            IrType::Class(name) => write!(f, "{name}"),
            IrType::TypeVar(name) => write!(f, "{name}"),
            IrType::Generic { base, args } => {
                write!(f, "{base}<")?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{arg}")?;
                }
                write!(f, ">")
            }
            IrType::Unknown => write!(f, "<unknown>"),
            IrType::Null => write!(f, "null"),
            IrType::Atomic(inner) => write!(f, "atomic({inner})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_classification() {
        assert!(IrType::Int.is_primitive());
        assert!(!IrType::String.is_primitive());
        assert!(!IrType::Class("Foo".into()).is_primitive());
    }

    #[test]
    fn display_generic() {
        let t = IrType::Generic {
            base: Box::new(IrType::Class("List".into())),
            args: vec![IrType::String],
        };
        assert_eq!(t.to_string(), "List<String>");
    }

    #[test]
    fn serde_roundtrip() {
        let ty = IrType::Nullable(Box::new(IrType::Class("java.lang.Integer".into())));
        let json = serde_json::to_string(&ty).unwrap();
        let back: IrType = serde_json::from_str(&json).unwrap();
        assert_eq!(ty, back);
    }
}
