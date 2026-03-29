//! [`JEnumSet<T>`] -- Rust representation of `java.util.EnumSet`.
//!
//! Backed by `HashSet<T>` since Rust enums with `Eq + Hash` work as elements.
//! Method names use Java's camelCase convention.

use std::collections::HashSet;

/// A Java-compatible enum set backed by `HashSet<T>`.
///
/// Mapping: `EnumSet<T>` -> `JEnumSet<T>`.
#[derive(Debug, Clone)]
pub struct JEnumSet<T> {
    inner: HashSet<T>,
}

impl<T> Default for JEnumSet<T> {
    fn default() -> Self {
        JEnumSet {
            inner: HashSet::new(),
        }
    }
}

impl<T> JEnumSet<T>
where
    T: Eq + std::hash::Hash + Clone,
{
    /// Create an empty enum set.  Mirrors `EnumSet.noneOf(...)`.
    pub fn new() -> Self {
        JEnumSet {
            inner: HashSet::new(),
        }
    }

    /// Create an enum set containing the given values.
    /// Mirrors `EnumSet.of(...)`.
    pub fn of(values: Vec<T>) -> Self {
        JEnumSet {
            inner: values.into_iter().collect(),
        }
    }

    /// Java `set.add(element)`.
    pub fn add(&mut self, element: T) -> bool {
        self.inner.insert(element)
    }

    /// Java `set.contains(element)`.
    pub fn contains(&self, element: T) -> bool {
        self.inner.contains(&element)
    }

    /// Java `set.remove(element)`.
    pub fn remove(&mut self, element: T) -> bool {
        self.inner.remove(&element)
    }

    /// Java `set.size()`.
    pub fn size(&self) -> i32 {
        self.inner.len() as i32
    }

    /// Java `set.isEmpty()`.
    #[allow(non_snake_case)]
    pub fn isEmpty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Java `set.clear()`.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Iterator over elements.
    pub fn iter(&self) -> std::collections::hash_set::Iter<'_, T> {
        self.inner.iter()
    }
}

impl<T> std::fmt::Display for JEnumSet<T>
where
    T: std::fmt::Display + Eq + std::hash::Hash,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        let mut first = true;
        for v in &self.inner {
            if !first {
                write!(f, ", ")?;
            }
            write!(f, "{}", v)?;
            first = false;
        }
        write!(f, "]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_operations() {
        let mut set: JEnumSet<String> = JEnumSet::new();
        assert!(set.isEmpty());
        assert!(set.add("A".to_string()));
        assert!(set.add("B".to_string()));
        assert_eq!(set.size(), 2);
        assert!(set.contains("A".to_string()));
        assert!(!set.contains("C".to_string()));
    }

    #[test]
    fn of_factory() {
        let set = JEnumSet::of(vec!["X".to_string(), "Y".to_string()]);
        assert_eq!(set.size(), 2);
        assert!(set.contains("X".to_string()));
        assert!(set.contains("Y".to_string()));
    }

    #[test]
    fn remove() {
        let mut set: JEnumSet<String> = JEnumSet::new();
        set.add("X".to_string());
        assert!(set.remove("X".to_string()));
        assert!(set.isEmpty());
    }
}
