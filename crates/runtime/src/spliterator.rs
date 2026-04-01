//! [`JSpliterator<T>`] — Minimal stub of `java.util.Spliterator`.
//!
//! Delegates to a `Vec<T>` snapshot. Only the most commonly used methods
//! are implemented; `trySplit()` always returns `null` (no real splitting).

use crate::JNull;

/// A minimal Spliterator backed by a snapshot of elements.
#[derive(Debug, Clone)]
pub struct JSpliterator<T> {
    elements: Vec<T>,
    pos: usize,
}

impl<T: Clone> JSpliterator<T> {
    /// Create a spliterator from a vec of elements.
    pub fn from_vec(elements: Vec<T>) -> Self {
        JSpliterator { elements, pos: 0 }
    }

    /// Java `spliterator.trySplit()` — always returns null (no splitting).
    #[allow(non_snake_case)]
    pub fn trySplit(&self) -> JNull {
        JNull
    }

    /// Java `spliterator.estimateSize()`.
    #[allow(non_snake_case)]
    pub fn estimateSize(&self) -> i64 {
        (self.elements.len() - self.pos) as i64
    }

    /// Java `spliterator.characteristics()`.
    pub fn characteristics(&self) -> i32 {
        // ORDERED | SIZED = 0x10 | 0x40
        0x50
    }

    /// Java `spliterator.forEachRemaining(action)`.
    #[allow(non_snake_case)]
    pub fn forEachRemaining<F: FnMut(T)>(&mut self, mut action: F) {
        while self.pos < self.elements.len() {
            action(self.elements[self.pos].clone());
            self.pos += 1;
        }
    }

    /// Java `spliterator.tryAdvance(action)`.
    #[allow(non_snake_case)]
    pub fn tryAdvance<F: FnMut(T)>(&mut self, mut action: F) -> bool {
        if self.pos < self.elements.len() {
            action(self.elements[self.pos].clone());
            self.pos += 1;
            true
        } else {
            false
        }
    }
}
