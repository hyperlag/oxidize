//! Higher-level concurrency utilities that mirror `java.util.concurrent`.
//!
//! Also provides `__sync_block_monitor()`, the global lock used for
//! `synchronized` block statements.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, OnceLock, RwLock};
use std::thread::{self, JoinHandle};

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

#[derive(Debug)]
struct ReentrantLockState {
    owner: Option<thread::ThreadId>,
    hold_count: u32,
}

#[derive(Debug)]
struct ReentrantLockInner {
    state: Mutex<ReentrantLockState>,
    /// Notified when the lock is released so waiting threads can try to acquire.
    lock_cvar: Condvar,
    /// Notified by `signal`/`signalAll` to wake threads in `await_`.
    cond_cvar: Condvar,
}

/// Mirrors `java.util.concurrent.locks.ReentrantLock`.
///
/// Supports full reentrant semantics: the owning thread may call `lock()`
/// multiple times, with a matching number of `unlock()` calls required to
/// release.  `newCondition()` returns a `JCondition` backed by the same lock.
#[derive(Clone, Debug)]
pub struct JReentrantLock(Arc<ReentrantLockInner>);

#[allow(non_snake_case)]
impl JReentrantLock {
    pub fn new() -> Self {
        JReentrantLock(Arc::new(ReentrantLockInner {
            state: Mutex::new(ReentrantLockState {
                owner: None,
                hold_count: 0,
            }),
            lock_cvar: Condvar::new(),
            cond_cvar: Condvar::new(),
        }))
    }

    /// Acquire the lock, blocking until it is available.
    /// If the calling thread already holds the lock, the hold count is incremented.
    pub fn lock(&mut self) {
        let tid = thread::current().id();
        let mut state = self.0.state.lock().unwrap();
        loop {
            if state.owner == Some(tid) {
                state.hold_count += 1;
                return;
            } else if state.owner.is_none() {
                state.owner = Some(tid);
                state.hold_count = 1;
                return;
            }
            state = self.0.lock_cvar.wait(state).unwrap();
        }
    }

    /// Release the lock.  If the lock was acquired reentrantly, the hold count
    /// is decremented; the lock is only released when the count reaches zero.
    pub fn unlock(&mut self) {
        let tid = thread::current().id();
        let mut state = self.0.state.lock().unwrap();
        assert_eq!(
            state.owner,
            Some(tid),
            "IllegalMonitorStateException: unlock() called by a thread that does not hold the lock"
        );
        state.hold_count -= 1;
        if state.hold_count == 0 {
            state.owner = None;
            self.0.lock_cvar.notify_one();
        }
    }

    /// Try to acquire the lock without blocking.
    /// Returns `true` if the lock was acquired (or was already held by this thread).
    pub fn tryLock(&self) -> bool {
        let tid = thread::current().id();
        let mut state = self.0.state.lock().unwrap();
        if state.owner == Some(tid) {
            state.hold_count += 1;
            true
        } else if state.owner.is_none() {
            state.owner = Some(tid);
            state.hold_count = 1;
            true
        } else {
            false
        }
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
///
/// All operations require the associated `JReentrantLock` to be held by the
/// calling thread.  `await_` atomically releases all holds on the lock, waits
/// for a `signal`/`signalAll`, and then re-acquires the lock before returning.
#[derive(Clone, Debug)]
pub struct JCondition(Arc<ReentrantLockInner>);

#[allow(non_snake_case)]
impl JCondition {
    /// Atomically release the lock, wait for a signal, then re-acquire the lock.
    pub fn await_(&self) {
        let tid = thread::current().id();
        let mut state = self.0.state.lock().unwrap();
        assert_eq!(
            state.owner,
            Some(tid),
            "IllegalMonitorStateException: await_() called by a thread that does not hold the lock"
        );
        // Save and release all holds on the lock.
        let saved_hold_count = state.hold_count;
        state.owner = None;
        state.hold_count = 0;
        // Wake a thread that may be waiting to acquire the lock.
        self.0.lock_cvar.notify_one();
        // Wait for a condition signal.
        state = self.0.cond_cvar.wait(state).unwrap();
        // Re-acquire the lock for this thread (may need to wait if another thread
        // grabbed it while we were waking up).
        loop {
            if state.owner.is_none() {
                state.owner = Some(tid);
                state.hold_count = saved_hold_count;
                return;
            }
            state = self.0.lock_cvar.wait(state).unwrap();
        }
    }

    /// Wake one thread waiting on this condition.
    pub fn signal(&self) {
        self.0.cond_cvar.notify_one();
    }

    /// Wake all threads waiting on this condition.
    pub fn signalAll(&self) {
        self.0.cond_cvar.notify_all();
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
impl<K: Eq + std::hash::Hash + Clone, V: Clone> JConcurrentHashMap<K, V> {
    pub fn new() -> Self {
        JConcurrentHashMap(Arc::new(RwLock::new(HashMap::new())))
    }

    pub fn put(&mut self, key: K, value: V) {
        self.0.write().unwrap().insert(key, value);
    }

    /// # Panics
    /// Panics if the key is not present (analogous to Java's
    /// `NullPointerException` when auto-unboxing a `null` returned by
    /// `ConcurrentHashMap.get`).
    pub fn get(&self, key: &K) -> V {
        self.0
            .read()
            .unwrap()
            .get(key)
            .cloned()
            .unwrap_or_else(|| panic!("NullPointerException: key not found in ConcurrentHashMap"))
    }

    pub fn containsKey(&self, key: &K) -> bool {
        self.0.read().unwrap().contains_key(key)
    }

    /// # Panics
    /// Panics if the key is not present.
    pub fn remove(&mut self, key: &K) -> V {
        self.0
            .write()
            .unwrap()
            .remove(key)
            .unwrap_or_else(|| panic!("NullPointerException: key not found in ConcurrentHashMap"))
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

    /// Inserts the key-value pair if the key is absent.
    /// Returns the existing value if the key was already present, or
    /// `V::default()` (modelling Java's `null`) if the pair was newly inserted.
    pub fn putIfAbsent(&mut self, key: K, value: V) -> V
    where
        V: Default,
    {
        let mut map = self.0.write().unwrap();
        if let Some(existing) = map.get(&key) {
            existing.clone()
        } else {
            map.insert(key, value);
            V::default()
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

impl<K: Eq + std::hash::Hash + Clone, V: Clone> Default for JConcurrentHashMap<K, V> {
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
impl<T: Clone> JCopyOnWriteArrayList<T> {
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
        let vec = self.0.read().unwrap();
        let len = vec.len();
        let idx = index as usize;
        if idx >= len {
            panic!(
                "IndexOutOfBoundsException: index {} out of bounds for length {}",
                index, len
            );
        }
        vec[idx].clone()
    }

    pub fn set(&mut self, index: i32, element: T) -> T {
        let mut vec = self.0.write().unwrap();
        let len = vec.len();
        let idx = index as usize;
        if idx >= len {
            panic!(
                "IndexOutOfBoundsException: index {} out of bounds for length {}",
                index, len
            );
        }
        let mut new_vec = vec.clone();
        let old = std::mem::replace(&mut new_vec[idx], element);
        *vec = new_vec;
        old
    }

    pub fn remove_at(&mut self, index: i32) -> T {
        let mut vec = self.0.write().unwrap();
        let len = vec.len();
        let idx = index as usize;
        if idx >= len {
            panic!(
                "IndexOutOfBoundsException: index {} out of bounds for length {}",
                index, len
            );
        }
        let mut new_vec = vec.clone();
        let removed = new_vec.remove(idx);
        *vec = new_vec;
        removed
    }

    pub fn size(&self) -> i32 {
        self.0.read().unwrap().len() as i32
    }

    pub fn isEmpty(&self) -> bool {
        self.0.read().unwrap().is_empty()
    }

    pub fn clear(&mut self) {
        *self.0.write().unwrap() = Vec::new();
    }
}

#[allow(non_snake_case)]
impl<T: Clone + PartialEq> JCopyOnWriteArrayList<T> {
    pub fn contains(&self, element: &T) -> bool {
        self.0.read().unwrap().contains(element)
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

impl<T: Clone> Default for JCopyOnWriteArrayList<T> {
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
/// Uses a `HashMap<ThreadId, T>` protected by a `Mutex` to simulate
/// per-thread storage in translated code.  The initializer (if any) is stored
/// as an `Arc<dyn Fn() -> T + Send + Sync>` so that capturing closures
/// (equivalent to Java's `Supplier<T>` lambdas) are fully supported.
pub struct JThreadLocal<T: Clone + Send>(
    Arc<Mutex<HashMap<thread::ThreadId, T>>>,
    Option<Arc<dyn Fn() -> T + Send + Sync>>,
);

impl<T: Clone + Send> Clone for JThreadLocal<T> {
    fn clone(&self) -> Self {
        JThreadLocal(self.0.clone(), self.1.clone())
    }
}

impl<T: Clone + Send> std::fmt::Debug for JThreadLocal<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JThreadLocal")
            .field("has_initializer", &self.1.is_some())
            .finish()
    }
}

#[allow(non_snake_case)]
impl<T: Clone + Send + Default> JThreadLocal<T> {
    pub fn new() -> Self {
        JThreadLocal(Arc::new(Mutex::new(HashMap::new())), None)
    }

    /// Create a `ThreadLocal` with the given initializer closure.
    /// Equivalent to Java's `ThreadLocal.withInitial(Supplier<T>)`.
    pub fn withInitial<F: Fn() -> T + Send + Sync + 'static>(init: F) -> Self {
        JThreadLocal(Arc::new(Mutex::new(HashMap::new())), Some(Arc::new(init)))
    }

    pub fn get(&self) -> T {
        let tid = thread::current().id();
        let map = self.0.lock().unwrap();
        if let Some(val) = map.get(&tid) {
            val.clone()
        } else {
            drop(map);
            let val = if let Some(init) = &self.1 {
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
/// Backed by a `Mutex<WorkQueue> + Condvar` so all workers can wait concurrently
/// without serialising on a single channel receiver lock.
#[derive(Clone)]
pub struct JExecutorService {
    inner: Arc<ExecutorInner>,
}

struct WorkQueue {
    tasks: VecDeque<Box<dyn FnOnce() + Send + 'static>>,
    shutdown: bool,
}

struct ExecutorInner {
    queue: Mutex<WorkQueue>,
    cvar: Condvar,
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
    ///
    /// # Panics
    /// Panics if `n_threads` is not greater than zero.
    pub fn newFixedThreadPool(n_threads: i32) -> Self {
        assert!(
            n_threads > 0,
            "IllegalArgumentException: nThreads must be greater than 0, got {}",
            n_threads
        );
        let n = n_threads as usize;
        let inner = Arc::new(ExecutorInner {
            queue: Mutex::new(WorkQueue {
                tasks: VecDeque::new(),
                shutdown: false,
            }),
            cvar: Condvar::new(),
            workers: Mutex::new(Vec::with_capacity(n)),
            pool_size: n,
        });

        let mut handles = Vec::with_capacity(n);
        for _ in 0..n {
            let inner2 = inner.clone();
            handles.push(thread::spawn(move || loop {
                let task = {
                    let mut queue = inner2.queue.lock().unwrap();
                    loop {
                        if let Some(task) = queue.tasks.pop_front() {
                            break Some(task);
                        }
                        if queue.shutdown {
                            break None;
                        }
                        queue = inner2.cvar.wait(queue).unwrap();
                    }
                };
                match task {
                    Some(f) => f(),
                    None => break,
                }
            }));
        }
        *inner.workers.lock().unwrap() = handles;

        JExecutorService { inner }
    }

    /// Create a single-thread executor (mirrors `Executors.newSingleThreadExecutor()`).
    pub fn newSingleThreadExecutor() -> Self {
        Self::newFixedThreadPool(1)
    }

    /// Create a cached thread pool. For simplicity, maps to a large fixed pool.
    pub fn newCachedThreadPool() -> Self {
        Self::newFixedThreadPool(16)
    }

    /// Enqueue a task, panicking if the executor has already been shut down.
    fn enqueue(&self, task: Box<dyn FnOnce() + Send + 'static>) {
        let mut queue = self.inner.queue.lock().unwrap();
        if queue.shutdown {
            panic!("RejectedExecutionException: executor has been shut down");
        }
        queue.tasks.push_back(task);
        self.inner.cvar.notify_one();
    }

    /// Submit a `Runnable`-like closure. Returns a `JFuture<()>`.
    pub fn submit_runnable<F: FnOnce() + Send + 'static>(&self, task: F) -> JFuture<()> {
        let result = Arc::new((Mutex::new(None::<()>), Condvar::new()));
        let result2 = result.clone();
        self.enqueue(Box::new(move || {
            task();
            let (lock, cvar) = &*result2;
            *lock.lock().unwrap() = Some(());
            cvar.notify_all();
        }));
        JFuture(result)
    }

    /// Submit a callable-like closure that returns a value. Returns a `JFuture<T>`.
    pub fn submit_callable<T: Send + 'static + Clone, F: FnOnce() -> T + Send + 'static>(
        &self,
        task: F,
    ) -> JFuture<T> {
        let result = Arc::new((Mutex::new(None::<T>), Condvar::new()));
        let result2 = result.clone();
        self.enqueue(Box::new(move || {
            let val = task();
            let (lock, cvar) = &*result2;
            *lock.lock().unwrap() = Some(val);
            cvar.notify_all();
        }));
        JFuture(result)
    }

    /// Submit a plain Runnable (no return value, no Future needed for codegen).
    pub fn execute<F: FnOnce() + Send + 'static>(&self, task: F) {
        self.enqueue(Box::new(task));
    }

    /// Initiate orderly shutdown: no new tasks accepted, existing tasks run to completion.
    pub fn shutdown(&self) {
        let mut queue = self.inner.queue.lock().unwrap();
        queue.shutdown = true;
        self.inner.cvar.notify_all();
    }

    /// Block until all worker threads terminate after shutdown.
    ///
    /// This method must be called only after [`shutdown`]. If called before
    /// shutdown it returns `false` immediately. Once shutdown has been initiated
    /// this call blocks until all worker threads have terminated and returns `true`.
    ///
    /// The `timeout_ms` parameter is accepted for API compatibility with Java's
    /// `ExecutorService` interface but is currently ignored; the method blocks
    /// until all workers exit regardless of the timeout value.
    pub fn awaitTermination(&self, _timeout_ms: i64) -> bool {
        if !self.isShutdown() {
            return false;
        }
        let mut workers = self.inner.workers.lock().unwrap();
        while let Some(handle) = workers.pop() {
            handle.join().ok();
        }
        true
    }

    /// Check if the executor has been shut down.
    pub fn isShutdown(&self) -> bool {
        self.inner.queue.lock().unwrap().shutdown
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

// ─── StampedLock ─────────────────────────────────────────────────────────────

struct StampedLockState {
    stamp: u64,
    writers: u32,
    readers: u32,
}

/// Mirrors `java.util.concurrent.locks.StampedLock`.
pub struct JStampedLock {
    state: Arc<Mutex<StampedLockState>>,
    cvar: Arc<Condvar>,
}

impl Clone for JStampedLock {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            cvar: Arc::clone(&self.cvar),
        }
    }
}

impl std::fmt::Debug for JStampedLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JStampedLock")
    }
}

impl Default for JStampedLock {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(non_snake_case)]
impl JStampedLock {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(StampedLockState {
                stamp: 1,
                writers: 0,
                readers: 0,
            })),
            cvar: Arc::new(Condvar::new()),
        }
    }

    /// Acquire the write lock, blocking until no readers or writers hold it.
    /// Returns an opaque stamp that must be passed to `unlockWrite`.
    pub fn writeLock(&self) -> i64 {
        let mut st = self.state.lock().unwrap();
        while st.writers > 0 || st.readers > 0 {
            st = self.cvar.wait(st).unwrap();
        }
        st.writers = 1;
        st.stamp += 1;
        st.stamp as i64
    }

    /// Release the write lock obtained via `writeLock`.
    pub fn unlockWrite(&self, _stamp: i64) {
        let mut st = self.state.lock().unwrap();
        st.writers = 0;
        self.cvar.notify_all();
    }

    /// Acquire the read lock, blocking until no writer holds it.
    /// Returns an opaque stamp.
    pub fn readLock(&self) -> i64 {
        let mut st = self.state.lock().unwrap();
        while st.writers > 0 {
            st = self.cvar.wait(st).unwrap();
        }
        st.readers += 1;
        st.stamp as i64
    }

    /// Release the read lock obtained via `readLock`.
    pub fn unlockRead(&self, _stamp: i64) {
        let mut st = self.state.lock().unwrap();
        assert!(st.readers > 0, "unlockRead called with no active read lock");
        st.readers -= 1;
        if st.readers == 0 {
            self.cvar.notify_all();
        }
    }

    /// Try to obtain an optimistic read stamp.  Returns `0` if a write lock
    /// is currently held (indicating that an optimistic read is not safe).
    pub fn tryOptimisticRead(&self) -> i64 {
        let st = self.state.lock().unwrap();
        if st.writers > 0 {
            0
        } else {
            st.stamp as i64
        }
    }

    /// Validate that the given optimistic-read stamp is still current (no
    /// write has occurred since the stamp was obtained).
    pub fn validate(&self, stamp: i64) -> bool {
        let st = self.state.lock().unwrap();
        st.writers == 0 && st.stamp as i64 == stamp
    }
}

// ─── ForkJoinPool / RecursiveTask ────────────────────────────────────────────

/// Mirrors `java.util.concurrent.ForkJoinPool`.
///
/// For oxidize's purposes, `invoke(task)` is rewritten by codegen to call
/// `task.compute()` directly.  The pool itself is a zero-size token.
#[derive(Clone, Debug, Default)]
pub struct JForkJoinPool;

#[allow(non_snake_case)]
impl JForkJoinPool {
    pub fn new() -> Self {
        JForkJoinPool
    }

    pub fn commonPool() -> Self {
        JForkJoinPool
    }
}

/// A lightweight handle that carries the result of a `fork()`ed RecursiveTask.
///
/// Injected into RecursiveTask subclasses by codegen as `__fork_handle`.
#[derive(Clone, Debug)]
pub struct JForkJoinHandle<T: Clone + Send + 'static>(Arc<(Mutex<Option<T>>, Condvar)>);

impl<T: Clone + Send + 'static> Default for JForkJoinHandle<T> {
    fn default() -> Self {
        JForkJoinHandle(Arc::new((Mutex::new(None), Condvar::new())))
    }
}

impl<T: Clone + Send + 'static> JForkJoinHandle<T> {
    /// Create a new, empty handle.
    pub fn __new() -> Self {
        JForkJoinHandle(Arc::new((Mutex::new(None), Condvar::new())))
    }

    /// Store the result and wake any waiting `__get` caller.
    pub fn __set(&self, val: T) {
        let (lock, cvar) = &*self.0;
        *lock.lock().unwrap() = Some(val);
        cvar.notify_all();
    }

    /// Block until the result is available, then return it.
    pub fn __get(&self) -> T {
        let (lock, cvar) = &*self.0;
        let mut guard = lock.lock().unwrap();
        while guard.is_none() {
            guard = cvar.wait(guard).unwrap();
        }
        guard.as_ref().unwrap().clone()
    }
}

// ─── Per-object monitor ──────────────────────────────────────────────────────

/// Provides the monitor semantics (`wait` / `notify` / `notifyAll`) for
/// arbitrary Java `synchronized(obj)` blocks.
///
/// Injected by codegen as `pub __monitor: JMonitor` into every user class so
/// that `synchronized(obj)` can lock the *object's own* mutex rather than the
/// process-global fallback.
pub struct JMonitor(Arc<(Mutex<()>, Condvar)>);

impl JMonitor {
    pub fn new() -> Self {
        JMonitor(Arc::new((Mutex::new(()), Condvar::new())))
    }

    /// Return a cloned `Arc` to the underlying `(Mutex, Condvar)` pair so that
    /// codegen-emitted synchronized blocks can bind `__sync_lock`/`__sync_cond`.
    pub fn pair(&self) -> Arc<(Mutex<()>, Condvar)> {
        Arc::clone(&self.0)
    }
}

impl Default for JMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for JMonitor {
    fn clone(&self) -> Self {
        JMonitor(Arc::clone(&self.0))
    }
}

impl std::fmt::Debug for JMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JMonitor")
    }
}
