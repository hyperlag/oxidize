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
    JCompletableFuture, JConcurrentHashMap, JCondition, JCopyOnWriteArrayList, JCountDownLatch,
    JExecutorService, JExecutors, JForkJoinHandle, JForkJoinPool, JFuture, JMonitor, JReadLock,
    JReentrantLock, JReentrantReadWriteLock, JSemaphore, JStampedLock, JThreadLocal, JTimeUnit,
    JWriteLock, __sync_block_monitor,
};
pub use enum_map::JEnumMap;
pub use enum_set::JEnumSet;
pub use exception::JException;
pub use io::{
    JBufferedReader, JBufferedWriter, JByteArrayInputStream, JByteArrayOutputStream, JFile,
    JFileInputStream, JFileOutputStream, JFileReader, JFileWriter, JFiles, JPath, JPaths,
    JPrintWriter, JScanner, JStringReader, JStringWriter,
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
