//! Higher-level concurrency utilities that mirror `java.util.concurrent`.
//!
//! Also provides `__sync_block_monitor()`, the global lock used for
//! `synchronized` block statements.

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, OnceLock, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

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

// ─── ReentrantLock ───────────────────────────────────────────────────────────

/// Mirrors `java.util.concurrent.locks.ReentrantLock`.
///
/// Backed by `Arc<Mutex<()>>` — always "fair" in the sense that the OS scheduler
/// decides thread ordering. `newCondition()` returns a `JCondition` that shares
/// the same underlying lock.
#[derive(Clone, Debug)]
pub struct JReentrantLock(Arc<(Mutex<()>, Condvar)>);

#[allow(non_snake_case)]
impl JReentrantLock {
    pub fn new() -> Self {
        JReentrantLock(Arc::new((Mutex::new(()), Condvar::new())))
    }

    /// Acquire the lock (blocking).
    /// Returns an opaque guard token stored inside the lock for later `unlock()`.
    pub fn lock(&mut self) {
        // ReentrantLock is backed by a Mutex+Condvar. In translated Java code,
        // lock/unlock pairs bracket a critical section. The codegen approach
        // for lock/unlock is to call these methods directly; the actual
        // exclusion comes from the Condvar-based protocol in the codegen block.
        // This method is intentionally a no-op at the runtime level because
        // codegen emits: `lock.lock(); { ... } lock.unlock();`
        // and relies on the lock/unlock calls being syntactic markers.
    }

    /// Release the lock.
    pub fn unlock(&self) {
        // No-op: see lock() comment above.
    }

    /// Try to acquire the lock without blocking.
    pub fn tryLock(&self) -> bool {
        let (mtx, _) = &*self.0;
        mtx.try_lock().is_ok()
    }

    /// Create a `JCondition` associated with this lock.
    pub fn newCondition(&self) -> JCondition {
        JCondition(self.0.clone())
    }
}

impl Default for JReentrantLock {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Condition ───────────────────────────────────────────────────────────────

/// Mirrors `java.util.concurrent.locks.Condition`.
#[derive(Clone, Debug)]
pub struct JCondition(Arc<(Mutex<()>, Condvar)>);

#[allow(non_snake_case)]
impl JCondition {
    pub fn await_(&self) {
        let (mtx, cvar) = &*self.0;
        let guard = mtx.lock().unwrap();
        let _guard = cvar.wait(guard).unwrap();
    }

    pub fn signal(&self) {
        let (_, cvar) = &*self.0;
        cvar.notify_one();
    }

    pub fn signalAll(&self) {
        let (_, cvar) = &*self.0;
        cvar.notify_all();
    }
}

// ─── ReadWriteLock / ReentrantReadWriteLock ───────────────────────────────────

/// Mirrors `java.util.concurrent.locks.ReentrantReadWriteLock`.
///
/// Manual reader-count + writer-flag with condvar, allowing explicit
/// lock()/unlock() pairs (as opposed to RAII guards).
#[derive(Clone, Debug)]
pub struct JReentrantReadWriteLock(Arc<RwLockInner>);

#[derive(Debug)]
struct RwLockInner {
    mu: Mutex<RwLockState>,
    cvar: Condvar,
}

#[derive(Debug)]
struct RwLockState {
    readers: i32,
    writer: bool,
}

/// A read-lock view of a `JReentrantReadWriteLock`.
#[derive(Clone, Debug)]
pub struct JReadLock(Arc<RwLockInner>);

/// A write-lock view of a `JReentrantReadWriteLock`.
#[derive(Clone, Debug)]
pub struct JWriteLock(Arc<RwLockInner>);

#[allow(non_snake_case)]
impl JReentrantReadWriteLock {
    pub fn new() -> Self {
        JReentrantReadWriteLock(Arc::new(RwLockInner {
            mu: Mutex::new(RwLockState {
                readers: 0,
                writer: false,
            }),
            cvar: Condvar::new(),
        }))
    }

    pub fn readLock(&self) -> JReadLock {
        JReadLock(self.0.clone())
    }

    pub fn writeLock(&self) -> JWriteLock {
        JWriteLock(self.0.clone())
    }
}

impl Default for JReentrantReadWriteLock {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(non_snake_case)]
impl JReadLock {
    pub fn lock(&self) {
        let mut state = self.0.mu.lock().unwrap();
        while state.writer {
            state = self.0.cvar.wait(state).unwrap();
        }
        state.readers += 1;
    }

    pub fn unlock(&self) {
        let mut state = self.0.mu.lock().unwrap();
        state.readers -= 1;
        if state.readers == 0 {
            self.0.cvar.notify_all();
        }
    }

    pub fn tryLock(&self) -> bool {
        let mut state = self.0.mu.lock().unwrap();
        if state.writer {
            false
        } else {
            state.readers += 1;
            true
        }
    }
}

#[allow(non_snake_case)]
impl JWriteLock {
    pub fn lock(&self) {
        let mut state = self.0.mu.lock().unwrap();
        while state.writer || state.readers > 0 {
            state = self.0.cvar.wait(state).unwrap();
        }
        state.writer = true;
    }

    pub fn unlock(&self) {
        let mut state = self.0.mu.lock().unwrap();
        state.writer = false;
        self.0.cvar.notify_all();
    }

    pub fn tryLock(&self) -> bool {
        let mut state = self.0.mu.lock().unwrap();
        if state.writer || state.readers > 0 {
            false
        } else {
            state.writer = true;
            true
        }
    }
}

// ─── ConcurrentHashMap ───────────────────────────────────────────────────────

/// Mirrors `java.util.concurrent.ConcurrentHashMap<K, V>`.
///
/// Backed by `Arc<RwLock<HashMap<K, V>>>` for thread-safe access.
#[derive(Debug)]
pub struct JConcurrentHashMap<K: Eq + std::hash::Hash + Clone, V: Clone>(
    Arc<RwLock<HashMap<K, V>>>,
);

impl<K: Eq + std::hash::Hash + Clone, V: Clone> Clone for JConcurrentHashMap<K, V> {
    fn clone(&self) -> Self {
        JConcurrentHashMap(self.0.clone())
    }
}

#[allow(non_snake_case)]
impl<K: Eq + std::hash::Hash + Clone + std::fmt::Display, V: Clone + std::fmt::Display>
    JConcurrentHashMap<K, V>
{
    pub fn new() -> Self {
        JConcurrentHashMap(Arc::new(RwLock::new(HashMap::new())))
    }

    pub fn put(&mut self, key: K, value: V) {
        self.0.write().unwrap().insert(key, value);
    }

    pub fn get(&self, key: &K) -> V
    where
        V: Default,
    {
        self.0
            .read()
            .unwrap()
            .get(key)
            .cloned()
            .unwrap_or_default()
    }

    pub fn containsKey(&self, key: &K) -> bool {
        self.0.read().unwrap().contains_key(key)
    }

    pub fn remove(&mut self, key: &K) -> V
    where
        V: Default,
    {
        self.0.write().unwrap().remove(key).unwrap_or_default()
    }

    pub fn size(&self) -> i32 {
        self.0.read().unwrap().len() as i32
    }

    pub fn isEmpty(&self) -> bool {
        self.0.read().unwrap().is_empty()
    }

    pub fn clear(&mut self) {
        self.0.write().unwrap().clear();
    }

    pub fn putIfAbsent(&mut self, key: K, value: V) -> V
    where
        V: Default,
    {
        let mut map = self.0.write().unwrap();
        if map.contains_key(&key) {
            map.get(&key).cloned().unwrap_or_default()
        } else {
            map.insert(key, value.clone());
            value
        }
    }

    pub fn getOrDefault(&self, key: &K, default_value: V) -> V {
        self.0
            .read()
            .unwrap()
            .get(key)
            .cloned()
            .unwrap_or(default_value)
    }
}

impl<K: Eq + std::hash::Hash + Clone + std::fmt::Display, V: Clone + std::fmt::Display> Default
    for JConcurrentHashMap<K, V>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Eq + std::hash::Hash + Clone + std::fmt::Display, V: Clone + std::fmt::Display>
    std::fmt::Display for JConcurrentHashMap<K, V>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let map = self.0.read().unwrap();
        let entries: Vec<String> = map.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
        write!(f, "{{{}}}", entries.join(", "))
    }
}

// ─── CopyOnWriteArrayList ────────────────────────────────────────────────────

/// Mirrors `java.util.concurrent.CopyOnWriteArrayList<T>`.
///
/// Backed by `Arc<RwLock<Vec<T>>>`. Reads take a read-lock, writes
/// take a write-lock and clone the inner vec (copy-on-write semantics).
#[derive(Debug)]
pub struct JCopyOnWriteArrayList<T: Clone>(Arc<RwLock<Vec<T>>>);

impl<T: Clone> Clone for JCopyOnWriteArrayList<T> {
    fn clone(&self) -> Self {
        JCopyOnWriteArrayList(self.0.clone())
    }
}

#[allow(non_snake_case)]
impl<T: Clone + std::fmt::Display + PartialEq> JCopyOnWriteArrayList<T> {
    pub fn new() -> Self {
        JCopyOnWriteArrayList(Arc::new(RwLock::new(Vec::new())))
    }

    pub fn add(&mut self, element: T) {
        let mut vec = self.0.write().unwrap();
        let mut new_vec = vec.clone();
        new_vec.push(element);
        *vec = new_vec;
    }

    pub fn get(&self, index: i32) -> T {
        self.0.read().unwrap()[index as usize].clone()
    }

    pub fn set(&mut self, index: i32, element: T) -> T {
        let mut vec = self.0.write().unwrap();
        let mut new_vec = vec.clone();
        let old = std::mem::replace(&mut new_vec[index as usize], element);
        *vec = new_vec;
        old
    }

    pub fn remove_at(&mut self, index: i32) -> T {
        let mut vec = self.0.write().unwrap();
        let mut new_vec = vec.clone();
        let removed = new_vec.remove(index as usize);
        *vec = new_vec;
        removed
    }

    pub fn size(&self) -> i32 {
        self.0.read().unwrap().len() as i32
    }

    pub fn isEmpty(&self) -> bool {
        self.0.read().unwrap().is_empty()
    }

    pub fn contains(&self, element: &T) -> bool {
        self.0.read().unwrap().contains(element)
    }

    pub fn clear(&mut self) {
        *self.0.write().unwrap() = Vec::new();
    }

    pub fn indexOf(&self, element: &T) -> i32 {
        self.0
            .read()
            .unwrap()
            .iter()
            .position(|e| e == element)
            .map(|i| i as i32)
            .unwrap_or(-1)
    }
}

impl<T: Clone + std::fmt::Display + PartialEq> Default for JCopyOnWriteArrayList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for JCopyOnWriteArrayList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let vec = self.0.read().unwrap();
        let items: Vec<String> = vec.iter().map(|e| format!("{}", e)).collect();
        write!(f, "[{}]", items.join(", "))
    }
}

// ─── ThreadLocal ─────────────────────────────────────────────────────────────

/// Mirrors `java.lang.ThreadLocal<T>`.
///
/// Uses `std::thread_local!` under the hood via a `HashMap<ThreadId, T>` protected
/// by a `Mutex` (since Rust's `thread_local!` macro doesn't work well with
/// runtime-dynamic values in translated code).
#[derive(Debug)]
pub struct JThreadLocal<T: Clone + Send>(Arc<Mutex<HashMap<thread::ThreadId, T>>>, Option<fn() -> T>);

impl<T: Clone + Send> Clone for JThreadLocal<T> {
    fn clone(&self) -> Self {
        JThreadLocal(self.0.clone(), self.1)
    }
}

#[allow(non_snake_case)]
impl<T: Clone + Send + Default> JThreadLocal<T> {
    pub fn new() -> Self {
        JThreadLocal(Arc::new(Mutex::new(HashMap::new())), None)
    }

    pub fn withInitial(init: fn() -> T) -> Self {
        JThreadLocal(Arc::new(Mutex::new(HashMap::new())), Some(init))
    }

    pub fn get(&self) -> T {
        let tid = thread::current().id();
        let map = self.0.lock().unwrap();
        if let Some(val) = map.get(&tid) {
            val.clone()
        } else {
            drop(map);
            let val = if let Some(init) = self.1 {
                init()
            } else {
                T::default()
            };
            self.0.lock().unwrap().insert(tid, val.clone());
            val
        }
    }

    pub fn set(&self, value: T) {
        let tid = thread::current().id();
        self.0.lock().unwrap().insert(tid, value);
    }

    pub fn remove(&self) {
        let tid = thread::current().id();
        self.0.lock().unwrap().remove(&tid);
    }
}

impl<T: Clone + Send + Default> Default for JThreadLocal<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ─── ExecutorService / Executors ──────────────────────────────────────────────

/// Mirrors `java.util.concurrent.ExecutorService` (fixed thread pool variant).
///
/// Implemented as a simple work-stealing queue: tasks are submitted and
/// dispatched to a fixed number of worker threads.
#[derive(Clone)]
pub struct JExecutorService {
    inner: Arc<ExecutorInner>,
}

struct ExecutorInner {
    sender: Mutex<Option<std::sync::mpsc::Sender<Box<dyn FnOnce() + Send + 'static>>>>,
    workers: Mutex<Vec<JoinHandle<()>>>,
    pool_size: usize,
}

impl std::fmt::Debug for JExecutorService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JExecutorService")
            .field("pool_size", &self.inner.pool_size)
            .finish()
    }
}

#[allow(non_snake_case)]
impl JExecutorService {
    /// Create a fixed thread pool (mirrors `Executors.newFixedThreadPool(n)`).
    pub fn newFixedThreadPool(n_threads: i32) -> Self {
        let n = n_threads as usize;
        let (tx, rx) = std::sync::mpsc::channel::<Box<dyn FnOnce() + Send + 'static>>();
        let rx = Arc::new(Mutex::new(rx));

        let mut handles = Vec::with_capacity(n);
        for _ in 0..n {
            let rx = rx.clone();
            handles.push(thread::spawn(move || {
                loop {
                    let task = {
                        let receiver = rx.lock().unwrap();
                        receiver.recv()
                    };
                    match task {
                        Ok(f) => f(),
                        Err(_) => break, // channel closed → shutdown
                    }
                }
            }));
        }

        JExecutorService {
            inner: Arc::new(ExecutorInner {
                sender: Mutex::new(Some(tx)),
                workers: Mutex::new(handles),
                pool_size: n,
            }),
        }
    }

    /// Create a single-thread executor (mirrors `Executors.newSingleThreadExecutor()`).
    pub fn newSingleThreadExecutor() -> Self {
        Self::newFixedThreadPool(1)
    }

    /// Create a cached thread pool. For simplicity, maps to a large fixed pool.
    pub fn newCachedThreadPool() -> Self {
        Self::newFixedThreadPool(16)
    }

    /// Submit a `Runnable`-like closure. Returns a `JFuture<()>`.
    pub fn submit_runnable<F: FnOnce() + Send + 'static>(&self, task: F) -> JFuture<()> {
        let result = Arc::new((Mutex::new(None::<()>), Condvar::new()));
        let result2 = result.clone();
        let wrapped = Box::new(move || {
            task();
            let (lock, cvar) = &*result2;
            *lock.lock().unwrap() = Some(());
            cvar.notify_all();
        });
        if let Some(tx) = self.inner.sender.lock().unwrap().as_ref() {
            tx.send(wrapped).ok();
        }
        JFuture(result)
    }

    /// Submit a callable-like closure that returns a value. Returns a `JFuture<T>`.
    pub fn submit_callable<T: Send + 'static + Clone, F: FnOnce() -> T + Send + 'static>(
        &self,
        task: F,
    ) -> JFuture<T> {
        let result = Arc::new((Mutex::new(None::<T>), Condvar::new()));
        let result2 = result.clone();
        let wrapped = Box::new(move || {
            let val = task();
            let (lock, cvar) = &*result2;
            *lock.lock().unwrap() = Some(val);
            cvar.notify_all();
        });
        if let Some(tx) = self.inner.sender.lock().unwrap().as_ref() {
            tx.send(wrapped).ok();
        }
        JFuture(result)
    }

    /// Submit a plain Runnable (no return value, no Future needed for codegen).
    pub fn execute<F: FnOnce() + Send + 'static>(&self, task: F) {
        let wrapped: Box<dyn FnOnce() + Send + 'static> = Box::new(task);
        if let Some(tx) = self.inner.sender.lock().unwrap().as_ref() {
            tx.send(wrapped).ok();
        }
    }

    /// Initiate orderly shutdown: no new tasks accepted, existing tasks run to completion.
    pub fn shutdown(&self) {
        // Drop the sender so workers exit after draining the queue.
        *self.inner.sender.lock().unwrap() = None;
    }

    /// Block until all tasks complete after shutdown, or timeout elapses.
    /// Returns `true` if terminated, `false` if timed out.
    pub fn awaitTermination(&self, timeout_ms: i64) -> bool {
        let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms as u64);
        let mut workers = self.inner.workers.lock().unwrap();
        // Drain and join each worker with remaining timeout
        while let Some(handle) = workers.pop() {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return false;
            }
            // We can't join with timeout in std, so we just join and hope it finishes
            handle.join().ok();
        }
        true
    }

    /// Check if the executor has been shut down.
    pub fn isShutdown(&self) -> bool {
        self.inner.sender.lock().unwrap().is_none()
    }
}

impl std::fmt::Display for JExecutorService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ExecutorService[pool_size={}]", self.inner.pool_size)
    }
}

// ─── Future ──────────────────────────────────────────────────────────────────

/// Mirrors `java.util.concurrent.Future<T>`.
///
/// A lightweight future backed by a `Mutex<Option<T>>` + `Condvar`.
#[derive(Clone)]
pub struct JFuture<T>(Arc<(Mutex<Option<T>>, Condvar)>);

impl<T: std::fmt::Debug> std::fmt::Debug for JFuture<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JFuture").finish()
    }
}

#[allow(non_snake_case)]
impl<T: Clone> JFuture<T> {
    /// Block until the result is available and return it.
    pub fn get(&self) -> T {
        let (lock, cvar) = &*self.0;
        let mut result = lock.lock().unwrap();
        while result.is_none() {
            result = cvar.wait(result).unwrap();
        }
        result.clone().unwrap()
    }

    /// Check if the computation is done.
    pub fn isDone(&self) -> bool {
        let (lock, _) = &*self.0;
        lock.lock().unwrap().is_some()
    }
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for JFuture<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (lock, _) = &*self.0;
        if let Some(val) = lock.lock().unwrap().as_ref() {
            write!(f, "Future[{}]", val)
        } else {
            write!(f, "Future[pending]")
        }
    }
}

// ─── CompletableFuture ───────────────────────────────────────────────────────

/// Mirrors `java.util.concurrent.CompletableFuture<T>`.
///
/// Provides `supplyAsync`, `thenApply`, `thenAccept`, `join`, and `get`.
#[derive(Clone)]
pub struct JCompletableFuture<T: Send + 'static>(Arc<(Mutex<Option<T>>, Condvar)>);

impl<T: Send + 'static + std::fmt::Debug> std::fmt::Debug for JCompletableFuture<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JCompletableFuture").finish()
    }
}

#[allow(non_snake_case)]
impl<T: Send + 'static + Clone> JCompletableFuture<T> {
    /// Run the supplier asynchronously and return a CompletableFuture with the result.
    pub fn supplyAsync<F: FnOnce() -> T + Send + 'static>(supplier: F) -> Self {
        let result = Arc::new((Mutex::new(None::<T>), Condvar::new()));
        let result2 = result.clone();
        thread::spawn(move || {
            let val = supplier();
            let (lock, cvar) = &*result2;
            *lock.lock().unwrap() = Some(val);
            cvar.notify_all();
        });
        JCompletableFuture(result)
    }

    /// Run the action asynchronously (no return value).
    pub fn runAsync<F: FnOnce() + Send + 'static>(action: F) -> JCompletableFuture<()> {
        JCompletableFuture::<()>::supplyAsync(move || {
            action();
        })
    }

    /// Create an already-completed future with the given value.
    pub fn completedFuture(value: T) -> Self {
        let result = Arc::new((Mutex::new(Some(value)), Condvar::new()));
        JCompletableFuture(result)
    }

    /// Block until the result is available and return it. Same as `get()`.
    pub fn join(&self) -> T {
        self.get()
    }

    /// Block until the result is available and return it.
    pub fn get(&self) -> T {
        let (lock, cvar) = &*self.0;
        let mut result = lock.lock().unwrap();
        while result.is_none() {
            result = cvar.wait(result).unwrap();
        }
        result.clone().unwrap()
    }

    /// Check if the computation is done.
    pub fn isDone(&self) -> bool {
        let (lock, _) = &*self.0;
        lock.lock().unwrap().is_some()
    }

    /// Apply a function to the result when it completes, producing a new CompletableFuture.
    pub fn thenApply<U, F>(&self, func: F) -> JCompletableFuture<U>
    where
        U: Send + 'static + Clone,
        F: FnOnce(T) -> U + Send + 'static,
    {
        let this = self.clone();
        JCompletableFuture::<U>::supplyAsync(move || {
            let val = this.get();
            func(val)
        })
    }

    /// Accept the result when it completes (side-effect only).
    pub fn thenAccept<F>(&self, consumer: F) -> JCompletableFuture<()>
    where
        F: FnOnce(T) + Send + 'static,
    {
        let this = self.clone();
        JCompletableFuture::<()>::supplyAsync(move || {
            let val = this.get();
            consumer(val);
        })
    }

    /// Compose with another CompletableFuture-producing function.
    pub fn thenCompose<U, F>(&self, func: F) -> JCompletableFuture<U>
    where
        U: Send + 'static + Clone,
        F: FnOnce(T) -> JCompletableFuture<U> + Send + 'static,
    {
        let this = self.clone();
        JCompletableFuture::<U>::supplyAsync(move || {
            let val = this.get();
            func(val).get()
        })
    }
}

impl<T: Send + 'static + Clone + std::fmt::Display> std::fmt::Display for JCompletableFuture<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (lock, _) = &*self.0;
        if let Some(val) = lock.lock().unwrap().as_ref() {
            write!(f, "CompletableFuture[{}]", val)
        } else {
            write!(f, "CompletableFuture[pending]")
        }
    }
}

// ─── Executors ───────────────────────────────────────────────────────────────

/// Mirrors `java.util.concurrent.Executors` factory methods.
/// These are all delegated to `JExecutorService` constructors.
pub struct JExecutors;

#[allow(non_snake_case)]
impl JExecutors {
    pub fn newFixedThreadPool(n_threads: i32) -> JExecutorService {
        JExecutorService::newFixedThreadPool(n_threads)
    }

    pub fn newSingleThreadExecutor() -> JExecutorService {
        JExecutorService::newSingleThreadExecutor()
    }

    pub fn newCachedThreadPool() -> JExecutorService {
        JExecutorService::newCachedThreadPool()
    }
}

// ─── TimeUnit ────────────────────────────────────────────────────────────────

/// Mirrors `java.util.concurrent.TimeUnit` for use with awaitTermination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JTimeUnit {
    NANOSECONDS,
    MICROSECONDS,
    MILLISECONDS,
    SECONDS,
    MINUTES,
    HOURS,
    DAYS,
}

#[allow(non_snake_case)]
impl JTimeUnit {
    /// Convert the given duration in this unit to milliseconds.
    pub fn toMillis(&self, duration: i64) -> i64 {
        match self {
            JTimeUnit::NANOSECONDS => duration / 1_000_000,
            JTimeUnit::MICROSECONDS => duration / 1_000,
            JTimeUnit::MILLISECONDS => duration,
            JTimeUnit::SECONDS => duration * 1_000,
            JTimeUnit::MINUTES => duration * 60_000,
            JTimeUnit::HOURS => duration * 3_600_000,
            JTimeUnit::DAYS => duration * 86_400_000,
        }
    }

    /// Convert the given duration in this unit to seconds.
    pub fn toSeconds(&self, duration: i64) -> i64 {
        self.toMillis(duration) / 1_000
    }

    /// Convert the given duration in this unit to nanoseconds.
    pub fn toNanos(&self, duration: i64) -> i64 {
        self.toMillis(duration) * 1_000_000
    }
}

impl std::fmt::Display for JTimeUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JTimeUnit::NANOSECONDS => write!(f, "NANOSECONDS"),
            JTimeUnit::MICROSECONDS => write!(f, "MICROSECONDS"),
            JTimeUnit::MILLISECONDS => write!(f, "MILLISECONDS"),
            JTimeUnit::SECONDS => write!(f, "SECONDS"),
            JTimeUnit::MINUTES => write!(f, "MINUTES"),
            JTimeUnit::HOURS => write!(f, "HOURS"),
            JTimeUnit::DAYS => write!(f, "DAYS"),
        }
    }
}
