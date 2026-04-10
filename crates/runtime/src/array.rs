//! [`JArray<T>`] — Rust representation of a Java array `T[]`.
//!
//! Java arrays are mutable, fixed-length, and heap-allocated. We model them
//! with `Arc<RwLock<Vec<T>>>` so that array references can be shared (Java
//! reference semantics) and mutated safely.

use std::sync::{Arc, RwLock};

/// A heap-allocated, shareable Java array.
///
/// Mapping: `T[]` → `JArray<T>` (wraps `Arc<RwLock<Vec<T>>>`).
#[derive(Debug, Clone)]
pub struct JArray<T>(Arc<RwLock<Vec<T>>>);

impl<T: Clone + std::fmt::Debug> JArray<T> {
    /// Create a new `JArray` with the given elements.
    pub fn from_vec(v: Vec<T>) -> Self {
        JArray(Arc::new(RwLock::new(v)))
    }

    /// Java `array.length` (field, not a method call).
    pub fn length(&self) -> i32 {
        self.0.read().unwrap().len() as i32
    }

    /// Index read: `array[index]`.
    ///
    /// # Panics
    /// Panics (like Java's `ArrayIndexOutOfBoundsException`) if `index` is
    /// out of range.
    pub fn get(&self, index: i32) -> T {
        let guard = self.0.read().unwrap();
        guard
            .get(index as usize)
            .cloned()
            .unwrap_or_else(|| panic!("ArrayIndexOutOfBoundsException: {index}"))
    }

    /// Index write: `array[index] = value`.
    ///
    /// # Panics
    /// Panics if `index` is out of range.
    pub fn set(&self, index: i32, value: T) {
        let mut guard = self.0.write().unwrap();
        let len = guard.len();
        if index < 0 || index as usize >= len {
            panic!("ArrayIndexOutOfBoundsException: {index}");
        }
        guard[index as usize] = value;
    }
}

impl<T: Default + Clone + std::fmt::Debug> JArray<T> {
    /// Create a zero-initialised array of length `len`.
    ///
    /// Mirrors `new T[len]` for default-constructible element types.
    pub fn new_default(len: i32) -> Self {
        if len < 0 {
            panic!("NegativeArraySizeException: {len}");
        }
        JArray(Arc::new(RwLock::new(vec![T::default(); len as usize])))
    }
}

impl<T: Clone + std::fmt::Debug> JArray<T> {
    /// Create an array of length `len` where each element is produced by
    /// calling `init(index)`.  Used for multi-dimensional array allocation:
    /// `new int[r][c]` → `JArray::new_with(r, |_| JArray::new_default(c))`.
    pub fn new_with<F: FnMut(i32) -> T>(len: i32, init: F) -> Self {
        if len < 0 {
            panic!("NegativeArraySizeException: {len}");
        }
        JArray(Arc::new(RwLock::new((0..len).map(init).collect())))
    }

    /// Return all elements as a cloned `Vec<T>`.
    ///
    /// Used by the enhanced-for desugaring:
    /// `for (T x : array)` → `for x in array.iter() { let x: T = x.clone(); … }`
    pub fn iter(&self) -> Vec<T> {
        self.0.read().unwrap().clone()
    }
}

impl<T: Clone + std::fmt::Debug> JArray<T> {
    /// Java `Arrays.fill(arr, val)` — sets every element to `val`.
    pub fn fill(&self, val: T) {
        let mut guard = self.0.write().unwrap();
        for elem in guard.iter_mut() {
            *elem = val.clone();
        }
    }
}

impl<T: Default + Clone + std::fmt::Debug> JArray<T> {
    /// Java `Arrays.copyOfRange(arr, from, to)` — returns a new array with
    /// elements at indices `[from, to)`.
    pub fn copy_of_range(&self, from: i32, to: i32) -> Self {
        if to < from {
            panic!("IllegalArgumentException: from({from}) > to({to})");
        }
        let guard = self.0.read().unwrap();
        let len = guard.len() as i32;
        if from < 0 || from > len {
            panic!("ArrayIndexOutOfBoundsException: {from}");
        }

        let from_usize = from as usize;
        let result_len = (to - from) as usize;
        let available = guard.len().saturating_sub(from_usize).min(result_len);
        let mut v = vec![T::default(); result_len];
        if available > 0 {
            v[..available].clone_from_slice(&guard[from_usize..from_usize + available]);
        }
        JArray(Arc::new(RwLock::new(v)))
    }

    /// Java `Arrays.copyOf(arr, newLen)` — returns a copy of the array
    /// truncated or zero-extended to `new_len`.
    pub fn copy_of_length(&self, new_len: i32) -> Self {
        if new_len < 0 {
            panic!("NegativeArraySizeException: {new_len}");
        }
        let guard = self.0.read().unwrap();
        let n = new_len as usize;
        let mut v = vec![T::default(); n];
        let copy = n.min(guard.len());
        v[..copy].clone_from_slice(&guard[..copy]);
        JArray(Arc::new(RwLock::new(v)))
    }
}

impl<T: Clone + std::fmt::Debug + Ord> JArray<T> {
    /// Java `Arrays.sort(arr)` — sorts in-place.
    pub fn sort_in_place(&self) {
        self.0.write().unwrap().sort();
    }

    /// Java `Arrays.binarySearch(arr, key)` — binary searches a sorted array.
    /// Returns the index of the key, or a negative value if not found.
    pub fn binary_search_val(&self, key: T) -> i32 {
        match self.0.read().unwrap().binary_search(&key) {
            Ok(i) => i as i32,
            Err(i) => -(i as i32) - 1,
        }
    }
}

impl<T: Clone + std::fmt::Debug + std::fmt::Display> JArray<T> {
    /// Java `Arrays.toString(arr)` — returns a display string like `[1, 2, 3]`.
    pub fn to_display_string(&self) -> crate::string::JString {
        let guard = self.0.read().unwrap();
        let inner: Vec<String> = guard.iter().map(|v| format!("{v}")).collect();
        crate::string::JString::from(format!("[{}]", inner.join(", ")).as_str())
    }
}

impl<T: Clone + std::fmt::Debug + PartialEq> JArray<T> {
    /// Java `Arrays.equals(a, b)` — element-wise equality.
    pub fn elements_equal(&self, other: &Self) -> bool {
        *self.0.read().unwrap() == *other.0.read().unwrap()
    }
}

impl<T: Clone + std::fmt::Debug> PartialEq for JArray<T>
where
    T: PartialEq,
{
    /// Reference equality — same `Arc` pointer — matching Java array `==`.
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_get_set() {
        let arr = JArray::from_vec(vec![1_i32, 2, 3]);
        assert_eq!(arr.get(0), 1);
        arr.set(1, 99);
        assert_eq!(arr.get(1), 99);
    }

    #[test]
    fn shared_mutation() {
        let a = JArray::from_vec(vec![0_i32; 3]);
        let b = a.clone(); // same Arc
        b.set(0, 42);
        assert_eq!(a.get(0), 42);
    }

    #[test]
    fn length() {
        let arr: JArray<i32> = JArray::new_default(5);
        assert_eq!(arr.length(), 5);
    }

    #[test]
    #[should_panic(expected = "ArrayIndexOutOfBoundsException")]
    fn out_of_bounds_panics() {
        let arr = JArray::from_vec(vec![1_i32]);
        arr.get(5);
    }

    #[test]
    fn copy_of_range_zero_extends() {
        let arr = JArray::from_vec(vec![1_i32, 2, 3]);
        let out = arr.copy_of_range(1, 5);
        assert_eq!(out.iter(), vec![2, 3, 0, 0]);
    }

    #[test]
    #[should_panic(expected = "NegativeArraySizeException")]
    fn copy_of_length_negative_panics() {
        let arr = JArray::from_vec(vec![1_i32, 2, 3]);
        let _ = arr.copy_of_length(-1);
    }
}
