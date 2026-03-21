//! `java-compat` — runtime support types that mirror Java semantics in Rust.
//!
//! This crate provides the Rust types that translated Java programs depend on
//! at runtime. All shared-mutable state uses `Arc<RwLock<T>>` to preserve Java
//! heap semantics without `unsafe`.

pub mod array;
pub mod object;
pub mod string;

pub use array::JArray;
pub use object::{JNull, JObject};
pub use string::JString;

/// Convenience re-export of all runtime types.
pub mod prelude {
    pub use super::{JArray, JNull, JObject, JString};
}
