//! [`JTreeSet<T>`] -- Rust representation of `java.util.TreeSet`.
//!
//! Backed by `BTreeSet<T>` which provides sorted iteration.

use std::collections::BTreeSet;

/// A Java-compatible sorted set backed by `BTreeSet<T>`.
///
/// Mapping: `TreeSet<T>` -> `JTreeSet<T>`.
#[derive(Debug, Clone)]
pub struct JTreeSet<T> {
    inner: BTreeSet<T>,
}

impl<T: Ord> Default for JTreeSet<T> {
    fn default() -> Self {
        JTreeSet {
            inner: BTreeSet::new(),
        }
    }
}

impl<T: Ord + Clone> JTreeSet<T> {
    pub fn new() -> Self {
        JTreeSet {
            inner: BTreeSet::new(),
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

    /// Java `set.first()` -- smallest element.
    pub fn first(&self) -> T {
        self.inner
            .iter()
            .next()
            .cloned()
            .unwrap_or_else(|| panic!("NoSuchElementException"))
    }

    /// Java `set.last()` -- largest element.
    pub fn last(&self) -> T {
        self.inner
            .iter()
            .next_back()
            .cloned()
            .unwrap_or_else(|| panic!("NoSuchElementException"))
    }

    /// Sorted iterator over set elements.
    pub fn iter(&self) -> std::collections::btree_set::Iter<'_, T> {
        self.inner.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorted_iteration() {
        let mut set: JTreeSet<i32> = JTreeSet::new();
        set.add(30);
        set.add(10);
        set.add(20);
        let elements: Vec<i32> = set.iter().cloned().collect();
        assert_eq!(elements, vec![10, 20, 30]);
    }

    #[test]
    fn basic_operations() {
        let mut set: JTreeSet<i32> = JTreeSet::new();
        assert!(set.isEmpty());
        assert!(set.add(1));
        assert!(set.add(2));
        assert!(!set.add(1));
        assert_eq!(set.size(), 2);
        assert!(set.contains(1));
    }

    #[test]
    fn first_last() {
        let mut set: JTreeSet<i32> = JTreeSet::new();
        set.add(10);
        set.add(5);
        set.add(20);
        assert_eq!(set.first(), 5);
        assert_eq!(set.last(), 20);
    }

    #[test]
    fn remove() {
        let mut set: JTreeSet<i32> = JTreeSet::new();
        set.add(1);
        set.add(2);
        assert!(set.remove(1));
        assert!(!set.remove(99));
        assert_eq!(set.size(), 1);
    }
}
