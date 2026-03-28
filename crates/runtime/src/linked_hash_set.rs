//! [`JLinkedHashSet<T>`] -- Rust representation of `java.util.LinkedHashSet`.
//!
//! Backed by `indexmap::IndexSet<T>` which preserves insertion order.

use indexmap::IndexSet;

/// A Java-compatible insertion-ordered set backed by `IndexSet<T>`.
///
/// Mapping: `LinkedHashSet<T>` -> `JLinkedHashSet<T>`.
#[derive(Debug, Clone)]
pub struct JLinkedHashSet<T> {
    inner: IndexSet<T>,
}

impl<T> Default for JLinkedHashSet<T> {
    fn default() -> Self {
        JLinkedHashSet {
            inner: IndexSet::new(),
        }
    }
}

impl<T: Eq + std::hash::Hash + Clone> JLinkedHashSet<T> {
    pub fn new() -> Self {
        JLinkedHashSet {
            inner: IndexSet::new(),
        }
    }

    /// Java `set.add(item)`.
    pub fn add(&mut self, item: T) -> bool {
        self.inner.insert(item)
    }

    /// Java `set.contains(item)`.
    pub fn contains(&self, item: T) -> bool {
        self.inner.contains(&item)
    }

    /// Java `set.remove(item)`.
    pub fn remove(&mut self, item: T) -> bool {
        self.inner.shift_remove(&item)
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

    /// Iterator over set elements in insertion order.
    pub fn iter(&self) -> indexmap::set::Iter<'_, T> {
        self.inner.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insertion_order() {
        let mut set: JLinkedHashSet<i32> = JLinkedHashSet::new();
        set.add(30);
        set.add(10);
        set.add(20);
        let elements: Vec<i32> = set.iter().cloned().collect();
        assert_eq!(elements, vec![30, 10, 20]);
    }

    #[test]
    fn basic_operations() {
        let mut set: JLinkedHashSet<i32> = JLinkedHashSet::new();
        assert!(set.isEmpty());
        assert!(set.add(1));
        assert!(set.add(2));
        assert!(!set.add(1));
        assert_eq!(set.size(), 2);
        assert!(set.contains(1));
    }

    #[test]
    fn remove_preserves_order() {
        let mut set: JLinkedHashSet<i32> = JLinkedHashSet::new();
        set.add(1);
        set.add(2);
        set.add(3);
        set.remove(2);
        let elements: Vec<i32> = set.iter().cloned().collect();
        assert_eq!(elements, vec![1, 3]);
    }
}
