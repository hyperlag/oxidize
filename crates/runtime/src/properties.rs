#![allow(non_snake_case)]
//! [`JProperties`] — Rust equivalent of `java.util.Properties`.
//!
//! Backed by a plain `HashMap<String, String>`.  Supports the subset of
//! `Properties` methods needed by transpiled code:
//! `new`, `load` (from string content), `store`, `getProperty`,
//! `setProperty`, `stringPropertyNames`, `containsKey`, `size`, `isEmpty`.

use crate::set::JSet;
use crate::string::JString;
use std::collections::HashMap;

/// Rust equivalent of `java.util.Properties`.
#[derive(Debug, Clone, Default)]
pub struct JProperties {
    map: HashMap<String, String>,
}

impl JProperties {
    /// `new Properties()`
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Load properties from a string (equivalent to `load(new StringReader(s))`).
    ///
    /// Supported formats per line:
    /// - `key=value`
    /// - `key: value`
    /// - `# comment` / `! comment` / blank — ignored
    pub fn load_string(&mut self, content: &JString) {
        for raw_line in content.as_str().lines() {
            // Handle line continuation (backslash at end)
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
                continue;
            }
            // Find the key-value separator: first unescaped '=' or ':'
            if let Some(sep_pos) = line.find('=').or_else(|| line.find(':')) {
                let key = line[..sep_pos].trim().to_owned();
                let val = line[sep_pos + 1..].trim().to_owned();
                if !key.is_empty() {
                    self.map.insert(key, val);
                }
            }
        }
    }

    /// `getProperty(String key)` — returns empty string if not found.
    pub fn getProperty(&self, key: &JString) -> JString {
        self.map
            .get(key.as_str())
            .map(|v| JString::from(v.as_str()))
            .unwrap_or_default()
    }

    /// `getProperty(String key, String defaultValue)`.
    pub fn getProperty_default(&self, key: &JString, default: &JString) -> JString {
        self.map
            .get(key.as_str())
            .map(|v| JString::from(v.as_str()))
            .unwrap_or_else(|| default.clone())
    }

    /// `setProperty(String key, String value)`.
    pub fn setProperty(&mut self, key: &JString, value: &JString) {
        self.map
            .insert(key.as_str().to_owned(), value.as_str().to_owned());
    }

    /// `stringPropertyNames()` — returns the set of all keys.
    pub fn stringPropertyNames(&self) -> JSet<JString> {
        let mut set = JSet::new();
        for k in self.map.keys() {
            set.add(JString::from(k.as_str()));
        }
        set
    }

    /// `containsKey(String key)`.
    pub fn containsKey(&self, key: &JString) -> bool {
        self.map.contains_key(key.as_str())
    }

    /// `size()` — number of entries.
    pub fn size(&self) -> i32 {
        self.map.len() as i32
    }

    /// `isEmpty()`.
    pub fn isEmpty(&self) -> bool {
        self.map.is_empty()
    }

    /// `store(Writer, String comments)` — returns the serialised content as a
    /// string (used when `store` is called with a `StringWriter`).
    pub fn store_string(&self, comments: &JString) -> JString {
        let mut out = String::new();
        if !comments.as_str().is_empty() {
            out.push_str(&format!("# {}\n", comments.as_str()));
        }
        // Emit keys in sorted order for deterministic output
        let mut pairs: Vec<_> = self.map.iter().collect();
        pairs.sort_by_key(|(k, _)| k.as_str());
        for (k, v) in pairs {
            out.push_str(&format!("{}={}\n", k, v));
        }
        JString::from(out.as_str())
    }

    /// Remove a key; returns the previous value or empty string.
    pub fn remove_key(&mut self, key: &JString) -> JString {
        self.map
            .remove(key.as_str())
            .map(|v| JString::from(v.as_str()))
            .unwrap_or_default()
    }
}

impl std::fmt::Display for JProperties {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.store_string(&JString::from("")))
    }
}
