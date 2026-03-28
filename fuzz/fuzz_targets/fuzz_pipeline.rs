//! Fuzz target for the full pipeline: parse → typecheck → codegen.
//!
//! Feeds grammar-aware Java source fragments through the entire translation
//! pipeline. The pipeline should never panic — any unhandled input should
//! return a clean error.

#![no_main]
use libfuzzer_sys::fuzz_target;

/// Grammar-aware Java source templates. The fuzzer fills in placeholders
/// with mutated data to exercise interesting parser paths.
const TEMPLATES: &[&str] = &[
    // Simple class with main method
    "public class {NAME} {{ public static void main(String[] args) {{ {BODY} }} }}",
    // Class with methods
    "public class {NAME} {{ {TYPE} {METHOD}({PARAMS}) {{ {BODY} }} }}",
    // Class with fields
    "public class {NAME} {{ {VISIBILITY} {TYPE} {FIELD}; }}",
    // Interface
    "public interface {NAME} {{ {TYPE} {METHOD}({PARAMS}); }}",
    // Class with inheritance
    "public class {NAME} extends {PARENT} {{ {BODY} }}",
    // Try-catch
    "public class {NAME} {{ public static void main(String[] args) {{ try {{ {BODY} }} catch (Exception e) {{ {BODY} }} }} }}",
];

const TYPES: &[&str] = &[
    "int", "long", "double", "boolean", "char", "String", "void", "float",
];
const NAMES: &[&str] = &[
    "Foo", "Bar", "Baz", "Test", "Main", "Hello", "Widget",
];
const METHODS: &[&str] = &[
    "run", "compute", "getValue", "process", "calc", "toString", "equals",
];
const BODIES: &[&str] = &[
    "System.out.println(42);",
    "int x = 1 + 2;",
    "if (true) {{ return; }}",
    "for (int i = 0; i < 10; i++) {{ }}",
    "while (false) {{ break; }}",
    "String s = \"hello\" + \" world\";",
    "return;",
    "",
];
const VISIBILITIES: &[&str] = &["public", "private", "protected", ""];

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    // Strategy 1: Raw UTF-8 through the parser (catches panics on arbitrary input)
    if let Ok(source) = std::str::from_utf8(data) {
        let _ = parser::parse_to_ir(source);

        // If parsing succeeds, try the full pipeline
        if let Ok(module) = parser::parse_to_ir(source) {
            if let Ok(typed) = typeck::type_check(module) {
                let _ = codegen::generate(&typed);
            }
        }
    }

    // Strategy 2: Grammar-aware generation from the fuzz input
    if data.len() >= 8 {
        let template_idx = data[0] as usize % TEMPLATES.len();
        let type_idx = data[1] as usize % TYPES.len();
        let name_idx = data[2] as usize % NAMES.len();
        let method_idx = data[3] as usize % METHODS.len();
        let body_idx = data[4] as usize % BODIES.len();
        let vis_idx = data[5] as usize % VISIBILITIES.len();

        let source = TEMPLATES[template_idx]
            .replace("{NAME}", NAMES[name_idx])
            .replace("{TYPE}", TYPES[type_idx])
            .replace("{METHOD}", METHODS[method_idx])
            .replace("{BODY}", BODIES[body_idx])
            .replace("{PARAMS}", "")
            .replace("{FIELD}", "value")
            .replace("{PARENT}", "Object")
            .replace("{VISIBILITY}", VISIBILITIES[vis_idx]);

        if let Ok(module) = parser::parse_to_ir(&source) {
            if let Ok(typed) = typeck::type_check(module) {
                let _ = codegen::generate(&typed);
            }
        }
    }
});
