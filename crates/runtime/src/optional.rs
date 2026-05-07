#![allow(non_snake_case)]
//! [`JOptional<T>`] — Rust representation of `java.util.Optional<T>`.
//!
//! Mapping: `Optional<T>` → `JOptional<T>` (wraps `Option<T>`).

/// A container that may or may not contain a non-null value.
///
/// Mapping: `java.util.Optional<T>` → `JOptional<T>`.
#[derive(Debug, Clone, Default)]
pub struct JOptional<T: Clone + Default + std::fmt::Debug> {
    inner: Option<T>,
}

impl<T: Clone + Default + std::fmt::Debug> JOptional<T> {
    /// Java `Optional.of(value)` — wraps a non-null value.
    pub fn of(val: T) -> Self {
        Self { inner: Some(val) }
    }

    /// Java `Optional.empty()` — an absent value.
    pub fn empty() -> Self {
        Self { inner: None }
    }

    /// Java `Optional.ofNullable(value)` — wraps a potentially-null value.
    pub fn of_nullable(val: Option<T>) -> Self {
        Self { inner: val }
    }

    /// Java `opt.isPresent()`.
    pub fn isPresent(&self) -> bool {
        self.inner.is_some()
    }

    /// Java `opt.isEmpty()` (Java 11+).
    pub fn isEmpty(&self) -> bool {
        self.inner.is_none()
    }

    /// Java `opt.get()` — panics if absent.
    pub fn get(&self) -> T {
        self.inner.clone().expect("NoSuchElementException")
    }

    /// Java `opt.orElse(default)`.
    pub fn orElse(&self, default: T) -> T {
        self.inner.clone().unwrap_or(default)
    }

    /// Java `opt.ifPresent(consumer)`.
    pub fn ifPresent<F: Fn(&T)>(&self, f: F) {
        if let Some(x) = &self.inner {
            f(x);
        }
    }

    /// Java `opt.filter(predicate)`.
    pub fn filter<F: Fn(&T) -> bool>(&self, pred: F) -> Self {
        Self {
            inner: self.inner.as_ref().filter(|x| pred(x)).cloned(),
        }
    }

    /// Java `opt.map(mapper)`.
    pub fn map<U, F>(&self, f: F) -> JOptional<U>
    where
        U: Clone + Default + std::fmt::Debug,
        F: Fn(T) -> U,
    {
        JOptional {
            inner: self.inner.clone().map(f),
        }
    }

    /// Java `opt.flatMap(mapper)` — monadic bind.
    pub fn flatMap<U, F>(&self, f: F) -> JOptional<U>
    where
        U: Clone + Default + std::fmt::Debug,
        F: Fn(T) -> JOptional<U>,
    {
        match &self.inner {
            Some(x) => f(x.clone()),
            None => JOptional::empty(),
        }
    }

    /// Java `opt.orElseGet(supplier)` — lazy default.
    pub fn orElseGet<F: FnOnce() -> T>(&self, supplier: F) -> T {
        self.inner.clone().unwrap_or_else(supplier)
    }

    /// Java `opt.orElseThrow()` (no-arg, Java 10+) — panics if empty.
    pub fn orElseThrow(&self) -> T {
        self.inner
            .clone()
            .unwrap_or_else(|| panic!("JException:NoSuchElementException:No value present"))
    }

    /// Java `opt.orElseThrow(exceptionSupplier)` — calls supplier to get a message, then panics.
    ///
    /// The supplier is expected to return a displayable value whose string form
    /// becomes the exception message.  Codegen maps `opt.orElseThrow(supplier)`
    /// to this method when the supplier produces a `JString` / printable type.
    pub fn orElseThrowWith<F: FnOnce() -> String>(&self, msg_fn: F) -> T {
        self.inner
            .clone()
            .unwrap_or_else(|| panic!("JException:NoSuchElementException:{}", msg_fn()))
    }

    /// Java `opt.ifPresentOrElse(consumer, emptyAction)` (Java 9+).
    pub fn ifPresentOrElse<F: Fn(&T), R: Fn()>(&self, consumer: F, empty_action: R) {
        match &self.inner {
            Some(x) => consumer(x),
            None => empty_action(),
        }
    }

    /// Java `opt.or(supplier)` — return self if present, otherwise call supplier (Java 9+).
    pub fn or<F: FnOnce() -> JOptional<T>>(&self, supplier: F) -> JOptional<T> {
        if self.inner.is_some() {
            self.clone()
        } else {
            supplier()
        }
    }

    /// Java `opt.stream()` — a stream of 0 or 1 elements (Java 9+).
    pub fn stream(&self) -> crate::stream::JStream<T> {
        match &self.inner {
            Some(x) => crate::stream::JStream::new(vec![x.clone()]),
            None => crate::stream::JStream::new(vec![]),
        }
    }
}

impl<T: Clone + Default + std::fmt::Debug + std::fmt::Display> std::fmt::Display for JOptional<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.inner {
            Some(val) => write!(f, "Optional[{}]", val),
            None => write!(f, "Optional.empty"),
        }
    }
}
