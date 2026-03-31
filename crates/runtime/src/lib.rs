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
pub mod reflect;
pub mod regex_support;
pub mod set;
pub mod stream;
pub mod string;
pub mod string_builder;
pub mod thread;
pub mod time;
pub mod tree_map;
pub mod tree_set;

pub use array::JArray;
pub use atomic::{JAtomicBoolean, JAtomicInteger, JAtomicLong};
pub use bigdecimal::{JBigDecimal, JMathContext, JRoundingMode};
pub use bigint::JBigInteger;
pub use concurrent::{JCountDownLatch, JSemaphore, __sync_block_monitor};
pub use enum_map::JEnumMap;
pub use enum_set::JEnumSet;
pub use exception::JException;
pub use io::{
    JBufferedReader, JBufferedWriter, JFile, JFileInputStream, JFileOutputStream, JFileReader,
    JFileWriter, JFiles, JPath, JPaths, JPrintWriter, JScanner,
};
pub use iterator::JIterator;
pub use linked_hash_map::JLinkedHashMap;
pub use linked_hash_set::JLinkedHashSet;
pub use linked_list::JLinkedList;
pub use list::JList;
pub use map::JMap;
pub use net::{JHttpURLConnection, JServerSocket, JSocket, JURL};
pub use object::{JNull, JObject, JavaObject};
pub use optional::JOptional;
pub use priority_queue::JPriorityQueue;
pub use reflect::JClass;
pub use regex_support::{JMatcher, JPattern};
pub use set::JSet;
pub use stream::JStream;
pub use string::JString;
pub use string::jformat;
pub use string_builder::JStringBuilder;
pub use thread::JThread;
pub use time::JLocalDate;
pub use time::{JLocalTime, JLocalDateTime, JInstant, JDuration, JPeriod, JDateTimeFormatter};
pub use tree_map::JTreeMap;
pub use tree_set::JTreeSet;

/// Convenience re-export of all runtime types.
pub mod prelude {
    pub use super::{
        JArray, JAtomicBoolean, JAtomicInteger, JAtomicLong, JBigDecimal, JBigInteger,
        JBufferedReader, JBufferedWriter, JClass, JCountDownLatch, JEnumMap, JEnumSet, JException,
        JFile, JFileInputStream, JFileOutputStream, JFileReader, JFileWriter, JFiles,
        JHttpURLConnection, JIterator, JLinkedHashMap, JLinkedHashSet, JLinkedList, JList,
        JLocalDate, JLocalTime, JLocalDateTime, JInstant, JDuration, JPeriod, JDateTimeFormatter,
        JMap, JMatcher, JMathContext, JNull, JObject, JOptional, JPath, JPaths,
        JPattern, JPrintWriter, JPriorityQueue, JRoundingMode, JScanner, JSemaphore, JServerSocket,
        JSet, JSocket, JStream, JString, JStringBuilder, JThread, JTreeMap, JTreeSet, JURL,
    };
}
