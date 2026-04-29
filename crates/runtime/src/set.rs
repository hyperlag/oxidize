//! [`JSet<T>`] — Rust representation of `java.util.Set` / `java.util.HashSet`.
//!
//! Method names use Java's camelCase convention.

use std::collections::HashSet;

/// A Java-compatible set backed by a `HashSet<T>`.
///
/// Mapping: `Set<T>` / `HashSet<T>` → `JSet<T>`.
#[derive(Debug, Clone)]
pub struct JSet<T> {
    inner: HashSet<T>,
}

impl<T> Default for JSet<T> {
    fn default() -> Self {
        JSet {
            inner: HashSet::new(),
        }
    }
}

impl<T: Eq + std::hash::Hash + Clone> JSet<T> {
    /// Create an empty set.  Mirrors `new HashSet<>()`.
    pub fn new() -> Self {
        JSet {
            inner: HashSet::new(),
        }
    }

    /// Java `set.add(item)`.  Returns `true` if the item was not already
    /// present.
    pub fn add(&mut self, item: T) -> bool {
        self.inner.insert(item)
    }

    /// Java `set.contains(item)`.
    pub fn contains(&self, item: T) -> bool {
        self.inner.contains(&item)
    }

    /// Java `set.remove(item)`.  Returns `true` if the item was present.
    pub fn remove(&mut self, item: T) -> bool {
        self.inner.remove(&item)
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

    /// Create a set from a `Vec<T>`, mirroring `Set.of(a, b, c)` /
    /// `Set.copyOf(collection)`.
    pub fn from_vec(v: Vec<T>) -> Self {
        let mut set = JSet::new();
        for item in v {
            set.add(item);
        }
        set
    }

    /// Iterator over set elements.
    pub fn iter(&self) -> std::collections::hash_set::Iter<'_, T> {
        self.inner.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_operations() {
        let mut set: JSet<i32> = JSet::new();
        assert!(set.isEmpty());
        assert!(set.add(1));
        assert!(set.add(2));
        assert!(!set.add(1)); // duplicate returns false
        assert_eq!(set.size(), 2);
        assert!(set.contains(1));
        assert!(!set.contains(99));
    }

    #[test]
    fn remove() {
        let mut set: JSet<i32> = JSet::new();
        set.add(10);
        assert!(set.remove(10));
        assert!(!set.remove(10));
        assert!(set.isEmpty());
    }

    #[test]
    fn default_is_empty() {
        let set: JSet<i32> = JSet::default();
        assert!(set.isEmpty());
    }
}
