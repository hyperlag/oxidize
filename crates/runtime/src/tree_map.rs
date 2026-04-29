//! [`JTreeMap<K, V>`] -- Rust representation of `java.util.TreeMap`.
//!
//! Backed by `BTreeMap<K, V>` which provides sorted key iteration.

use crate::map::JMapEntry;
use crate::JList;
use std::collections::BTreeMap;

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
    pub fn entrySet(&self) -> JList<JMapEntry<K, V>> {
        let mut l = JList::new();
        for (k, v) in &self.inner {
            l.add(JMapEntry {
                key: k.clone(),
                value: v.clone(),
            });
        }
        l
    }

    /// Java `map.putIfAbsent(key, value)`.
    #[allow(non_snake_case)]
    pub fn putIfAbsent(&mut self, key: K, value: V) -> Option<V> {
        match self.inner.entry(key) {
            std::collections::btree_map::Entry::Occupied(e) => Some(e.get().clone()),
            std::collections::btree_map::Entry::Vacant(e) => {
                e.insert(value);
                None
            }
        }
    }

    /// Java `map.computeIfAbsent(key, mappingFn)`.
    #[allow(non_snake_case)]
    pub fn computeIfAbsent(&mut self, key: K, mut mapping_fn: impl FnMut(K) -> V) -> V {
        match self.inner.entry(key) {
            std::collections::btree_map::Entry::Occupied(e) => e.get().clone(),
            std::collections::btree_map::Entry::Vacant(e) => {
                let k = e.key().clone();
                let v = mapping_fn(k);
                e.insert(v.clone());
                v
            }
        }
    }

    /// Java `map.compute(key, remappingFn)`.
    /// The map is not modified if `remapping_fn` panics.
    pub fn compute(&mut self, key: K, mut remapping_fn: impl FnMut(K, Option<V>) -> V) -> V {
        let old = self.inner.get(&key).cloned();
        let new_val = remapping_fn(key.clone(), old);
        self.inner.insert(key, new_val.clone());
        new_val
    }

    /// Java `map.merge(key, value, remappingFn)`.
    /// The map is not modified if `remapping_fn` panics.
    pub fn merge(&mut self, key: K, value: V, mut remapping_fn: impl FnMut(V, V) -> V) -> V {
        let new_val = if let Some(old) = self.inner.get(&key).cloned() {
            remapping_fn(old, value)
        } else {
            value
        };
        self.inner.insert(key, new_val.clone());
        new_val
    }

    /// Java `map.forEach(biConsumer)`.
    #[allow(non_snake_case)]
    pub fn forEach(&self, mut consumer: impl FnMut(K, V)) {
        for (k, v) in &self.inner {
            consumer(k.clone(), v.clone());
        }
    }

    /// Java `map.replace(key, value)`.
    pub fn replace(&mut self, key: K, value: V) -> V {
        if let Some(slot) = self.inner.get_mut(&key) {
            let old = slot.clone();
            *slot = value;
            old
        } else {
            panic!("NullPointerException: key not found in map for replace")
        }
    }

    /// Java `map.replaceAll(biFunction)`.
    #[allow(non_snake_case)]
    pub fn replaceAll(&mut self, mut f: impl FnMut(K, V) -> V) {
        for (k, v) in self.inner.iter_mut() {
            *v = f(k.clone(), v.clone());
        }
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
