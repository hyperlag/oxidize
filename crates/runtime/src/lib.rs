//! `java-compat` — runtime support types that mirror Java semantics in Rust.
//!
//! This crate provides the Rust types that translated Java programs depend on
//! at runtime. All shared-mutable state uses `Arc<RwLock<T>>` to preserve Java
//! heap semantics without `unsafe`.

pub mod array;
pub mod atomic;
pub mod concurrent;
pub mod exception;
pub mod list;
pub mod map;
pub mod object;
pub mod set;
pub mod string;
pub mod thread;

pub use array::JArray;
pub use atomic::{JAtomicBoolean, JAtomicInteger, JAtomicLong};
pub use concurrent::{JCountDownLatch, JSemaphore, __sync_block_monitor};
pub use exception::JException;
pub use list::JList;
pub use map::JMap;
pub use object::{JNull, JObject};
pub use set::JSet;
pub use string::JString;
pub use thread::JThread;

/// Convenience re-export of all runtime types.
pub mod prelude {
    pub use super::{
        JArray, JAtomicBoolean, JAtomicInteger, JAtomicLong, JCountDownLatch, JException, JList,
        JMap, JNull, JObject, JSemaphore, JSet, JString, JThread,
    };
}
