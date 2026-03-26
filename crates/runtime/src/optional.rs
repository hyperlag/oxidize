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
}

impl<T: Clone + Default + std::fmt::Debug + std::fmt::Display> std::fmt::Display for JOptional<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.inner {
            Some(val) => write!(f, "Optional[{}]", val),
            None => write!(f, "Optional.empty"),
        }
    }
}
