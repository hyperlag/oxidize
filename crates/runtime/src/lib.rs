//! `java-compat` — runtime support types that mirror Java semantics in Rust.
//!
//! This crate provides the Rust types that translated Java programs depend on
//! at runtime. All shared-mutable state uses `Arc<RwLock<T>>` to preserve Java
//! heap semantics without `unsafe`.

pub mod array;
pub mod atomic;
pub mod bigint;
pub mod concurrent;
pub mod exception;
pub mod io;
pub mod list;
pub mod map;
pub mod object;
pub mod optional;
pub mod reflect;
pub mod regex_support;
pub mod set;
pub mod stream;
pub mod string;
pub mod string_builder;
pub mod thread;
pub mod time;

pub use array::JArray;
pub use atomic::{JAtomicBoolean, JAtomicInteger, JAtomicLong};
pub use bigint::JBigInteger;
pub use concurrent::{JCountDownLatch, JSemaphore, __sync_block_monitor};
pub use exception::JException;
pub use io::JFile;
pub use list::JList;
pub use map::JMap;
pub use object::{JNull, JObject};
pub use optional::JOptional;
pub use reflect::JClass;
pub use regex_support::{JMatcher, JPattern};
pub use set::JSet;
pub use stream::JStream;
pub use string::JString;
pub use string_builder::JStringBuilder;
pub use thread::JThread;
pub use time::JLocalDate;

/// Convenience re-export of all runtime types.
pub mod prelude {
    pub use super::{
        JArray, JAtomicBoolean, JAtomicInteger, JAtomicLong, JBigInteger, JClass, JCountDownLatch,
        JException, JFile, JList, JLocalDate, JMap, JMatcher, JNull, JObject, JOptional, JPattern,
        JSemaphore, JSet, JStream, JString, JStringBuilder, JThread,
    };
}
