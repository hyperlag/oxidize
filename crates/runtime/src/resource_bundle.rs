#![allow(non_snake_case)]
//! [`JResourceBundle`] — Rust equivalent of `java.util.ResourceBundle` /
//! `java.util.PropertyResourceBundle`.
//!
//! Stub implementation backed by a `HashMap<String, String>`.
//! Supports loading from `.properties`-formatted files on the filesystem
//! (via `getBundle`) and from in-memory byte streams (via `from_input_stream`).

use crate::io::JByteArrayInputStream;
use crate::string::JString;
use std::collections::HashMap;

/// Rust equivalent of `java.util.ResourceBundle` / `PropertyResourceBundle`.
#[derive(Debug, Clone, Default)]
pub struct JResourceBundle {
    map: HashMap<String, String>,
}

impl JResourceBundle {
    /// `ResourceBundle.getBundle(baseName)` — loads `<baseName>.properties`
    /// from the current working directory.
    pub fn get_bundle(base_name: JString) -> Self {
        let path = format!("{}.properties", base_name.as_str());
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        Self {
            map: parse_properties(&content),
        }
    }

    /// `new PropertyResourceBundle(inputStream)` — load from a byte stream
    /// that contains `.properties`-formatted content.
    pub fn from_input_stream(mut stream: JByteArrayInputStream) -> Self {
        let bytes = stream.read_all_bytes();
        let content = String::from_utf8_lossy(&bytes).into_owned();
        Self {
            map: parse_properties(&content),
        }
    }

    /// `bundle.getString(key)` — returns the value for `key`, panics if absent.
    pub fn getString(&self, key: JString) -> JString {
        let k = key.as_str();
        match self.map.get(k) {
            Some(v) => JString::from(v.as_str()),
            None => panic!("JException:MissingResourceException:key '{k}' not found"),
        }
    }

    /// `bundle.getObject(key)` — returns the value as a `JString`.
    pub fn getObject(&self, key: JString) -> JString {
        self.getString(key)
    }

    /// `bundle.containsKey(key)`
    pub fn containsKey(&self, key: JString) -> bool {
        self.map.contains_key(key.as_str())
    }

    /// `bundle.keySet()` — returns all keys as a `JString` list.
    pub fn keySet(&self) -> crate::list::JList<JString> {
        let mut list = crate::list::JList::new();
        for k in self.map.keys() {
            list.add(JString::from(k.as_str()));
        }
        list
    }

    /// Raw map access for internal use.
    pub fn get_map(&self) -> &HashMap<String, String> {
        &self.map
    }
}

/// Parse `.properties` file content into a `HashMap`.
///
/// Supported line formats (after trimming):
/// - `key=value`
/// - `key: value`
/// - `# comment` / `! comment` — ignored
/// - blank lines — ignored
fn parse_properties(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }
        if let Some(idx) = trimmed.find('=') {
            let key = trimmed[..idx].trim().to_owned();
            let value = trimmed[idx + 1..].trim().to_owned();
            map.insert(key, value);
        } else if let Some(idx) = trimmed.find(": ") {
            let key = trimmed[..idx].trim().to_owned();
            let value = trimmed[idx + 2..].trim().to_owned();
            map.insert(key, value);
        }
    }
    map
}
