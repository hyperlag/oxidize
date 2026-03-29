//! [`JEnumMap<K, V>`] -- Rust representation of `java.util.EnumMap`.
//!
//! Backed by `BTreeMap<K, V>` to preserve Java's natural enum key ordering.
//! Method names use Java's camelCase convention.

use std::collections::BTreeMap;

/// A Java-compatible enum map backed by `BTreeMap<K, V>`.
///
/// Mapping: `EnumMap<K,V>` -> `JEnumMap<K,V>`.
#[derive(Debug, Clone)]
pub struct JEnumMap<K, V> {
    inner: BTreeMap<K, V>,
}

impl<K: Ord, V> Default for JEnumMap<K, V> {
    fn default() -> Self {
        JEnumMap {
            inner: BTreeMap::new(),
        }
    }
}

impl<K, V> JEnumMap<K, V>
where
    K: Ord + Clone,
    V: Clone,
{
    /// Create an empty enum map.  Mirrors `new EnumMap<>(KeyType.class)`.
    pub fn new() -> Self {
        JEnumMap {
            inner: BTreeMap::new(),
        }
    }

    /// Java `map.put(key, value)`.
    pub fn put(&mut self, key: K, value: V) -> Option<V> {
        self.inner.insert(key, value)
    }

    /// Java `map.get(key)`.
    ///
    /// # Panics
    /// Panics if the key is not present.
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
    pub fn iter(&self) -> std::collections::btree_map::Iter<'_, K, V> {
        self.inner.iter()
    }
}

impl<K, V> JEnumMap<K, V>
where
    K: Ord + Clone,
    V: Clone + PartialEq,
{
    /// Java `map.containsValue(value)`.
    #[allow(non_snake_case)]
    pub fn containsValue(&self, value: V) -> bool {
        self.inner.values().any(|v| v == &value)
    }
}

impl<K, V> std::fmt::Display for JEnumMap<K, V>
where
    K: std::fmt::Display + Ord,
    V: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{")?;
        let mut first = true;
        for (k, v) in &self.inner {
            if !first {
                write!(f, ", ")?;
            }
            write!(f, "{}={}", k, v)?;
            first = false;
        }
        write!(f, "}}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_operations() {
        let mut map: JEnumMap<String, i32> = JEnumMap::new();
        assert!(map.isEmpty());
        map.put("A".to_string(), 1);
        map.put("B".to_string(), 2);
        assert_eq!(map.size(), 2);
        assert_eq!(map.get("A".to_string()), 1);
        assert!(map.containsKey("B".to_string()));
        assert!(!map.containsKey("C".to_string()));
    }

    #[test]
    fn default_is_empty() {
        let map: JEnumMap<String, i32> = JEnumMap::default();
        assert!(map.isEmpty());
        assert_eq!(map.size(), 0);
    }

    #[test]
    fn remove() {
        let mut map: JEnumMap<String, i32> = JEnumMap::new();
        map.put("X".to_string(), 42);
        assert_eq!(map.remove("X".to_string()), Some(42));
        assert!(map.isEmpty());
    }
}
