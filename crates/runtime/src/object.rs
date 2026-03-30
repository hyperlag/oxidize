//! [`JObject`] and [`JNull`] — the root of the Java object hierarchy.

use std::sync::{Arc, RwLock};

/// The Rust equivalent of `java.lang.Object`.
///
/// All translated Java class instances are wrapped in `Arc<RwLock<T>>` where
/// `T` implements `JObject`. This gives Java-like reference semantics (sharing
/// + mutation) without `unsafe`.
pub trait JObject: std::fmt::Debug + Send + Sync {
    /// Java `Object.toString()`.
    fn to_string_java(&self) -> JString
    where
        Self: Sized;

    /// Java `Object.hashCode()`.
    fn hash_code(&self) -> i32;

    /// Java `Object.equals(Object other)`.
    fn equals(&self, other: &dyn JObject) -> bool;
}

/// A shared, heap-allocated Java object reference.
pub type JRef<T> = Arc<RwLock<T>>;

/// Convenience constructor for a new [`JRef`].
pub fn jref<T>(value: T) -> JRef<T> {
    Arc::new(RwLock::new(value))
}

/// Concrete stand-in for Java's `Object` type.
///
/// Used by the code generator when a type has been erased (raw types like bare
/// `List` without type parameters, unbounded wildcards `<?>`, or explicit
/// `Object` declarations).  Implements the standard Rust traits so it can be
/// used as a generic type parameter.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JavaObject;

impl std::fmt::Display for JavaObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Object")
    }
}

/// The `null` sentinel — a unit type that represents a Java `null` reference.
///
/// In the type system, nullable types are represented as `Option<JRef<T>>`.
/// `JNull` is used when a `null` literal must be materialised as a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JNull;

// Re-export JString here so the module boundary doesn't leak.
use crate::JString;

impl JObject for JNull {
    fn to_string_java(&self) -> JString {
        JString::from("null")
    }

    fn hash_code(&self) -> i32 {
        0
    }

    fn equals(&self, other: &dyn JObject) -> bool {
        // Only equal to another JNull — compared by type tag.
        std::ptr::eq(
            other as *const dyn JObject as *const (),
            self as *const JNull as *const (),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jref_allows_shared_mutation() {
        let r = jref(42_i32);
        let r2 = Arc::clone(&r);
        *r2.write().unwrap() = 99;
        assert_eq!(*r.read().unwrap(), 99);
    }

    #[test]
    fn jnull_to_string() {
        assert_eq!(JNull.to_string_java().as_str(), "null");
    }
}
