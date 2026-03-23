//! `java-compat` — runtime support types that mirror Java semantics in Rust.
//!
//! This crate provides the Rust types that translated Java programs depend on
//! at runtime. All shared-mutable state uses `Arc<RwLock<T>>` to preserve Java
//! heap semantics without `unsafe`.

pub mod array;
pub mod list;
pub mod map;
pub mod object;
pub mod set;
pub mod string;

pub use array::JArray;
pub use list::JList;
pub use map::JMap;
pub use object::{JNull, JObject};
pub use set::JSet;
pub use string::JString;

/// Convenience re-export of all runtime types.
pub mod prelude {
    pub use super::{JArray, JList, JMap, JNull, JObject, JSet, JString};
}
