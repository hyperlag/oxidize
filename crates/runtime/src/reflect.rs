//! Compile-time reflection shim: `JClass` mirrors `java.lang.Class<?>`.
//!
//! Because the translator operates on single-file Java programs with
//! statically known class structures, reflection is resolved at translation
//! time.  Every generated class receives a `getClass()` method that returns
//! a `JClass` value whose name is a compile-time string literal.

use crate::JString;

/// A handle to the compile-time class descriptor.
///
/// Corresponds to `java.lang.Class<?>`.  The class name is baked in as a
/// `&'static str` so no heap allocation is needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JClass {
    name: &'static str,
}

#[allow(non_snake_case)]
impl JClass {
    /// Create a `JClass` for the given fully-qualified class name.
    pub fn new(name: &'static str) -> Self {
        JClass { name }
    }

    /// Returns the fully-qualified class name (e.g. `"com.example.Foo"`).
    ///
    /// For single-file translations there is no package, so this returns
    /// the simple name.
    pub fn getName(&self) -> JString {
        JString::from(self.name)
    }

    /// Returns just the simple (unqualified) class name.
    pub fn getSimpleName(&self) -> JString {
        let simple = self.name.rsplit('.').next().unwrap_or(self.name);
        JString::from(simple)
    }

    /// Returns the canonical name, identical to `getName()` for non-inner
    /// classes.
    pub fn getCanonicalName(&self) -> JString {
        self.getName()
    }

    /// Returns `true` if `obj._instanceof(self.name)` would be true.
    ///
    /// Provided as a helper so callers never need to spell out the name
    /// string themselves.
    pub fn raw_name(&self) -> &'static str {
        self.name
    }
}

impl std::fmt::Display for JClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "class {}", self.name)
    }
}
