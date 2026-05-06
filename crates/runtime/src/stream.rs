#![allow(non_snake_case)]
//! [`JStream<T>`] — Rust representation of `java.util.stream.Stream<T>`.
//!
//! Mapping: `Stream<T>` → `JStream<T>` (backed by `Vec<T>` for eager evaluation).

use crate::list::JList;
use crate::map::JMap;
use crate::set::JSet;
use crate::string::JString;

/// An ordered sequence supporting aggregate operations.
///
/// Mapping: `java.util.stream.Stream<T>` → `JStream<T>`.
#[derive(Debug, Clone, Default)]
pub struct JStream<T: Clone + Default + std::fmt::Debug + 'static> {
    data: Vec<T>,
}

impl<T: Clone + Default + std::fmt::Debug + 'static> JStream<T> {
    /// Create a stream from a `Vec<T>`.
    pub fn new(data: Vec<T>) -> Self {
        Self { data }
    }

    /// Java `Arrays.stream(arr)` — creates a stream from a `JArray<T>`.
    pub fn from_array(arr: &crate::array::JArray<T>) -> Self {
        Self::new(arr.iter())
    }

    /// Java `stream.filter(predicate)`.
    pub fn filter<F: FnMut(T) -> bool>(self, mut pred: F) -> Self {
        Self {
            data: self.data.into_iter().filter(|x| pred(x.clone())).collect(),
        }
    }

    /// Java `stream.limit(n)`.
    pub fn limit(self, n: i64) -> Self {
        if n < 0 {
            panic!("java.lang.IllegalArgumentException: negative limit");
        }
        let n = n as usize;
        Self {
            data: self.data.into_iter().take(n).collect(),
        }
    }

    /// Java `stream.skip(n)`.
    pub fn skip(self, n: i64) -> Self {
        if n < 0 {
            panic!("java.lang.IllegalArgumentException: negative skip");
        }
        let n = n as usize;
        Self {
            data: self.data.into_iter().skip(n).collect(),
        }
    }

    /// Java `stream.count()`.
    pub fn count(self) -> i64 {
        self.data.len() as i64
    }

    /// Java `stream.findFirst()`.
    pub fn findFirst(self) -> Option<T> {
        self.data.into_iter().next()
    }

    /// Java `stream.forEach(consumer)`.
    pub fn forEach<F: FnMut(T)>(self, action: F) {
        self.data.into_iter().for_each(action);
    }

    /// Java `stream.map(mapper)` — transforms each element (same or different type).
    pub fn map<U, F>(self, f: F) -> JStream<U>
    where
        U: Clone + Default + std::fmt::Debug + 'static,
        F: FnMut(T) -> U,
    {
        JStream {
            data: self.data.into_iter().map(f).collect(),
        }
    }

    /// Java `stream.flatMap(mapper)`.
    pub fn flatMap<U, F>(self, mut f: F) -> JStream<U>
    where
        U: Clone + Default + std::fmt::Debug + 'static,
        F: FnMut(T) -> JStream<U>,
    {
        JStream {
            data: self.data.into_iter().flat_map(|x| f(x).data).collect(),
        }
    }

    /// Java `stream.reduce(identity, accumulator)`.
    pub fn reduce<F: FnMut(T, T) -> T>(self, identity: T, f: F) -> T {
        self.data.into_iter().fold(identity, f)
    }

    /// Java `stream.distinct()`.
    pub fn distinct(self) -> Self
    where
        T: PartialEq,
    {
        let mut seen: Vec<T> = Vec::new();
        let data = self
            .data
            .into_iter()
            .filter(|x| {
                if seen.contains(x) {
                    false
                } else {
                    seen.push(x.clone());
                    true
                }
            })
            .collect();
        Self { data }
    }

    /// Java `stream.collect(Collectors.toList())` — desugared in codegen to this method.
    pub fn collect_to_list(self) -> JList<T> {
        let mut list = JList::new();
        for item in self.data {
            list.add(item);
        }
        list
    }

    /// Java `stream.anyMatch(predicate)`.
    pub fn anyMatch<F: FnMut(T) -> bool>(self, pred: F) -> bool {
        self.data.into_iter().any(pred)
    }

    /// Java `stream.allMatch(predicate)`.
    pub fn allMatch<F: FnMut(T) -> bool>(self, pred: F) -> bool {
        self.data.into_iter().all(pred)
    }

    /// Java `stream.noneMatch(predicate)`.
    pub fn noneMatch<F: FnMut(T) -> bool>(self, pred: F) -> bool {
        !self.data.into_iter().any(pred)
    }

    /// Java `stream.peek(action)` — applies action to each element, passes stream through.
    pub fn peek<F: FnMut(&T)>(self, mut action: F) -> Self {
        for item in &self.data {
            action(item);
        }
        self
    }

    /// Convert the stream to a raw `Vec<T>`.
    pub fn toArray(self) -> Vec<T> {
        self.data
    }
}

impl<T: Clone + Default + std::fmt::Debug + 'static> JStream<T> {
    /// Java `stream.sorted(Comparator)` — sort with a custom comparator closure.
    /// The comparator follows Java convention: negative / zero / positive i32.
    pub fn sorted_with<F>(mut self, cmp: F) -> Self
    where
        F: Fn(&T, &T) -> i32,
    {
        self.data.sort_by(|a, b| {
            let r = cmp(a, b);
            if r < 0 {
                std::cmp::Ordering::Less
            } else if r > 0 {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });
        self
    }
}

impl<T: Clone + Default + std::fmt::Debug + Ord + 'static> JStream<T> {
    /// Java `stream.sorted()`.
    pub fn sorted(mut self) -> Self {
        self.data.sort();
        self
    }
}

impl<T: Clone + Default + std::fmt::Debug + Eq + std::hash::Hash + 'static> JStream<T> {
    /// Java `stream.collect(Collectors.toSet())`.
    pub fn collect_to_set(self) -> JSet<T> {
        let mut set = JSet::new();
        for item in self.data {
            set.add(item);
        }
        set
    }
}

impl<T: Clone + Default + std::fmt::Debug + std::fmt::Display + 'static> JStream<T> {
    /// Java `stream.collect(Collectors.joining())` — concatenate elements without separator.
    pub fn collect_joining(self, sep: JString) -> JString {
        let parts: Vec<String> = self.data.iter().map(|x| x.to_string()).collect();
        JString::from(parts.join(sep.as_str()).as_str())
    }

    /// Java `stream.collect(Collectors.joining(sep, prefix, suffix))`.
    pub fn collect_joining_full(self, sep: JString, prefix: JString, suffix: JString) -> JString {
        let parts: Vec<String> = self.data.iter().map(|x| x.to_string()).collect();
        let joined = format!("{}{}{}", prefix, parts.join(sep.as_str()), suffix);
        JString::from(joined.as_str())
    }
}

impl JStream<i32> {
    /// Java `IntStream.range(start, end)` — half-open range [start, end).
    pub fn int_range(start: i32, end: i32) -> Self {
        Self {
            data: (start..end).collect(),
        }
    }

    /// Java `IntStream.rangeClosed(start, end)` — closed range [start, end].
    pub fn int_range_closed(start: i32, end: i32) -> Self {
        Self {
            data: (start..=end).collect(),
        }
    }

    /// Java `IntStream.sum()` — sum all elements.
    pub fn sum(self) -> i32 {
        self.data.into_iter().sum()
    }
}

impl<T: Clone + Default + std::fmt::Debug + 'static> JStream<T> {
    /// Java `stream.collect(Collectors.toMap(keyFn, valFn))`.
    pub fn collect_to_map<K, V, KF, VF>(self, mut key_fn: KF, mut val_fn: VF) -> JMap<K, V>
    where
        K: Clone + Default + std::fmt::Debug + Eq + std::hash::Hash + 'static,
        V: Clone + Default + std::fmt::Debug + 'static,
        KF: FnMut(T) -> K,
        VF: FnMut(T) -> V,
    {
        let mut map = JMap::new();
        for item in self.data {
            let k = key_fn(item.clone());
            let v = val_fn(item);
            map.put(k, v);
        }
        map
    }

    /// Java `stream.collect(Collectors.groupingBy(classifier))`.
    pub fn collect_grouping_by<K, F>(self, mut classifier: F) -> JMap<K, JList<T>>
    where
        K: Clone + Default + std::fmt::Debug + Eq + std::hash::Hash + 'static,
        F: FnMut(T) -> K,
    {
        let mut map = JMap::new();
        for item in self.data {
            let key = classifier(item.clone());
            let mut group: JList<T> = map.getOrDefault(key.clone(), JList::new());
            group.add(item);
            map.put(key, group);
        }
        map
    }
}

impl<T: Clone + Default + std::fmt::Debug + std::fmt::Display + 'static> std::fmt::Display
    for JStream<T>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for (i, item) in self.data.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", item)?;
        }
        write!(f, "]")
    }
}
