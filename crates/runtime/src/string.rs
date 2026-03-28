//! [`JString`] — Rust representation of `java.lang.String`.
//!
//! Java strings are immutable and reference-counted; we model them with
//! `Arc<str>`. The `+` operator is overloaded via the `Add` trait to preserve
//! Java's string-concatenation syntax.

use std::fmt;
use std::ops::Add;
use std::sync::Arc;

/// An immutable, reference-counted Java string.
///
/// Mapping: `java.lang.String` → `JString` (wraps `Arc<str>`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JString(Arc<str>);

impl JString {
    /// Create a `JString` from any `&str`.
    pub fn new(s: &str) -> Self {
        JString(Arc::from(s))
    }

    /// Return the underlying string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Java `String.length()`.
    pub fn length(&self) -> i32 {
        // Java counts UTF-16 code units; this counts chars (good enough for
        // Stage 0 — full surrogate-pair handling is deferred).
        self.0.chars().count() as i32
    }

    /// Java `String.isEmpty()`.
    #[allow(non_snake_case)]
    pub fn isEmpty(&self) -> bool {
        self.0.is_empty()
    }

    /// Java `String.isEmpty()` — snake_case alias.
    pub fn is_empty_java(&self) -> bool {
        self.0.is_empty()
    }

    /// Java `String.charAt(int index)`.
    pub fn char_at(&self, index: i32) -> char {
        self.0
            .chars()
            .nth(index as usize)
            .expect("StringIndexOutOfBoundsException")
    }

    /// Java `String.substring(int beginIndex)`.
    pub fn substring(&self, begin: i32) -> JString {
        let s: String = self.0.chars().skip(begin as usize).collect();
        JString(Arc::from(s.as_str()))
    }

    /// Java `String.substring(int beginIndex, int endIndex)`.
    ///
    /// Indices are Unicode scalar values (not UTF-16 code units). Panics with
    /// `StringIndexOutOfBoundsException` if `begin < 0`, `end < begin`, or
    /// `end` exceeds the character count.
    pub fn substring_range(&self, begin: i32, end: i32) -> JString {
        assert!(begin >= 0 && end >= begin, "StringIndexOutOfBoundsException");
        let len = end
            .checked_sub(begin)
            .expect("StringIndexOutOfBoundsException") as usize;
        let s: String = self
            .0
            .chars()
            .skip(begin as usize)
            .take(len)
            .collect();
        assert_eq!(
            s.chars().count(),
            len,
            "StringIndexOutOfBoundsException"
        );
        JString(Arc::from(s.as_str()))
    }

    /// Java `String.trim()`.
    ///
    /// Removes only characters with code point `<= '\u0020'`, matching
    /// Java's `String.trim()` semantics (not Rust's Unicode-aware `trim()`).
    pub fn trim(&self) -> JString {
        JString::from(self.0.trim_matches(|c: char| c <= ' '))
    }

    /// Java `String.contains(CharSequence s)`.
    pub fn contains_str(&self, s: &str) -> bool {
        self.0.contains(s)
    }

    /// Java `String.equals(Object o)`.
    pub fn equals(&self, other: &JString) -> bool {
        *self.0 == *other.0
    }
}

impl From<&str> for JString {
    fn from(s: &str) -> Self {
        JString::new(s)
    }
}

impl From<String> for JString {
    fn from(s: String) -> Self {
        JString(Arc::from(s.as_str()))
    }
}

impl Default for JString {
    /// Returns an empty string — the natural zero-value for `java.lang.String`.
    fn default() -> Self {
        JString::from("")
    }
}

impl fmt::Display for JString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Java's `+` string-concatenation operator.
impl Add for JString {
    type Output = JString;

    fn add(self, rhs: JString) -> JString {
        let mut s = String::with_capacity(self.0.len() + rhs.0.len());
        s.push_str(&self.0);
        s.push_str(&rhs.0);
        JString(Arc::from(s.as_str()))
    }
}

impl Add<&str> for JString {
    type Output = JString;

    fn add(self, rhs: &str) -> JString {
        let mut s = String::with_capacity(self.0.len() + rhs.len());
        s.push_str(&self.0);
        s.push_str(rhs);
        JString(Arc::from(s.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concat_operator() {
        let a = JString::from("Hello, ");
        let b = JString::from("World!");
        assert_eq!((a + b).as_str(), "Hello, World!");
    }

    #[test]
    fn length() {
        assert_eq!(JString::from("abc").length(), 3);
    }

    #[test]
    fn char_at() {
        assert_eq!(JString::from("abc").char_at(1), 'b');
    }

    #[test]
    fn clone_shares_allocation() {
        let a = JString::from("shared");
        let b = a.clone();
        // Both should point to the same Arc — same pointer.
        assert!(Arc::ptr_eq(&a.0, &b.0));
    }
}
