//! [`JList<T>`] — Rust representation of `java.util.List` / `java.util.ArrayList`.
//!
//! Backed by a plain `Vec<T>`.  Method names use Java's camelCase convention so
//! that generated code can call them without renaming.

/// A Java-compatible list backed by a `Vec<T>`.
///
/// Mapping: `List<T>` / `ArrayList<T>` / `LinkedList<T>` → `JList<T>`.
#[derive(Debug, Clone)]
pub struct JList<T> {
    inner: Vec<T>,
}

impl<T> Default for JList<T> {
    fn default() -> Self {
        JList { inner: Vec::new() }
    }
}

impl<T: Clone> JList<T> {
    /// Create an empty list.  Mirrors `new ArrayList<>()`.
    pub fn new() -> Self {
        JList { inner: Vec::new() }
    }

    /// Java `list.add(item)`.
    pub fn add(&mut self, item: T) {
        self.inner.push(item);
    }

    /// Java `list.get(index)`.
    ///
    /// # Panics
    /// Panics (like Java's `IndexOutOfBoundsException`) if `index` is out of
    /// range.
    pub fn get(&self, index: i32) -> T {
        self.inner
            .get(index as usize)
            .cloned()
            .unwrap_or_else(|| panic!("IndexOutOfBoundsException: {index}"))
    }

    /// Java `list.set(index, item)`.
    pub fn set(&mut self, index: i32, item: T) {
        let i = index as usize;
        if i >= self.inner.len() {
            panic!("IndexOutOfBoundsException: {index}");
        }
        self.inner[i] = item;
    }

    /// Java `list.size()`.
    pub fn size(&self) -> i32 {
        self.inner.len() as i32
    }

    /// Java `list.isEmpty()`.
    #[allow(non_snake_case)]
    pub fn isEmpty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Java `list.clear()`.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Java `list.remove(int index)`.
    pub fn remove(&mut self, index: i32) -> T {
        self.inner.remove(index as usize)
    }

    /// Returns an iterator over references to elements.
    ///
    /// Used by the enhanced-for desugaring:
    /// `for (T x : list)` → `for x in list.iter() { let x: T = x.clone(); ... }`
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.inner.iter()
    }
}

impl<T: Clone + PartialEq> JList<T> {
    /// Java `list.contains(item)`.
    pub fn contains(&self, item: T) -> bool {
        self.inner.contains(&item)
    }

    /// Java `list.indexOf(item)`.
    #[allow(non_snake_case)]
    pub fn indexOf(&self, item: T) -> i32 {
        self.inner
            .iter()
            .position(|x| x == &item)
            .map(|i| i as i32)
            .unwrap_or(-1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_operations() {
        let mut list: JList<i32> = JList::new();
        assert!(list.isEmpty());
        list.add(1);
        list.add(2);
        list.add(3);
        assert_eq!(list.size(), 3);
        assert_eq!(list.get(0), 1);
        assert_eq!(list.get(2), 3);
    }

    #[test]
    fn set_and_remove() {
        let mut list: JList<i32> = JList::new();
        list.add(10);
        list.add(20);
        list.set(0, 99);
        assert_eq!(list.get(0), 99);
        let removed = list.remove(0);
        assert_eq!(removed, 99);
        assert_eq!(list.size(), 1);
    }

    #[test]
    fn iteration() {
        let mut list: JList<i32> = JList::new();
        list.add(10);
        list.add(20);
        let sum: i32 = list.iter().copied().sum();
        assert_eq!(sum, 30);
    }

    #[test]
    fn contains_and_index_of() {
        let mut list: JList<i32> = JList::new();
        list.add(5);
        list.add(10);
        assert!(list.contains(5));
        assert!(!list.contains(99));
        assert_eq!(list.indexOf(10), 1);
        assert_eq!(list.indexOf(99), -1);
    }

    #[test]
    fn default_is_empty() {
        let list: JList<i32> = JList::default();
        assert!(list.isEmpty());
    }
}
