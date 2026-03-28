//! Atomic reference types that mirror `java.util.concurrent.atomic`.
//!
//! All three types wrap an `Arc<AtomicT>` so they are cheaply `Clone`-able
//! (matching Java's reference semantics) and implement `Default`.

use std::sync::{
    atomic::{AtomicBool, AtomicI32, AtomicI64, Ordering},
    Arc,
};

// ─── AtomicInteger ───────────────────────────────────────────────────────────

/// Thread-safe atomic integer.
///
/// Mapping: `java.util.concurrent.atomic.AtomicInteger` → `JAtomicInteger`.
#[derive(Debug, Clone, Default)]
pub struct JAtomicInteger(Arc<AtomicI32>);

#[allow(non_snake_case)]
impl JAtomicInteger {
    pub fn new(val: i32) -> Self {
        JAtomicInteger(Arc::new(AtomicI32::new(val)))
    }

    pub fn get(&self) -> i32 {
        self.0.load(Ordering::SeqCst)
    }

    pub fn set(&self, val: i32) {
        self.0.store(val, Ordering::SeqCst);
    }

    pub fn getAndIncrement(&self) -> i32 {
        self.0.fetch_add(1, Ordering::SeqCst)
    }

    pub fn incrementAndGet(&self) -> i32 {
        self.0.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn getAndDecrement(&self) -> i32 {
        self.0.fetch_sub(1, Ordering::SeqCst)
    }

    pub fn decrementAndGet(&self) -> i32 {
        self.0.fetch_sub(1, Ordering::SeqCst) - 1
    }

    pub fn getAndAdd(&self, delta: i32) -> i32 {
        self.0.fetch_add(delta, Ordering::SeqCst)
    }

    pub fn addAndGet(&self, delta: i32) -> i32 {
        self.0.fetch_add(delta, Ordering::SeqCst) + delta
    }

    pub fn compareAndSet(&self, expect: i32, update: i32) -> bool {
        self.0
            .compare_exchange(expect, update, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn intValue(&self) -> i32 {
        self.get()
    }
}

impl std::fmt::Display for JAtomicInteger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get())
    }
}

// ─── AtomicLong ──────────────────────────────────────────────────────────────

/// Thread-safe atomic long.
///
/// Mapping: `java.util.concurrent.atomic.AtomicLong` → `JAtomicLong`.
#[derive(Debug, Clone, Default)]
pub struct JAtomicLong(Arc<AtomicI64>);

#[allow(non_snake_case)]
impl JAtomicLong {
    pub fn new(val: i64) -> Self {
        JAtomicLong(Arc::new(AtomicI64::new(val)))
    }

    pub fn get(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }

    pub fn set(&self, val: i64) {
        self.0.store(val, Ordering::SeqCst);
    }

    pub fn getAndIncrement(&self) -> i64 {
        self.0.fetch_add(1, Ordering::SeqCst)
    }

    pub fn incrementAndGet(&self) -> i64 {
        self.0.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn getAndDecrement(&self) -> i64 {
        self.0.fetch_sub(1, Ordering::SeqCst)
    }

    pub fn decrementAndGet(&self) -> i64 {
        self.0.fetch_sub(1, Ordering::SeqCst) - 1
    }

    pub fn getAndAdd(&self, delta: i64) -> i64 {
        self.0.fetch_add(delta, Ordering::SeqCst)
    }

    pub fn addAndGet(&self, delta: i64) -> i64 {
        self.0.fetch_add(delta, Ordering::SeqCst) + delta
    }

    pub fn compareAndSet(&self, expect: i64, update: i64) -> bool {
        self.0
            .compare_exchange(expect, update, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn longValue(&self) -> i64 {
        self.get()
    }
}

impl std::fmt::Display for JAtomicLong {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get())
    }
}

// ─── AtomicBoolean ───────────────────────────────────────────────────────────

/// Thread-safe atomic boolean.
///
/// Mapping: `java.util.concurrent.atomic.AtomicBoolean` → `JAtomicBoolean`.
#[derive(Debug, Clone, Default)]
pub struct JAtomicBoolean(Arc<AtomicBool>);

#[allow(non_snake_case)]
impl JAtomicBoolean {
    pub fn new(val: bool) -> Self {
        JAtomicBoolean(Arc::new(AtomicBool::new(val)))
    }

    pub fn get(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    pub fn set(&self, val: bool) {
        self.0.store(val, Ordering::SeqCst);
    }

    pub fn getAndSet(&self, val: bool) -> bool {
        self.0.swap(val, Ordering::SeqCst)
    }

    pub fn compareAndSet(&self, expect: bool, update: bool) -> bool {
        self.0
            .compare_exchange(expect, update, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }
}

impl std::fmt::Display for JAtomicBoolean {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get())
    }
}
