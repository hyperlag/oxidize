#![allow(non_snake_case)]
//! [`JStringBuilder`] — Rust representation of `java.lang.StringBuilder`.
//!
//! Mapping: `java.lang.StringBuilder` → `JStringBuilder` (wraps `String`).

use crate::string::JString;
use std::fmt;

/// A mutable character sequence — Java's `StringBuilder`.
#[derive(Debug, Clone, Default)]
pub struct JStringBuilder {
    buf: String,
}

impl JStringBuilder {
    /// Create an empty `StringBuilder`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a `StringBuilder` initialised with `s`.
    pub fn new_from_string(s: JString) -> Self {
        Self {
            buf: s.as_str().to_owned(),
        }
    }

    /// Java `sb.append(val)` — accepts any type implementing `Display`.
    pub fn append<T: fmt::Display>(&mut self, val: T) -> &mut Self {
        self.buf.push_str(&val.to_string());
        self
    }

    /// Java `sb.toString()`.
    pub fn toString(&self) -> JString {
        JString::from(self.buf.as_str())
    }

    /// Java `sb.length()`.
    pub fn length(&self) -> i32 {
        self.buf.chars().count() as i32
    }

    /// Java `sb.charAt(i)`.
    pub fn charAt(&self, i: i32) -> char {
        if i < 0 {
            panic!("JStringBuilder::charAt index out of bounds: {}", i);
        }
        let target = i as usize;
        let len = self.buf.chars().count();
        if target >= len {
            panic!("JStringBuilder::charAt index out of bounds: {}", i);
        }
        self.buf.chars().nth(target).unwrap()
    }

    /// Alias used by codegen's `charAt` → `char_at` rename.
    pub fn char_at(&self, i: i32) -> char {
        self.charAt(i)
    }

    /// Java `sb.reverse()`.
    pub fn reverse(&mut self) -> &mut Self {
        let r: String = self.buf.chars().rev().collect();
        self.buf = r;
        self
    }

    /// Java `sb.insert(offset, s)`.
    pub fn insert(&mut self, offset: i32, s: JString) -> &mut Self {
        let len_chars = self.buf.chars().count() as i32;
        if offset < 0 || offset > len_chars {
            panic!("StringIndexOutOfBoundsException: offset {}", offset);
        }
        let byte_idx = if offset == len_chars {
            self.buf.len()
        } else {
            self.buf
                .char_indices()
                .nth(offset as usize)
                .map(|(i, _)| i)
                .expect("valid character index after bounds check")
        };
        self.buf.insert_str(byte_idx, s.as_str());
        self
    }

    /// Java `sb.delete(start, end)` (exclusive end).
    pub fn delete(&mut self, start: i32, end: i32) -> &mut Self {
        let chars: String = self
            .buf
            .chars()
            .enumerate()
            .filter(|(i, _)| *i < start as usize || *i >= end as usize)
            .map(|(_, c)| c)
            .collect();
        self.buf = chars;
        self
    }

    /// Java `sb.deleteCharAt(i)`.
    pub fn deleteCharAt(&mut self, i: i32) -> &mut Self {
        if let Some((byte_idx, _)) = self.buf.char_indices().nth(i as usize) {
            self.buf.remove(byte_idx);
        }
        self
    }

    /// Java `sb.indexOf(s)`.
    pub fn indexOf(&self, s: JString) -> i32 {
        // Character-index version (Java indexOf returns char offset)
        self.buf
            .find(s.as_str())
            .map(|b| self.buf[..b].chars().count() as i32)
            .unwrap_or(-1)
    }

    /// Alias used by codegen's `indexOf` → `index_of` rename.
    pub fn index_of(&self, s: JString) -> i32 {
        self.indexOf(s)
    }

    /// Java `sb.lastIndexOf(s)`.
    pub fn lastIndexOf(&self, s: JString) -> i32 {
        let needle = s.as_str();
        self.buf
            .rmatch_indices(needle)
            .next()
            .map(|(b, _)| self.buf[..b].chars().count() as i32)
            .unwrap_or(-1)
    }

    /// Java `sb.replace(start, end, str)` — replaces chars [start, end) with str.
    pub fn replace(&mut self, start: i32, end: i32, s: JString) -> &mut Self {
        let start = start as usize;
        let end = end as usize;
        let chars: Vec<char> = self.buf.chars().collect();
        let end = end.min(chars.len());
        let before: String = chars[..start].iter().collect();
        let after: String = chars[end..].iter().collect();
        self.buf = format!("{}{}{}", before, s.as_str(), after);
        self
    }

    /// Java `sb.setCharAt(i, c)`.
    pub fn setCharAt(&mut self, i: i32, c: char) {
        let idx = i as usize;
        let chars: String = self
            .buf
            .chars()
            .enumerate()
            .map(|(j, ch)| if j == idx { c } else { ch })
            .collect();
        self.buf = chars;
    }

    /// Java `sb.substring(start)`.
    pub fn substring(&self, start: i32) -> JString {
        let s: String = self.buf.chars().skip(start as usize).collect();
        JString::from(s.as_str())
    }

    /// Java `sb.substring(start, end)`.
    pub fn substringRange(&self, start: i32, end: i32) -> JString {
        let s: String = self
            .buf
            .chars()
            .skip(start as usize)
            .take((end - start) as usize)
            .collect();
        JString::from(s.as_str())
    }
}

impl fmt::Display for JStringBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.buf)
    }
}
