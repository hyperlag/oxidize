//! Source map generation — maps Rust output lines back to Java source lines.
//!
//! The `.jtrans-map` format is a simple text file:
//! ```text
//! # jtrans source map v1
//! # rust_line -> java_line
//! 1 -> 0
//! 5 -> 3
//! 6 -> 4
//! ```
//!
//! Lines that do not correspond to any Java source (e.g., use declarations,
//! generated boilerplate) map to `0`.

use std::fmt;

/// A mapping from Rust source lines to Java source lines.
pub struct SourceMap {
    /// `entries[i]` is the 1-based Java line number that produced Rust line `i+1`,
    /// or `0` if there is no corresponding Java line.
    entries: Vec<u32>,
}

impl SourceMap {
    /// Build a source map by heuristically matching significant tokens between
    /// the Java source and the generated Rust source.
    ///
    /// The algorithm:
    /// 1. Extract "significant lines" from Java (non-empty, non-brace-only).
    /// 2. For each Rust line, find the best-matching Java line by comparing
    ///    normalised content (stripped of whitespace and common syntax differences).
    /// 3. Produce a mapping that preserves monotonicity where possible.
    pub fn build(java_source: &str, rust_source: &str) -> Self {
        let java_lines: Vec<&str> = java_source.lines().collect();
        let rust_lines: Vec<&str> = rust_source.lines().collect();

        let java_sigs: Vec<String> = java_lines.iter().map(|l| normalise(l)).collect();

        let mut entries = Vec::with_capacity(rust_lines.len());
        let mut last_java_line = 0u32;

        for rust_line in &rust_lines {
            let norm = normalise(rust_line);

            if norm.is_empty() || is_boilerplate(&norm) {
                entries.push(0);
                continue;
            }

            // Try to find a matching Java line, preferring lines near the last match.
            let best = find_best_match(&norm, &java_sigs, last_java_line);
            entries.push(best);
            if best > 0 {
                last_java_line = best;
            }
        }

        Self { entries }
    }
}

impl fmt::Display for SourceMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "# jtrans source map v1")?;
        writeln!(f, "# rust_line -> java_line")?;
        for (i, &java_line) in self.entries.iter().enumerate() {
            writeln!(f, "{} -> {}", i + 1, java_line)?;
        }
        Ok(())
    }
}

/// Normalise a line for fuzzy matching: lowercase, strip whitespace, remove
/// common syntax differences between Java and Rust.
fn normalise(line: &str) -> String {
    let s = line.trim().to_lowercase();
    s.replace("system.out.println", "println")
        .replace("string", "str")
        .replace("boolean", "bool")
        .replace([';', '{', '}'], "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Returns `true` for lines that are clearly generated boilerplate (use
/// statements, `#[allow(...)]`, `fn main`, etc.) and should map to line 0.
fn is_boilerplate(normalised: &str) -> bool {
    normalised.starts_with("use ")
        || normalised.starts_with("#[")
        || normalised.starts_with("//")
        || normalised == "fn main()"
}

/// Find the best-matching Java line for a normalised Rust line. Returns a
/// 1-based Java line number, or 0 if no reasonable match is found.
fn find_best_match(rust_norm: &str, java_sigs: &[String], hint_line: u32) -> u32 {
    let mut best_score = 0usize;
    let mut best_line = 0u32;

    for (i, java_sig) in java_sigs.iter().enumerate() {
        if java_sig.is_empty() {
            continue;
        }

        let score = similarity(rust_norm, java_sig);
        if score == 0 {
            continue;
        }

        // Prefer lines near the previous match to preserve monotonicity.
        let proximity_bonus = if hint_line > 0 {
            let dist = (((i + 1) as i64) - hint_line as i64).unsigned_abs() as usize;
            5usize.saturating_sub(dist)
        } else {
            0
        };

        let adjusted = score + proximity_bonus;
        if adjusted > best_score {
            best_score = adjusted;
            best_line = (i + 1) as u32;
        }
    }

    // Require a minimum similarity to avoid false matches.
    if best_score >= 3 {
        best_line
    } else {
        0
    }
}

/// Simple word-overlap similarity between two normalised lines.
fn similarity(a: &str, b: &str) -> usize {
    let a_words: Vec<&str> = a.split_whitespace().collect();
    let b_words: Vec<&str> = b.split_whitespace().collect();
    a_words.iter().filter(|w| b_words.contains(w)).count()
}
