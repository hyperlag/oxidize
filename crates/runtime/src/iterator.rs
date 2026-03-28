//! [`JIterator<T>`] -- Rust representation of `java.util.Iterator`.
//!
//! Supports `hasNext()`, `next()`, and `remove()` by operating on a cloned
//! `Vec<T>` with an index cursor.

/// A Java-compatible iterator with `remove()` support.
///
/// This wraps a `Vec<T>` and tracks a cursor position, allowing `hasNext()`,
/// `next()`, and `remove()` to work as in Java.
#[derive(Debug, Clone)]
pub struct JIterator<T> {
    items: Vec<T>,
    cursor: usize,
    last_returned: Option<usize>,
}

impl<T: Clone> JIterator<T> {
    /// Create an iterator over the given items.
    pub fn new(items: Vec<T>) -> Self {
        JIterator {
            items,
            cursor: 0,
            last_returned: None,
        }
    }

    /// Java `iterator.hasNext()`.
    #[allow(non_snake_case)]
    pub fn hasNext(&self) -> bool {
        self.cursor < self.items.len()
    }

    /// Java `iterator.next()`.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> T {
        if self.cursor >= self.items.len() {
            panic!("NoSuchElementException");
        }
        let item = self.items[self.cursor].clone();
        self.last_returned = Some(self.cursor);
        self.cursor += 1;
        item
    }

    /// Java `iterator.remove()` -- removes the last element returned by `next()`.
    pub fn remove(&mut self) {
        let idx = self
            .last_returned
            .unwrap_or_else(|| panic!("IllegalStateException: next() has not been called"));
        self.items.remove(idx);
        self.cursor = idx;
        self.last_returned = None;
    }

    /// Get the remaining items (used to write back into the original collection).
    pub fn into_items(self) -> Vec<T> {
        self.items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_iteration() {
        let mut it = JIterator::new(vec![1, 2, 3]);
        assert!(it.hasNext());
        assert_eq!(it.next(), 1);
        assert_eq!(it.next(), 2);
        assert_eq!(it.next(), 3);
        assert!(!it.hasNext());
    }

    #[test]
    fn remove_during_iteration() {
        let mut it = JIterator::new(vec![1, 2, 3, 4]);
        let mut kept = Vec::new();
        while it.hasNext() {
            let val = it.next();
            if val % 2 == 0 {
                it.remove();
            } else {
                kept.push(val);
            }
        }
        assert_eq!(kept, vec![1, 3]);
        assert_eq!(it.into_items(), vec![1, 3]);
    }

    #[test]
    #[should_panic(expected = "IllegalStateException")]
    fn remove_without_next_panics() {
        let mut it = JIterator::new(vec![1, 2]);
        it.remove();
    }

    #[test]
    #[should_panic(expected = "NoSuchElementException")]
    fn next_past_end_panics() {
        let mut it = JIterator::new(vec![1]);
        it.next();
        it.next();
    }
}
