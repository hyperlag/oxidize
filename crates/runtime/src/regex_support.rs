//! [`JPattern`] and [`JMatcher`] — Rust representations of `java.util.regex`.
//!
//! Mapping: `Pattern` → `JPattern`, `Matcher` → `JMatcher`.
//! Uses the `regex` crate for regex compilation and matching.

use crate::string::JString;
use regex::Regex;

/// Java `java.util.regex.Pattern`.
#[derive(Debug, Clone)]
pub struct JPattern {
    inner: Regex,
    source: String,
}

impl Default for JPattern {
    fn default() -> Self {
        Self {
            inner: Regex::new("").unwrap(),
            source: String::new(),
        }
    }
}

impl JPattern {
    /// Java `Pattern.compile(pattern)`.
    pub fn compile(pattern: JString) -> Self {
        let src = pattern.as_str().to_owned();
        // Java uses the same regex syntax as the `regex` crate for common patterns.
        let inner = Regex::new(&src).unwrap_or_else(|_| Regex::new("").unwrap());
        Self { inner, source: src }
    }

    /// Java `Pattern.matches(pattern, input)` — full-string match.
    pub fn static_matches(pattern: JString, input: JString) -> bool {
        let re_src = format!("^(?:{})$", pattern.as_str());
        Regex::new(&re_src)
            .map(|re| re.is_match(input.as_str()))
            .unwrap_or(false)
    }

    /// Java `pattern.matcher(input)`.
    pub fn matcher(&self, input: JString) -> JMatcher {
        JMatcher::new(self.inner.clone(), input)
    }

    /// Java `pattern.pattern()`.
    pub fn pattern(&self) -> JString {
        JString::from(self.source.as_str())
    }
}

/// Java `java.util.regex.Matcher`.
#[derive(Debug, Clone)]
pub struct JMatcher {
    pattern: Regex,
    input: String,
    captures: Option<Vec<Option<String>>>,
}

impl Default for JMatcher {
    fn default() -> Self {
        Self {
            pattern: Regex::new("").unwrap(),
            input: String::new(),
            captures: None,
        }
    }
}

impl JMatcher {
    pub fn new(pattern: Regex, input: JString) -> Self {
        Self {
            pattern,
            input: input.as_str().to_owned(),
            captures: None,
        }
    }

    /// Java `matcher.matches()` — matches the entire input.
    pub fn matches(&mut self) -> bool {
        let re_src = format!("^(?:{})$", self.pattern.as_str());
        if let Ok(full_re) = Regex::new(&re_src) {
            if let Some(caps) = full_re.captures(&self.input) {
                self.captures = Some(
                    caps.iter()
                        .map(|m| m.map(|m| m.as_str().to_owned()))
                        .collect(),
                );
                // Re-run original pattern captures for group() access
                if let Some(orig_caps) = self.pattern.captures(&self.input) {
                    self.captures = Some(
                        orig_caps
                            .iter()
                            .map(|m| m.map(|m| m.as_str().to_owned()))
                            .collect(),
                    );
                }
                return true;
            }
        }
        self.captures = None;
        false
    }

    /// Java `matcher.find()` — finds the next match.
    pub fn find(&mut self) -> bool {
        if let Some(caps) = self.pattern.captures(&self.input) {
            self.captures = Some(
                caps.iter()
                    .map(|m| m.map(|m| m.as_str().to_owned()))
                    .collect(),
            );
            true
        } else {
            self.captures = None;
            false
        }
    }

    /// Java `matcher.lookingAt()` — matches from the beginning (not necessarily the full string).
    pub fn lookingAt(&mut self) -> bool {
        let re_src = format!("^(?:{})", self.pattern.as_str());
        if let Ok(re) = Regex::new(&re_src) {
            if let Some(caps) = re.captures(&self.input) {
                self.captures = Some(
                    caps.iter()
                        .map(|m| m.map(|m| m.as_str().to_owned()))
                        .collect(),
                );
                return true;
            }
        }
        false
    }

    /// Java `matcher.group()` — returns whole match (group 0).
    pub fn group(&self) -> JString {
        self.captures
            .as_ref()
            .and_then(|caps| caps.first())
            .and_then(|s| s.as_ref())
            .map(|s| JString::from(s.as_str()))
            .unwrap_or(JString::from(""))
    }

    /// Java `matcher.group(n)` — returns capture group n (0-indexed like Java).
    pub fn group_n(&self, n: i32) -> JString {
        self.captures
            .as_ref()
            .and_then(|caps| caps.get(n as usize))
            .and_then(|s| s.as_ref())
            .map(|s| JString::from(s.as_str()))
            .unwrap_or(JString::from(""))
    }

    /// Java `matcher.groupCount()`.
    pub fn groupCount(&self) -> i32 {
        self.captures
            .as_ref()
            .map(|caps| (caps.len() as i32) - 1)
            .unwrap_or(0)
    }
}
