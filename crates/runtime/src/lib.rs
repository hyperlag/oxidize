//! `java-compat` — runtime support types that mirror Java semantics in Rust.
//!
//! This crate provides the Rust types that translated Java programs depend on
//! at runtime. All shared-mutable state uses `Arc<RwLock<T>>` to preserve Java
//! heap semantics without `unsafe`.

pub mod array;
pub mod atomic;
pub mod bigdecimal;
pub mod bigint;
pub mod collections_util;
pub mod concurrent;
pub mod enum_map;
pub mod enum_set;
pub mod exception;
pub mod io;
pub mod iterator;
pub mod linked_hash_map;
pub mod linked_hash_set;
pub mod linked_list;
pub mod list;
pub mod map;
pub mod net;
pub mod object;
pub mod optional;
pub mod priority_queue;
pub mod process;
pub mod properties;
pub mod reflect;
pub mod regex_support;
pub mod resource_bundle;
pub mod set;
pub mod spliterator;
pub mod stream;
pub mod string;
pub mod string_builder;
pub mod thread;
pub mod time;
pub mod timer;
pub mod tree_map;
pub mod tree_set;

pub use array::JArray;
pub use atomic::{JAtomicBoolean, JAtomicInteger, JAtomicLong};
pub use bigdecimal::{JBigDecimal, JMathContext, JRoundingMode};
pub use bigint::JBigInteger;
pub use concurrent::{
    __sync_block_monitor, JCompletableFuture, JConcurrentHashMap, JCondition,
    JCopyOnWriteArrayList, JCountDownLatch, JExecutorService, JExecutors, JForkJoinHandle,
    JForkJoinPool, JFuture, JMonitor, JReadLock, JReentrantLock, JReentrantReadWriteLock,
    JSemaphore, JStampedLock, JThreadLocal, JTimeUnit, JWriteLock,
};
pub use enum_map::JEnumMap;
pub use enum_set::JEnumSet;
pub use exception::JException;
pub use io::{
    JBufferedReader, JBufferedWriter, JByteArrayInputStream, JByteArrayOutputStream, JFile,
    JFileInputStream, JFileOutputStream, JFileReader, JFileWriter, JFiles, JInputStream,
    JOutputStream, JPath, JPaths, JPrintWriter, JReader, JScanner, JStringReader, JStringWriter,
    JWriter,
};
pub use iterator::JIterator;
pub use linked_hash_map::JLinkedHashMap;
pub use linked_hash_set::JLinkedHashSet;
pub use linked_list::JLinkedList;
pub use list::JList;
pub use map::{JMap, JMapEntry};
pub use net::{
    JHttpClient, JHttpRequest, JHttpRequestBuilder, JHttpResponse, JHttpURLConnection,
    JServerSocket, JSocket, JURL,
};
pub use object::{JNull, JObject, JavaObject};
pub use optional::JOptional;
pub use priority_queue::JPriorityQueue;
pub use process::{JProcess, JProcessBuilder};
pub use properties::JProperties;
pub use reflect::JClass;
pub use regex_support::{JMatcher, JPattern};
pub use resource_bundle::JResourceBundle;
pub use set::JSet;
pub use spliterator::JSpliterator;
pub use stream::JStream;
pub use string::jformat;
pub use string::JString;
pub use string_builder::JStringBuilder;
pub use thread::JThread;
pub use time::JLocalDate;
pub use time::{
    JClock, JDateTimeFormatter, JDuration, JInstant, JLocalDateTime, JLocalTime, JPeriod, JZoneId,
    JZonedDateTime,
};
pub use timer::{JTimer, JTimerTask};
pub use tree_map::JTreeMap;
pub use tree_set::JTreeSet;

/// Compare two items by a key function. Used by generated Comparator.comparing(keyFn) code.
/// The key function receives a reference to each element; Rust can infer `T` from the
/// references `x` and `y`, which fixes the type-inference problem for inline closures.
#[inline]
pub fn compare_by_key<T, K: Ord>(x: &T, y: &T, key: impl Fn(&T) -> K) -> i32 {
    let kx = key(x);
    let ky = key(y);
    kx.cmp(&ky) as i32
}

/// Reverse a comparator result. Used by generated Comparator.reversed() code.
/// Rust infers `T` from `x: &T`, which then constrains the `cmp` closure's parameter types.
#[inline]
pub fn compare_reversed<T>(x: &T, y: &T, cmp: impl Fn(&T, &T) -> i32) -> i32 {
    cmp(y, x)
}

/// Compare two items by an f64 key function.
/// Used by generated Comparator.comparingDouble(keyFn) code.
/// Uses `partial_cmp` to handle f64 (which does not implement `Ord`).
/// NaN is treated as less than any non-NaN value, matching Java's Double.compare semantics.
#[inline]
pub fn compare_by_key_f64<T>(x: &T, y: &T, key: impl Fn(&T) -> f64) -> i32 {
    let kx = key(x);
    let ky = key(y);
    match kx.partial_cmp(&ky).unwrap_or(std::cmp::Ordering::Less) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

/// Compose two comparators: primary first, secondary key on tie.
/// Used by generated Comparator.thenComparing(keyFn) code.
#[inline]
pub fn compare_then<T, K: Ord>(
    x: &T,
    y: &T,
    cmp: impl Fn(&T, &T) -> i32,
    key: impl Fn(&T) -> K,
) -> i32 {
    let r = cmp(x, y);
    if r != 0 {
        r
    } else {
        compare_by_key(x, y, key)
    }
}

/// Compose two comparators: primary first, secondary comparator on tie.
/// Used by generated Comparator.thenComparing(Comparator) code.
#[inline]
pub fn compare_then_cmp<T>(
    x: &T,
    y: &T,
    primary: impl Fn(&T, &T) -> i32,
    secondary: impl Fn(&T, &T) -> i32,
) -> i32 {
    let r = primary(x, y);
    if r != 0 {
        r
    } else {
        secondary(x, y)
    }
}

/// Convenience re-export of all runtime types.
pub mod prelude {
    pub use super::{
        JArray, JAtomicBoolean, JAtomicInteger, JAtomicLong, JBigDecimal, JBigInteger,
        JBufferedReader, JBufferedWriter, JClass, JClock, JCompletableFuture, JConcurrentHashMap,
        JCondition, JCopyOnWriteArrayList, JCountDownLatch, JDateTimeFormatter, JDuration,
        JEnumMap, JEnumSet, JException, JExecutorService, JExecutors, JFile, JFileInputStream,
        JFileOutputStream, JFileReader, JFileWriter, JFiles, JFuture, JHttpClient, JHttpRequest,
        JHttpRequestBuilder, JHttpResponse, JHttpURLConnection, JInstant, JIterator,
        JLinkedHashMap, JLinkedHashSet, JLinkedList, JList, JLocalDate, JLocalDateTime, JLocalTime,
        JMap, JMapEntry, JMatcher, JMathContext, JNull, JObject, JOptional, JPath, JPaths,
        JPattern, JPeriod, JPrintWriter, JPriorityQueue, JProcess, JProcessBuilder, JProperties,
        JReadLock, JReentrantLock, JReentrantReadWriteLock, JRoundingMode, JScanner, JSemaphore,
        JServerSocket, JSet, JSocket, JSpliterator, JStream, JString, JStringBuilder, JThread,
        JThreadLocal, JTimeUnit, JTimer, JTimerTask, JTreeMap, JTreeSet, JWriteLock, JZoneId,
        JZonedDateTime, JURL,
    };
}
