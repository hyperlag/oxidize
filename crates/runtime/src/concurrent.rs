//! Higher-level concurrency utilities that mirror `java.util.concurrent`.
//!
//! Also provides `__sync_block_monitor()`, the global lock used for
//! `synchronized` block statements.

use std::sync::{Arc, Condvar, Mutex, OnceLock};

// ─── CountDownLatch ──────────────────────────────────────────────────────────

/// Mirrors `java.util.concurrent.CountDownLatch`.
#[derive(Clone, Debug)]
pub struct JCountDownLatch(Arc<(Mutex<i64>, Condvar)>);

#[allow(non_snake_case)]
impl JCountDownLatch {
    /// Create a new latch with an initial count of `count`.
    pub fn new(count: i32) -> Self {
        JCountDownLatch(Arc::new((Mutex::new(count as i64), Condvar::new())))
    }

    /// Decrement the count.  When it reaches zero all waiting threads are woken.
    pub fn countDown(&mut self) {
        let (lock, cvar) = &*self.0;
        let mut count = lock.lock().unwrap();
        if *count > 0 {
            *count -= 1;
        }
        if *count == 0 {
            cvar.notify_all();
        }
    }

    /// Block until the count reaches zero.
    pub fn await_(&mut self) {
        let (lock, cvar) = &*self.0;
        let mut count = lock.lock().unwrap();
        while *count > 0 {
            count = cvar.wait(count).unwrap();
        }
    }

    /// Return the current count.
    pub fn getCount(&self) -> i64 {
        let (lock, _) = &*self.0;
        *lock.lock().unwrap()
    }
}

// ─── Semaphore ───────────────────────────────────────────────────────────────

/// Mirrors `java.util.concurrent.Semaphore`.
#[derive(Clone, Debug)]
pub struct JSemaphore(Arc<(Mutex<i32>, Condvar)>);

#[allow(non_snake_case)]
impl JSemaphore {
    /// Create a semaphore with `permits` initial permits.
    pub fn new(permits: i32) -> Self {
        JSemaphore(Arc::new((Mutex::new(permits), Condvar::new())))
    }

    /// Acquire one permit, blocking if none are available.
    pub fn acquire(&mut self) {
        let (lock, cvar) = &*self.0;
        let mut permits = lock.lock().unwrap();
        while *permits <= 0 {
            permits = cvar.wait(permits).unwrap();
        }
        *permits -= 1;
    }

    /// Release one permit.
    pub fn release(&mut self) {
        let (lock, cvar) = &*self.0;
        let mut permits = lock.lock().unwrap();
        *permits += 1;
        cvar.notify_one();
    }

    /// Return the number of currently available permits.
    pub fn availablePermits(&self) -> i32 {
        let (lock, _) = &*self.0;
        *lock.lock().unwrap()
    }
}

// ─── synchronized-block monitor ──────────────────────────────────────────────

/// Returns the process-global `(Mutex, Condvar)` pair used for
/// `synchronized`-block statements.
///
/// Using a single global lock is a safe simplification: it ensures mutual
/// exclusion at the cost of reduced concurrency (acceptable because oxidize
/// targets single-class single-file translation where nested synchronization
/// is rare).
pub fn __sync_block_monitor() -> &'static (Mutex<()>, Condvar) {
    static MONITOR: OnceLock<(Mutex<()>, Condvar)> = OnceLock::new();
    MONITOR.get_or_init(|| (Mutex::new(()), Condvar::new()))
}
