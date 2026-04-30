//! [`JMap<K, V>`] — Rust representation of `java.util.Map` / `java.util.HashMap`.
//!
//! Lookup methods take keys **by value** (not reference) to avoid borrow
//! complications in generated code.  Method names use Java's camelCase
//! convention.

use crate::JList;
use std::collections::HashMap;

/// A Java-compatible `Map.Entry` pair.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JMapEntry<K, V> {
    pub(crate) key: K,
    pub(crate) value: V,
}

impl<K: Clone, V: Clone> JMapEntry<K, V> {
    /// Create a new `Map.Entry` pair, mirroring `Map.entry(key, value)`.
    pub fn new(key: K, value: V) -> Self {
        JMapEntry { key, value }
    }

    #[allow(non_snake_case)]
    pub fn getKey(&self) -> K {
        self.key.clone()
    }

    #[allow(non_snake_case)]
    pub fn getValue(&self) -> V {
        self.value.clone()
    }
}

impl<K: std::fmt::Display, V: std::fmt::Display> std::fmt::Display for JMapEntry<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

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
    /// Inserts `value` only if no mapping for `key` exists.
    /// Returns the previous value if present, otherwise returns `None`.
    #[allow(non_snake_case)]
    pub fn putIfAbsent(&mut self, key: K, value: V) -> Option<V> {
        match self.inner.entry(key) {
            std::collections::hash_map::Entry::Occupied(e) => Some(e.get().clone()),
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(value);
                None
            }
        }
    }

    /// Java `map.computeIfAbsent(key, mappingFn)`.
    /// If `key` is absent, calls `mapping_fn(key.clone())`, inserts the result,
    /// and returns it.  If `key` is present, returns the existing value.
    #[allow(non_snake_case)]
    pub fn computeIfAbsent(&mut self, key: K, mut mapping_fn: impl FnMut(K) -> V) -> V {
        match self.inner.entry(key) {
            std::collections::hash_map::Entry::Occupied(e) => e.get().clone(),
            std::collections::hash_map::Entry::Vacant(e) => {
                let k = e.key().clone();
                let v = mapping_fn(k);
                e.insert(v.clone());
                v
            }
        }
    }

    /// Java `map.compute(key, remappingFn)`.
    /// Calls `remapping_fn(key.clone(), existing_option)`, stores the result,
    /// and returns it.  The map is not modified if `remapping_fn` panics.
    pub fn compute(&mut self, key: K, mut remapping_fn: impl FnMut(K, Option<V>) -> V) -> V {
        let old = self.inner.get(&key).cloned();
        let new_val = remapping_fn(key.clone(), old);
        self.inner.insert(key, new_val.clone());
        new_val
    }

    /// Java `map.merge(key, value, remappingFn)`.
    /// If the key is absent, inserts `value`.  If present, calls
    /// `remapping_fn(old, value)` and stores the result.  Returns the new value.
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
    /// Calls `consumer(key, value)` for every entry in the map.
    #[allow(non_snake_case)]
    pub fn forEach(&self, mut consumer: impl FnMut(K, V)) {
        for (k, v) in &self.inner {
            consumer(k.clone(), v.clone());
        }
    }

    /// Java `map.replace(key, value)`.
    /// Replaces the value for `key` if a mapping exists.  Returns the old value
    /// if replaced, panics otherwise (mirrors Java NullPointerException on
    /// auto-unboxing a null return).
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
    /// Replaces each value with `f(key, old_value)`.
    #[allow(non_snake_case)]
    pub fn replaceAll(&mut self, mut f: impl FnMut(K, V) -> V) {
        for (k, v) in self.inner.iter_mut() {
            *v = f(k.clone(), v.clone());
        }
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
