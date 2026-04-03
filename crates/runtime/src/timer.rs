#![allow(non_snake_case)]
//! [`JTimer`] and [`JTimerTask`] — Rust equivalents of `java.util.Timer` and
//! `java.util.TimerTask`.
//!
//! `JTimerTask` is an empty base struct.  User subclasses override `run()`.
//! `JTimer` spawns a background thread per `schedule` call, checking a shared
//! `AtomicBool` for cancellation.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

// ── TimerTask ─────────────────────────────────────────────────────────────────

/// Base struct for `java.util.TimerTask`.
///
/// Generated user subclasses include this as a `_super` field and override
/// the `run()` method.
#[derive(Debug, Clone, Default)]
pub struct JTimerTask {}

impl JTimerTask {
    pub fn new() -> Self {
        Self {}
    }

    /// Supports `instanceof` checks in generated code.
    pub fn _instanceof(&self, type_name: &str) -> bool {
        type_name == "TimerTask"
    }
}

// ── Timer ─────────────────────────────────────────────────────────────────────

/// Rust equivalent of `java.util.Timer`.
///
/// Each `schedule` call spawns a background thread.  `cancel()` signals all
/// running threads to stop via a shared `AtomicBool`.
#[derive(Debug, Clone, Default)]
pub struct JTimer {
    cancelled: Arc<AtomicBool>,
}

impl JTimer {
    /// `new Timer()`
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// `timer.schedule(task, delayMs)` — one-shot execution after `delay_ms`.
    ///
    /// Called by codegen when `schedule` is invoked with two arguments.
    pub fn schedule_fn_once(
        &self,
        f: Box<dyn FnOnce() + Send + 'static>,
        delay_ms: i64,
    ) {
        let cancelled = Arc::clone(&self.cancelled);
        thread::spawn(move || {
            if delay_ms > 0 {
                thread::sleep(Duration::from_millis(delay_ms as u64));
            }
            if !cancelled.load(Ordering::Acquire) {
                f();
            }
        });
    }

    /// `timer.schedule(task, delayMs, periodMs)` — repeating execution.
    ///
    /// Called by codegen when `schedule` is invoked with three arguments.
    pub fn schedule_fn(
        &self,
        mut f: Box<dyn FnMut() + Send + 'static>,
        delay_ms: i64,
        period_ms: i64,
    ) {
        let cancelled = Arc::clone(&self.cancelled);
        thread::spawn(move || {
            if delay_ms > 0 && !cancelled.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(delay_ms as u64));
            }
            while !cancelled.load(Ordering::Acquire) {
                f();
                if cancelled.load(Ordering::Acquire) {
                    break;
                }
                thread::sleep(Duration::from_millis(period_ms as u64));
            }
        });
    }

    /// `timer.cancel()` — signals all scheduled tasks to stop.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// `timer.purge()` — no-op (no scheduled-task queue to purge).
    pub fn purge(&self) -> i32 {
        0
    }
}
