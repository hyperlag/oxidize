//! `JThread` — thin wrapper around `std::thread` that mirrors the
//! `Thread` / `Runnable` pattern from Java.

use std::thread::{self, JoinHandle};

/// A Java-style thread that stores its task until `start()` is called.
///
/// ```ignore
/// let mut t = JThread::new(|| do_something());
/// t.start();  // spawns the OS thread
/// t.join();   // waits for completion
/// ```
pub struct JThread {
    task: Option<Box<dyn FnOnce() + Send + 'static>>,
    handle: Option<JoinHandle<()>>,
}

impl JThread {
    /// Create a new (not-yet-started) thread with the given closure.
    pub fn new<F: FnOnce() + Send + 'static>(f: F) -> Self {
        JThread {
            task: Some(Box::new(f)),
            handle: None,
        }
    }

    /// Start the thread.  Has no effect if called more than once.
    pub fn start(&mut self) {
        if let Some(task) = self.task.take() {
            self.handle = Some(thread::spawn(task));
        }
    }

    /// Wait for the thread to finish.  Ignores panics in the child thread.
    pub fn join(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.join().ok();
        }
    }

    /// Sleep the *current* thread for `millis` milliseconds.
    pub fn sleep(millis: i64) {
        thread::sleep(std::time::Duration::from_millis(millis as u64));
    }
}

impl std::fmt::Debug for JThread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JThread")
            .field("started", &self.task.is_none())
            .field("running", &self.handle.is_some())
            .finish()
    }
}
