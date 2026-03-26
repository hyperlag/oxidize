#![allow(non_snake_case)]
//! [`JStream<T>`] — Rust representation of `java.util.stream.Stream<T>`.
//!
//! Mapping: `Stream<T>` → `JStream<T>` (backed by `Vec<T>` for eager evaluation).

use crate::list::JList;

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

    /// Java `stream.filter(predicate)`.
    pub fn filter<F: Fn(T) -> bool>(self, pred: F) -> Self {
        Self {
            data: self.data.into_iter().filter(|x| pred(x.clone())).collect(),
        }
    }

    /// Java `stream.limit(n)`.
    pub fn limit(self, n: i64) -> Self {
        Self {
            data: self.data.into_iter().take(n as usize).collect(),
        }
    }

    /// Java `stream.skip(n)`.
    pub fn skip(self, n: i64) -> Self {
        Self {
            data: self.data.into_iter().skip(n as usize).collect(),
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
    pub fn forEach<F: Fn(T)>(self, action: F) {
        self.data.into_iter().for_each(action);
    }

    /// Java `stream.map(mapper)` — transforms each element (same or different type).
    pub fn map<U, F>(self, f: F) -> JStream<U>
    where
        U: Clone + Default + std::fmt::Debug + 'static,
        F: Fn(T) -> U,
    {
        JStream {
            data: self.data.into_iter().map(f).collect(),
        }
    }

    /// Java `stream.flatMap(mapper)`.
    pub fn flatMap<U, F>(self, f: F) -> JStream<U>
    where
        U: Clone + Default + std::fmt::Debug + 'static,
        F: Fn(T) -> JStream<U>,
    {
        JStream {
            data: self.data.into_iter().flat_map(|x| f(x).data).collect(),
        }
    }

    /// Java `stream.reduce(identity, accumulator)`.
    pub fn reduce<F: Fn(T, T) -> T>(self, identity: T, f: F) -> T {
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

    /// Convert the stream to a raw `Vec<T>`.
    pub fn toArray(self) -> Vec<T> {
        self.data
    }
}

impl<T: Clone + Default + std::fmt::Debug + Ord + 'static> JStream<T> {
    /// Java `stream.sorted()`.
    pub fn sorted(mut self) -> Self {
        self.data.sort();
        self
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
