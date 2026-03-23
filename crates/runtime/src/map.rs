//! [`JMap<K, V>`] — Rust representation of `java.util.Map` / `java.util.HashMap`.
//!
//! Lookup methods take keys **by value** (not reference) to avoid borrow
//! complications in generated code.  Method names use Java's camelCase
//! convention.

use std::collections::HashMap;

/// A Java-compatible map backed by a `HashMap<K, V>`.
///
/// Mapping: `Map<K,V>` / `HashMap<K,V>` → `JMap<K,V>`.
#[derive(Debug, Clone)]
pub struct JMap<K, V> {
    inner: HashMap<K, V>,
}

impl<K, V> Default for JMap<K, V> {
    fn default() -> Self {
        JMap {
            inner: HashMap::new(),
        }
    }
}

impl<K, V> JMap<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    /// Create an empty map.  Mirrors `new HashMap<>()`.
    pub fn new() -> Self {
        JMap {
            inner: HashMap::new(),
        }
    }

    /// Java `map.put(key, value)`.  Returns the previous value if present.
    pub fn put(&mut self, key: K, value: V) -> Option<V> {
        self.inner.insert(key, value)
    }

    /// Java `map.get(key)`.
    ///
    /// # Panics
    /// Panics if the key is not present (analogous to Java's
    /// `NullPointerException` when auto-unboxing a `null` returned by
    /// `Map.get`).
    pub fn get(&self, key: K) -> V {
        self.inner
            .get(&key)
            .cloned()
            .unwrap_or_else(|| panic!("NullPointerException: key not found in map"))
    }

    /// Java `map.getOrDefault(key, defaultValue)`.
    #[allow(non_snake_case)]
    pub fn getOrDefault(&self, key: K, default: V) -> V {
        self.inner.get(&key).cloned().unwrap_or(default)
    }

    /// Java `map.containsKey(key)`.
    #[allow(non_snake_case)]
    pub fn containsKey(&self, key: K) -> bool {
        self.inner.contains_key(&key)
    }

    /// Java `map.remove(key)`.
    pub fn remove(&mut self, key: K) -> Option<V> {
        self.inner.remove(&key)
    }

    /// Java `map.size()`.
    pub fn size(&self) -> i32 {
        self.inner.len() as i32
    }

    /// Java `map.isEmpty()`.
    #[allow(non_snake_case)]
    pub fn isEmpty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Java `map.clear()`.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Iterator over `(&key, &value)` pairs.
    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, K, V> {
        self.inner.iter()
    }
}

impl<K, V> JMap<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone + PartialEq,
{
    /// Java `map.containsValue(value)`.
    #[allow(non_snake_case)]
    pub fn containsValue(&self, value: V) -> bool {
        self.inner.values().any(|v| v == &value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::JString;

    #[test]
    fn basic_operations() {
        let mut map: JMap<JString, i32> = JMap::new();
        assert!(map.isEmpty());
        map.put(JString::from("a"), 1);
        map.put(JString::from("b"), 2);
        assert_eq!(map.size(), 2);
        assert_eq!(map.get(JString::from("a")), 1);
        assert!(map.containsKey(JString::from("b")));
        assert!(!map.containsKey(JString::from("c")));
    }

    #[test]
    fn overwrite() {
        let mut map: JMap<JString, i32> = JMap::new();
        map.put(JString::from("x"), 10);
        map.put(JString::from("x"), 20);
        assert_eq!(map.get(JString::from("x")), 20);
        assert_eq!(map.size(), 1);
    }

    #[test]
    fn remove() {
        let mut map: JMap<JString, i32> = JMap::new();
        map.put(JString::from("k"), 99);
        map.remove(JString::from("k"));
        assert!(map.isEmpty());
    }

    #[test]
    fn default_is_empty() {
        let map: JMap<JString, i32> = JMap::default();
        assert!(map.isEmpty());
    }
}
