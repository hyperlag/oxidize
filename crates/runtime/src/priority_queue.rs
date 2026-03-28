//! [`JPriorityQueue<T>`] -- Rust representation of `java.util.PriorityQueue`.
//!
//! Backed by `BinaryHeap<Reverse<T>>` to give min-heap behaviour matching
//! Java's natural-ordering `PriorityQueue`.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// A Java-compatible priority queue (min-heap) backed by `BinaryHeap<Reverse<T>>`.
///
/// Mapping: `PriorityQueue<T>` -> `JPriorityQueue<T>`.
#[derive(Debug, Clone)]
pub struct JPriorityQueue<T: Ord> {
    inner: BinaryHeap<Reverse<T>>,
}

impl<T: Ord> Default for JPriorityQueue<T> {
    fn default() -> Self {
        JPriorityQueue {
            inner: BinaryHeap::new(),
        }
    }
}

impl<T: Ord + Clone> JPriorityQueue<T> {
    pub fn new() -> Self {
        JPriorityQueue {
            inner: BinaryHeap::new(),
        }
    }

    /// Java `pq.add(item)` / `pq.offer(item)`.
    pub fn add(&mut self, item: T) -> bool {
        self.inner.push(Reverse(item));
        true
    }

    /// Java `pq.offer(item)`.
    pub fn offer(&mut self, item: T) -> bool {
        self.add(item)
    }

    /// Java `pq.peek()` -- returns smallest element without removing.
    pub fn peek(&self) -> T {
        self.inner
            .peek()
            .map(|r| r.0.clone())
            .unwrap_or_else(|| panic!("NoSuchElementException"))
    }

    /// Java `pq.poll()` -- removes and returns smallest element.
    pub fn poll(&mut self) -> T {
        self.inner
            .pop()
            .map(|r| r.0)
            .unwrap_or_else(|| panic!("NoSuchElementException"))
    }

    /// Java `pq.remove(item)` -- removes a specific element (linear scan).
    pub fn remove(&mut self, item: T) -> bool {
        let old_len = self.inner.len();
        let items: Vec<Reverse<T>> = self.inner.drain().filter(|r| r.0 != item).collect();
        let removed = items.len() < old_len;
        self.inner = BinaryHeap::from(items);
        removed
    }

    /// Java `pq.size()`.
    pub fn size(&self) -> i32 {
        self.inner.len() as i32
    }

    /// Java `pq.isEmpty()`.
    #[allow(non_snake_case)]
    pub fn isEmpty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Java `pq.clear()`.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Java `pq.contains(item)`.
    pub fn contains(&self, item: T) -> bool {
        self.inner.iter().any(|r| r.0 == item)
    }

    /// Iterator (unordered, matching Java's PriorityQueue.iterator() contract).
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.inner.iter().map(|r| &r.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_heap_order() {
        let mut pq: JPriorityQueue<i32> = JPriorityQueue::new();
        pq.add(30);
        pq.add(10);
        pq.add(20);
        assert_eq!(pq.peek(), 10);
        assert_eq!(pq.poll(), 10);
        assert_eq!(pq.poll(), 20);
        assert_eq!(pq.poll(), 30);
    }

    #[test]
    fn offer_and_size() {
        let mut pq: JPriorityQueue<i32> = JPriorityQueue::new();
        assert!(pq.isEmpty());
        pq.offer(5);
        pq.offer(3);
        assert_eq!(pq.size(), 2);
    }

    #[test]
    fn remove_element() {
        let mut pq: JPriorityQueue<i32> = JPriorityQueue::new();
        pq.add(1);
        pq.add(2);
        pq.add(3);
        assert!(pq.remove(2));
        assert!(!pq.remove(99));
        assert_eq!(pq.size(), 2);
        assert_eq!(pq.poll(), 1);
        assert_eq!(pq.poll(), 3);
    }

    #[test]
    fn contains() {
        let mut pq: JPriorityQueue<i32> = JPriorityQueue::new();
        pq.add(10);
        pq.add(20);
        assert!(pq.contains(10));
        assert!(!pq.contains(99));
    }

    #[test]
    fn clear() {
        let mut pq: JPriorityQueue<i32> = JPriorityQueue::new();
        pq.add(1);
        pq.add(2);
        pq.clear();
        assert!(pq.isEmpty());
    }
}
