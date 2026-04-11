//! [`JString`] — Rust representation of `java.lang.String`.
//!
//! Java strings are immutable and reference-counted; we model them with
//! `Arc<str>`. The `+` operator is overloaded via the `Add` trait to preserve
//! Java's string-concatenation syntax.

use std::fmt;
use std::ops::Add;
use std::sync::Arc;

/// An immutable, reference-counted Java string.
///
/// Mapping: `java.lang.String` → `JString` (wraps `Arc<str>`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JString(Arc<str>);

impl JString {
    /// Create a `JString` from any `&str`.
    pub fn new(s: &str) -> Self {
        JString(Arc::from(s))
    }

    /// Return the underlying string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Java `String.length()`.
    pub fn length(&self) -> i32 {
        // Java counts UTF-16 code units; this counts chars (good enough for
        // Stage 0 — full surrogate-pair handling is deferred).
        self.0.chars().count() as i32
    }

    /// Java `String.isEmpty()`.
    #[allow(non_snake_case)]
    pub fn isEmpty(&self) -> bool {
        self.0.is_empty()
    }

    /// Java `String.isEmpty()` — snake_case alias.
    pub fn is_empty_java(&self) -> bool {
        self.0.is_empty()
    }

    /// Java `String.charAt(int index)`.
    pub fn char_at(&self, index: i32) -> char {
        self.0
            .chars()
            .nth(index as usize)
            .expect("StringIndexOutOfBoundsException")
    }

    /// Java `String.substring(int beginIndex)`.
    pub fn substring(&self, begin: i32) -> JString {
        let s: String = self.0.chars().skip(begin as usize).collect();
        JString(Arc::from(s.as_str()))
    }

    /// Java `String.substring(int beginIndex, int endIndex)`.
    ///
    /// Indices are Unicode scalar values (not UTF-16 code units). Panics with
    /// `StringIndexOutOfBoundsException` if `begin < 0`, `end < begin`, or
    /// `end` exceeds the character count.
    pub fn substring_range(&self, begin: i32, end: i32) -> JString {
        assert!(
            begin >= 0 && end >= begin,
            "StringIndexOutOfBoundsException: begin={begin}, end={end}"
        );
        let len = (end - begin) as usize;
        let s: String = self.0.chars().skip(begin as usize).take(len).collect();
        assert_eq!(
            s.chars().count(),
            len,
            "StringIndexOutOfBoundsException: end index {end} exceeds string length"
        );
        JString(Arc::from(s.as_str()))
    }

    /// Java `String.trim()`.
    ///
    /// Removes only characters with code point `<= '\u0020'`, matching
    /// Java's `String.trim()` semantics (not Rust's Unicode-aware `trim()`).
    pub fn trim(&self) -> JString {
        JString::from(self.0.trim_matches(|c: char| c <= ' '))
    }

    /// Java `String.getBytes()` — returns the UTF-8 byte representation as a
    /// `JArray<i8>`. Any charset argument in the Java source is ignored by
    /// codegen (UTF-8 is always used).
    #[allow(non_snake_case)]
    pub fn getBytes(&self) -> crate::array::JArray<i8> {
        crate::array::JArray::from_vec(self.0.as_bytes().iter().map(|&b| b as i8).collect())
    }

    /// Java `String.contains(CharSequence s)`.
    pub fn contains_str(&self, s: &str) -> bool {
        self.0.contains(s)
    }

    /// Java `String.startsWith(String prefix)`.
    #[allow(non_snake_case)]
    pub fn startsWith(&self, prefix: JString) -> bool {
        self.0.starts_with(prefix.as_str())
    }

    /// Java `String.endsWith(String suffix)`.
    #[allow(non_snake_case)]
    pub fn endsWith(&self, suffix: JString) -> bool {
        self.0.ends_with(suffix.as_str())
    }

    /// Java `String.equals(Object o)`.
    pub fn equals(&self, other: &JString) -> bool {
        *self.0 == *other.0
    }

    // ── Java 11+ String methods ───────────────────────────────────────────

    /// Java `String.strip()` — Unicode-aware whitespace trimming.
    pub fn strip(&self) -> JString {
        JString::from(self.0.trim())
    }

    /// Java `String.stripLeading()`.
    #[allow(non_snake_case)]
    pub fn stripLeading(&self) -> JString {
        JString::from(self.0.trim_start())
    }

    /// Java `String.stripTrailing()`.
    #[allow(non_snake_case)]
    pub fn stripTrailing(&self) -> JString {
        JString::from(self.0.trim_end())
    }

    /// Java `String.isBlank()` — true if empty or contains only whitespace.
    #[allow(non_snake_case)]
    pub fn isBlank(&self) -> bool {
        self.0.chars().all(|c| c.is_whitespace())
    }

    /// Java `String.repeat(int count)`.
    pub fn repeat(&self, n: i32) -> JString {
        if n < 0 {
            panic!("count is negative: {n}");
        }
        JString::from(self.0.repeat(n as usize).as_str())
    }

    /// Java `String.lines()` — returns a stream of lines.
    ///
    /// Named `lines_stream` internally; codegen maps `lines` → `lines_stream`.
    pub fn lines_stream(&self) -> crate::stream::JStream<JString> {
        crate::stream::JStream::new(self.0.lines().map(JString::from).collect())
    }

    /// Java `String.chars()` — returns a stream of chars.
    ///
    /// Named `chars_stream` internally; codegen maps `chars` → `chars_stream`.
    pub fn chars_stream(&self) -> crate::stream::JStream<char> {
        crate::stream::JStream::new(self.0.chars().collect())
    }

    /// Java `String.toCharArray()`.
    pub fn to_char_array(&self) -> crate::array::JArray<char> {
        crate::array::JArray::from_vec(self.0.chars().collect())
    }

    /// Java `String.compareTo(other)` — returns negative, zero, or positive.
    #[allow(non_snake_case)]
    pub fn compareTo(&self, other: JString) -> i32 {
        match self.cmp(&other) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    }
}

impl From<&str> for JString {
    fn from(s: &str) -> Self {
        JString::new(s)
    }
}

impl From<String> for JString {
    fn from(s: String) -> Self {
        JString(Arc::from(s.as_str()))
    }
}

impl Default for JString {
    /// Returns an empty string — the natural zero-value for `java.lang.String`.
    fn default() -> Self {
        JString::from("")
    }
}

impl PartialOrd for JString {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for JString {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Match Java's String.compareTo semantics: compare by UTF-16 code units.
        self.0.encode_utf16().cmp(other.0.encode_utf16())
    }
}

impl fmt::Display for JString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Java's `+` string-concatenation operator.
impl Add for JString {
    type Output = JString;

    fn add(self, rhs: JString) -> JString {
        let mut s = String::with_capacity(self.0.len() + rhs.0.len());
        s.push_str(&self.0);
        s.push_str(&rhs.0);
        JString(Arc::from(s.as_str()))
    }
}

impl Add<&str> for JString {
    type Output = JString;

    fn add(self, rhs: &str) -> JString {
        let mut s = String::with_capacity(self.0.len() + rhs.len());
        s.push_str(&self.0);
        s.push_str(rhs);
        JString(Arc::from(s.as_str()))
    }
}

/// Java-style `String.format(fmt, args...)` — replaces `%s`, `%d`, `%f`, etc.
/// with the pre-formatted string representations of the arguments.
///
/// Supported: `%s`, `%d`, `%f`, `%e`, `%x`, `%o`, `%b`, `%n`, `%%`, and a limited
/// set of width/precision modifiers like `%-10s`, `%08d`, and `%.2f`.
pub fn jformat(fmt: JString, args: &[String]) -> JString {
    let fmt = fmt.as_str();
    let mut result = String::with_capacity(fmt.len() + 32);
    let mut arg_idx = 0;
    let chars: Vec<char> = fmt.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '%' {
            result.push(chars[i]);
            i += 1;
            continue;
        }
        i += 1; // skip '%'
        if i >= chars.len() {
            break;
        }
        // Parse flags
        let mut flags = String::new();
        while i < chars.len() && matches!(chars[i], '-' | '+' | '0' | ' ' | '#') {
            flags.push(chars[i]);
            i += 1;
        }
        // Parse width
        let mut width = String::new();
        while i < chars.len() && chars[i].is_ascii_digit() {
            width.push(chars[i]);
            i += 1;
        }
        // Parse precision
        let mut precision = String::new();
        if i < chars.len() && chars[i] == '.' {
            i += 1;
            while i < chars.len() && chars[i].is_ascii_digit() {
                precision.push(chars[i]);
                i += 1;
            }
        }
        if i >= chars.len() {
            break;
        }
        let spec = chars[i];
        i += 1;
        match spec {
            '%' => result.push('%'),
            'n' => result.push('\n'),
            's' | 'S' | 'd' | 'f' | 'e' | 'g' | 'x' | 'X' | 'o' | 'c' | 'b' => {
                let val = if arg_idx < args.len() {
                    &args[arg_idx]
                } else {
                    ""
                };
                arg_idx += 1;

                // Try to apply width/precision formatting
                let formatted = match spec {
                    'f' => {
                        if let Ok(v) = val.parse::<f64>() {
                            let prec: usize = if precision.is_empty() {
                                6
                            } else {
                                precision.parse().unwrap_or(6)
                            };
                            format!("{:.prec$}", v, prec = prec)
                        } else {
                            val.to_string()
                        }
                    }
                    'e' | 'E' => {
                        if let Ok(v) = val.parse::<f64>() {
                            let prec: usize = if precision.is_empty() {
                                6
                            } else {
                                precision.parse().unwrap_or(6)
                            };
                            if spec == 'E' {
                                format!("{:.prec$E}", v, prec = prec)
                            } else {
                                format!("{:.prec$e}", v, prec = prec)
                            }
                        } else {
                            val.to_string()
                        }
                    }
                    'x' | 'X' => {
                        if let Ok(v) = val.parse::<i64>() {
                            if spec == 'X' {
                                format!("{:X}", v)
                            } else {
                                format!("{:x}", v)
                            }
                        } else {
                            val.to_string()
                        }
                    }
                    'o' => {
                        if let Ok(v) = val.parse::<i64>() {
                            format!("{:o}", v)
                        } else {
                            val.to_string()
                        }
                    }
                    'b' => {
                        // Java %b: "true" if non-null and non-false
                        match val {
                            "false" | "0" | "" => "false".to_string(),
                            _ => "true".to_string(),
                        }
                    }
                    _ => val.to_string(),
                };

                // Apply width/alignment
                if !width.is_empty() {
                    let w: usize = width.parse().unwrap_or(0);
                    if flags.contains('-') {
                        result.push_str(&format!("{:<w$}", formatted, w = w));
                    } else if flags.contains('0') && matches!(spec, 'd' | 'f' | 'e' | 'x' | 'o') {
                        result.push_str(&format!("{:0>w$}", formatted, w = w));
                    } else {
                        result.push_str(&format!("{:>w$}", formatted, w = w));
                    }
                } else {
                    result.push_str(&formatted);
                }
            }
            _ => {
                // Unknown specifier — pass through
                result.push('%');
                result.push_str(&flags);
                result.push_str(&width);
                if !precision.is_empty() {
                    result.push('.');
                    result.push_str(&precision);
                }
                result.push(spec);
            }
        }
    }
    JString::from(result.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concat_operator() {
        let a = JString::from("Hello, ");
        let b = JString::from("World!");
        assert_eq!((a + b).as_str(), "Hello, World!");
    }

    #[test]
    fn length() {
        assert_eq!(JString::from("abc").length(), 3);
    }

    #[test]
    fn char_at() {
        assert_eq!(JString::from("abc").char_at(1), 'b');
    }

    #[test]
    fn clone_shares_allocation() {
        let a = JString::from("shared");
        let b = a.clone();
        // Both should point to the same Arc — same pointer.
        assert!(Arc::ptr_eq(&a.0, &b.0));
    }

    #[test]
    fn jformat_basic() {
        let r = super::jformat(
            JString::from("Hello %s, you are %s years old"),
            &["World".into(), "30".into()],
        );
        assert_eq!(r.as_str(), "Hello World, you are 30 years old");
    }

    #[test]
    fn jformat_specifiers() {
        assert_eq!(
            super::jformat(JString::from("%d items"), &["42".into()]).as_str(),
            "42 items"
        );
        assert_eq!(super::jformat(JString::from("100%%"), &[]).as_str(), "100%");
        assert_eq!(
            super::jformat(JString::from("line1%nline2"), &[]).as_str(),
            "line1\nline2"
        );
    }

    #[test]
    #[should_panic(expected = "count is negative")]
    fn repeat_negative_panics() {
        let _ = JString::from("ab").repeat(-1);
    }
}
