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

    /// Java `list.spliterator()`.
    pub fn spliterator(&self) -> crate::JSpliterator<T> {
        crate::JSpliterator::from_vec(self.inner.clone())
    }
}

impl<T: Clone + Ord> JList<T> {
    /// Java `Collections.sort(list)` -- sorts by natural ordering.
    pub fn sort(&mut self) {
        self.inner.sort();
    }

    /// Java `Collections.min(list)`.
    pub fn min_element(&self) -> T {
        self.inner
            .iter()
            .min()
            .cloned()
            .expect("NoSuchElementException: empty list")
    }

    /// Java `Collections.max(list)`.
    pub fn max_element(&self) -> T {
        self.inner
            .iter()
            .max()
            .cloned()
            .expect("NoSuchElementException: empty list")
    }

    /// Java `Collections.binarySearch(list, key)`.
    pub fn binary_search_val(&self, key: T) -> i32 {
        match self.inner.binary_search(&key) {
            Ok(i) => i as i32,
            Err(i) => -(i as i32) - 1,
        }
    }
}

impl<T: Clone> JList<T> {
    /// Java `Collections.sort(list, comparator)` -- sorts with a custom comparator.
    /// The comparator follows Java convention: returns negative, zero, or positive i32.
    pub fn sort_with(&mut self, cmp: impl Fn(&T, &T) -> i32) {
        self.inner.sort_by(|a, b| {
            let r = cmp(a, b);
            if r < 0 {
                std::cmp::Ordering::Less
            } else if r > 0 {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });
    }

    /// Java `Collections.reverse(list)`.
    pub fn reverse(&mut self) {
        self.inner.reverse();
    }

    /// Java `Collections.singletonList(item)`.
    pub fn singleton(item: T) -> Self {
        JList { inner: vec![item] }
    }

    /// Java `list.retainAll` / `Iterator.remove()` pattern support.
    pub fn retain(&mut self, f: impl Fn(&T) -> bool) {
        self.inner.retain(|item| f(item));
    }

    /// Java `Collections.nCopies(n, val)` — list of n copies of val.
    pub fn n_copies(n: i32, val: T) -> Self {
        if n < 0 {
            panic!("IllegalArgumentException: List length = {n}");
        }
        JList {
            inner: vec![val; n as usize],
        }
    }

    /// Java `Collections.fill(list, val)` — replaces every element with val.
    pub fn fill_all(&mut self, val: T) {
        for elem in self.inner.iter_mut() {
            *elem = val.clone();
        }
    }

    /// Java `Collections.swap(list, i, j)` — swaps two elements.
    pub fn swap(&mut self, i: i32, j: i32) {
        let len = self.inner.len();
        if i < 0 || j < 0 || i as usize >= len || j as usize >= len {
            panic!("java.lang.IndexOutOfBoundsException");
        }
        self.inner.swap(i as usize, j as usize);
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

    /// Java `Collections.frequency(list, elem)` — count occurrences.
    pub fn frequency(&self, item: T) -> i32 {
        self.inner.iter().filter(|x| *x == &item).count() as i32
    }

    /// Java `Collections.disjoint(a, b)` — true if the lists share no elements.
    pub fn disjoint(&self, other: &JList<T>) -> bool {
        self.inner.iter().all(|x| !other.inner.contains(x))
    }
}

impl<T: Clone + Default + std::fmt::Debug + 'static> JList<T> {
    /// Java `list.stream()` — returns a `JStream` over the list elements.
    pub fn stream(&self) -> crate::stream::JStream<T> {
        crate::stream::JStream::new(self.inner.clone())
    }

    /// Create a `JList` from an existing `Vec<T>`.
    pub fn from_vec(v: Vec<T>) -> Self {
        JList { inner: v }
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

    #[test]
    fn spliterator_estimate_size_and_try_advance() {
        let mut list: JList<i32> = JList::new();
        list.add(10);
        list.add(20);
        list.add(30);
        let mut sp = list.spliterator();
        assert_eq!(sp.estimateSize(), 3);
        let mut seen = Vec::new();
        assert!(sp.tryAdvance(|v| seen.push(v)));
        assert!(sp.tryAdvance(|v| seen.push(v)));
        assert!(sp.tryAdvance(|v| seen.push(v)));
        assert!(!sp.tryAdvance(|v| seen.push(v)));
        assert_eq!(seen, vec![10, 20, 30]);
    }

    #[test]
    #[should_panic(expected = "IllegalArgumentException")]
    fn n_copies_negative_panics() {
        let _ = JList::n_copies(-1, 7);
    }

    #[test]
    #[should_panic(expected = "java.lang.IndexOutOfBoundsException")]
    fn swap_invalid_index_panics() {
        let mut list: JList<i32> = JList::from_vec(vec![1, 2, 3]);
        list.swap(-1, 2);
    }
}
