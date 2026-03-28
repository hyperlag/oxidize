//! Fuzz target for the parser: feeds arbitrary bytes as Java source code.
//!
//! This exercises tree-sitter-java parsing and IR lowering, ensuring the parser
//! does not panic on any input.

#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Only fuzz valid UTF-8 inputs (Java source is always text)
    if let Ok(source) = std::str::from_utf8(data) {
        // The parser should never panic, even on garbage input
        let _ = parser::parse_to_ir(source);
    }
});
