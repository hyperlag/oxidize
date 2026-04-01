//! [`JLinkedHashMap<K, V>`] -- Rust representation of `java.util.LinkedHashMap`.
//!
//! Backed by `indexmap::IndexMap<K, V>` which preserves insertion order.

use indexmap::IndexMap;
use crate::JList;
use crate::map::JMapEntry;

/// A Java-compatible insertion-ordered map backed by `IndexMap<K, V>`.
///
/// Mapping: `LinkedHashMap<K,V>` -> `JLinkedHashMap<K,V>`.
#[derive(Debug, Clone)]
pub struct JLinkedHashMap<K, V> {
    inner: IndexMap<K, V>,
}

impl<K, V> Default for JLinkedHashMap<K, V> {
    fn default() -> Self {
        JLinkedHashMap {
            inner: IndexMap::new(),
        }
    }
}

impl<K, V> JLinkedHashMap<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    pub fn new() -> Self {
        JLinkedHashMap {
            inner: IndexMap::new(),
        }
    }

    /// Java `map.put(key, value)`.
    pub fn put(&mut self, key: K, value: V) -> Option<V> {
        self.inner.insert(key, value)
    }

    /// Java `map.get(key)`.
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
        self.inner.shift_remove(&key)
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

    /// Iterator over `(&K, &V)` pairs in insertion order.
    pub fn iter(&self) -> indexmap::map::Iter<'_, K, V> {
        self.inner.iter()
    }

    /// Java `map.keySet()`.
    #[allow(non_snake_case)]
    pub fn keySet(&self) -> JList<K> {
        let mut l = JList::new();
        for k in self.inner.keys() {
            l.add(k.clone());
        }
        l
    }

    /// Java `map.values()`.
    pub fn values(&self) -> JList<V> {
        let mut l = JList::new();
        for v in self.inner.values() {
            l.add(v.clone());
        }
        l
    }

    /// Java `map.entrySet()`.
    #[allow(non_snake_case)]
    pub fn entrySet(&self) -> JList<JMapEntry<K, V>>
    {
        let mut l = JList::new();
        for (k, v) in &self.inner {
            l.add(JMapEntry { key: k.clone(), value: v.clone() });
        }
        l
    }
}

impl<K, V> JLinkedHashMap<K, V>
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

    #[test]
    fn insertion_order() {
        let mut map: JLinkedHashMap<String, i32> = JLinkedHashMap::new();
        map.put("c".to_string(), 3);
        map.put("a".to_string(), 1);
        map.put("b".to_string(), 2);
        let keys: Vec<String> = map.iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(keys, vec!["c", "a", "b"]);
    }

    #[test]
    fn basic_operations() {
        let mut map: JLinkedHashMap<String, i32> = JLinkedHashMap::new();
        assert!(map.isEmpty());
        map.put("x".to_string(), 10);
        assert_eq!(map.size(), 1);
        assert_eq!(map.get("x".to_string()), 10);
        assert!(map.containsKey("x".to_string()));
    }

    #[test]
    fn remove_preserves_order() {
        let mut map: JLinkedHashMap<String, i32> = JLinkedHashMap::new();
        map.put("a".to_string(), 1);
        map.put("b".to_string(), 2);
        map.put("c".to_string(), 3);
        map.remove("b".to_string());
        let keys: Vec<String> = map.iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(keys, vec!["a", "c"]);
    }
}
