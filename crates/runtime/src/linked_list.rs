//! [`JLinkedList<T>`] -- Rust representation of `java.util.LinkedList` / `java.util.ArrayDeque`.
//!
//! Backed by a `VecDeque<T>` which provides O(1) push/pop at both ends.

use std::collections::VecDeque;

/// A Java-compatible linked list / deque backed by `VecDeque<T>`.
///
/// Mapping: `LinkedList<T>` / `ArrayDeque<T>` -> `JLinkedList<T>`.
#[derive(Debug, Clone)]
pub struct JLinkedList<T> {
    inner: VecDeque<T>,
}

impl<T> Default for JLinkedList<T> {
    fn default() -> Self {
        JLinkedList {
            inner: VecDeque::new(),
        }
    }
}

impl<T: Clone> JLinkedList<T> {
    pub fn new() -> Self {
        JLinkedList {
            inner: VecDeque::new(),
        }
    }

    /// Java `list.add(item)` -- appends to tail.
    pub fn add(&mut self, item: T) {
        self.inner.push_back(item);
    }

    /// Java `list.addFirst(item)`.
    #[allow(non_snake_case)]
    pub fn addFirst(&mut self, item: T) {
        self.inner.push_front(item);
    }

    /// Java `list.addLast(item)` -- same as `add`.
    #[allow(non_snake_case)]
    pub fn addLast(&mut self, item: T) {
        self.inner.push_back(item);
    }

    /// Java `list.get(index)`.
    pub fn get(&self, index: i32) -> T {
        self.inner
            .get(index as usize)
            .cloned()
            .unwrap_or_else(|| panic!("IndexOutOfBoundsException: {index}"))
    }

    /// Java `list.getFirst()`.
    #[allow(non_snake_case)]
    pub fn getFirst(&self) -> T {
        self.inner
            .front()
            .cloned()
            .unwrap_or_else(|| panic!("NoSuchElementException"))
    }

    /// Java `list.getLast()`.
    #[allow(non_snake_case)]
    pub fn getLast(&self) -> T {
        self.inner
            .back()
            .cloned()
            .unwrap_or_else(|| panic!("NoSuchElementException"))
    }

    /// Java `list.removeFirst()`.
    #[allow(non_snake_case)]
    pub fn removeFirst(&mut self) -> T {
        self.inner
            .pop_front()
            .unwrap_or_else(|| panic!("NoSuchElementException"))
    }

    /// Java `list.removeLast()`.
    #[allow(non_snake_case)]
    pub fn removeLast(&mut self) -> T {
        self.inner
            .pop_back()
            .unwrap_or_else(|| panic!("NoSuchElementException"))
    }

    /// Java `list.remove(index)`.
    pub fn remove(&mut self, index: i32) -> T {
        self.inner
            .remove(index as usize)
            .unwrap_or_else(|| panic!("IndexOutOfBoundsException: {index}"))
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

    /// Java `list.peek()` -- returns first element.
    pub fn peek(&self) -> T {
        self.getFirst()
    }

    /// Java `list.poll()` -- removes and returns first element.
    pub fn poll(&mut self) -> T {
        self.removeFirst()
    }

    /// Java `list.offer(item)` -- queue interface, appends to tail.
    pub fn offer(&mut self, item: T) -> bool {
        self.inner.push_back(item);
        true
    }

    /// Java `list.push(item)` -- stack interface, pushes to front.
    pub fn push(&mut self, item: T) {
        self.inner.push_front(item);
    }

    /// Java `list.pop()` -- stack interface, removes from front.
    pub fn pop(&mut self) -> T {
        self.removeFirst()
    }

    /// Iterator over references to elements.
    pub fn iter(&self) -> std::collections::vec_deque::Iter<'_, T> {
        self.inner.iter()
    }
}

impl<T: Clone + PartialEq> JLinkedList<T> {
    /// Java `list.contains(item)`.
    pub fn contains(&self, item: T) -> bool {
        self.inner.contains(&item)
    }
}

impl<T: Clone + Default + std::fmt::Debug + 'static> JLinkedList<T> {
    /// Java `list.stream()`.
    pub fn stream(&self) -> crate::stream::JStream<T> {
        crate::stream::JStream::new(self.inner.iter().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_operations() {
        let mut list: JLinkedList<i32> = JLinkedList::new();
        assert!(list.isEmpty());
        list.add(1);
        list.add(2);
        list.add(3);
        assert_eq!(list.size(), 3);
        assert_eq!(list.get(0), 1);
        assert_eq!(list.get(2), 3);
    }

    #[test]
    fn add_first_last() {
        let mut list: JLinkedList<i32> = JLinkedList::new();
        list.add(2);
        list.addFirst(1);
        list.addLast(3);
        assert_eq!(list.getFirst(), 1);
        assert_eq!(list.getLast(), 3);
        assert_eq!(list.size(), 3);
    }

    #[test]
    fn remove_first_last() {
        let mut list: JLinkedList<i32> = JLinkedList::new();
        list.add(1);
        list.add(2);
        list.add(3);
        assert_eq!(list.removeFirst(), 1);
        assert_eq!(list.removeLast(), 3);
        assert_eq!(list.size(), 1);
    }

    #[test]
    fn queue_operations() {
        let mut list: JLinkedList<i32> = JLinkedList::new();
        list.offer(10);
        list.offer(20);
        assert_eq!(list.peek(), 10);
        assert_eq!(list.poll(), 10);
        assert_eq!(list.poll(), 20);
        assert!(list.isEmpty());
    }

    #[test]
    fn stack_operations() {
        let mut list: JLinkedList<i32> = JLinkedList::new();
        list.push(1);
        list.push(2);
        list.push(3);
        assert_eq!(list.pop(), 3);
        assert_eq!(list.pop(), 2);
        assert_eq!(list.pop(), 1);
    }

    #[test]
    fn iteration() {
        let mut list: JLinkedList<i32> = JLinkedList::new();
        list.add(10);
        list.add(20);
        list.add(30);
        let collected: Vec<i32> = list.iter().cloned().collect();
        assert_eq!(collected, vec![10, 20, 30]);
    }

    #[test]
    fn contains() {
        let mut list: JLinkedList<i32> = JLinkedList::new();
        list.add(1);
        list.add(2);
        assert!(list.contains(1));
        assert!(!list.contains(5));
    }
}
