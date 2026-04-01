//! [`JTreeMap<K, V>`] -- Rust representation of `java.util.TreeMap`.
//!
//! Backed by `BTreeMap<K, V>` which provides sorted key iteration.

use std::collections::BTreeMap;
use crate::JList;
use crate::map::JMapEntry;

/// A Java-compatible sorted map backed by `BTreeMap<K, V>`.
///
/// Mapping: `TreeMap<K,V>` -> `JTreeMap<K,V>`.
#[derive(Debug, Clone)]
pub struct JTreeMap<K, V> {
    inner: BTreeMap<K, V>,
}

impl<K: Ord, V> Default for JTreeMap<K, V> {
    fn default() -> Self {
        JTreeMap {
            inner: BTreeMap::new(),
        }
    }
}

impl<K: Ord + Clone, V: Clone> JTreeMap<K, V> {
    pub fn new() -> Self {
        JTreeMap {
            inner: BTreeMap::new(),
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

    /// Java `map.firstKey()`.
    #[allow(non_snake_case)]
    pub fn firstKey(&self) -> K {
        self.inner
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| panic!("NoSuchElementException"))
    }

    /// Java `map.lastKey()`.
    #[allow(non_snake_case)]
    pub fn lastKey(&self) -> K {
        self.inner
            .keys()
            .next_back()
            .cloned()
            .unwrap_or_else(|| panic!("NoSuchElementException"))
    }

    /// Iterator over `(&K, &V)` pairs in sorted key order.
    pub fn iter(&self) -> std::collections::btree_map::Iter<'_, K, V> {
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

impl<K: Ord + Clone, V: Clone + PartialEq> JTreeMap<K, V> {
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
    fn sorted_iteration() {
        let mut map: JTreeMap<i32, String> = JTreeMap::new();
        map.put(3, "three".to_string());
        map.put(1, "one".to_string());
        map.put(2, "two".to_string());
        let keys: Vec<i32> = map.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3]);
    }

    #[test]
    fn basic_operations() {
        let mut map: JTreeMap<String, i32> = JTreeMap::new();
        assert!(map.isEmpty());
        map.put("a".to_string(), 1);
        map.put("b".to_string(), 2);
        assert_eq!(map.size(), 2);
        assert_eq!(map.get("a".to_string()), 1);
        assert!(map.containsKey("b".to_string()));
    }

    #[test]
    fn first_last_key() {
        let mut map: JTreeMap<i32, i32> = JTreeMap::new();
        map.put(10, 100);
        map.put(5, 50);
        map.put(20, 200);
        assert_eq!(map.firstKey(), 5);
        assert_eq!(map.lastKey(), 20);
    }

    #[test]
    fn remove_and_contains_value() {
        let mut map: JTreeMap<i32, i32> = JTreeMap::new();
        map.put(1, 10);
        map.put(2, 20);
        assert!(map.containsValue(10));
        map.remove(1);
        assert!(!map.containsValue(10));
        assert_eq!(map.size(), 1);
    }
}
