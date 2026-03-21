//! Integration tests for the Java→Rust translator pipeline.
//!
//! Each test drives the full parse → typeck → codegen pipeline on a Java
//! source file and verifies the output token stream.

#[cfg(test)]
mod stage1 {
    use codegen::generate;
    use parser::parse_source;
    use typeck::check;

    fn pipeline(src: &str) -> String {
        let mut module = parse_source(src).expect("parse must succeed");
        let _errors = check(&mut module); // type errors are non-fatal in tests
        let tokens = generate(&module).expect("codegen must succeed");
        tokens.to_string()
    }

    // ── HelloWorld ─────────────────────────────────────────────────────────

    const HELLO_WORLD: &str = include_str!("../../tests/java/HelloWorld.java");

    #[test]
    fn hello_world_parses_and_generates() {
        let output = pipeline(HELLO_WORLD);
        assert!(output.contains("fn main"), "must emit fn main");
        assert!(output.contains("println"), "must emit println");
        assert!(output.contains("Hello"), "must include the string literal");
    }

    // ── Arithmetic ─────────────────────────────────────────────────────────

    const ARITHMETIC: &str = include_str!("../../tests/java/Arithmetic.java");

    #[test]
    fn arithmetic_generates_methods() {
        let output = pipeline(ARITHMETIC);
        assert!(output.contains("fn add"), "must emit fn add");
        assert!(output.contains("fn multiply"), "must emit fn multiply");
        assert!(output.contains("fn remainder"), "must emit fn remainder");
        assert!(output.contains("i32"), "integer params must map to i32");
        assert!(output.contains("f64"), "double return must map to f64");
    }

    #[test]
    fn arithmetic_typeck_resolves_params() {
        let src = ARITHMETIC;
        let mut module = parse_source(src).expect("parse must succeed");
        let errors = check(&mut module);
        assert!(errors.is_empty(), "expected no typeck errors, got: {errors:?}");
    }

    // ── ControlFlow ────────────────────────────────────────────────────────

    const CONTROL_FLOW: &str = include_str!("../../tests/java/ControlFlow.java");

    #[test]
    fn control_flow_generates() {
        let output = pipeline(CONTROL_FLOW);
        assert!(output.contains("fn factorial"), "must emit factorial");
        assert!(output.contains("fn fibonacci"), "must emit fibonacci");
        assert!(output.contains("fn classify"), "must emit classify");
        assert!(output.contains("while"), "must emit while loop");
        assert!(output.contains("if"), "must emit if");
    }

    #[test]
    fn control_flow_typeck_no_errors() {
        let mut module = parse_source(CONTROL_FLOW).expect("parse must succeed");
        let errors = check(&mut module);
        assert!(errors.is_empty(), "expected no typeck errors, got: {errors:?}");
    }

    // ── StringOps ──────────────────────────────────────────────────────────

    const STRING_OPS: &str = include_str!("../../tests/java/StringOps.java");

    #[test]
    fn string_ops_generates() {
        let output = pipeline(STRING_OPS);
        assert!(output.contains("fn greet"), "must emit fn greet");
        assert!(output.contains("JString"), "String params must map to JString");
    }

    // ── IR round-trip ──────────────────────────────────────────────────────

    #[test]
    fn ir_serde_roundtrip_from_source() {
        let mut module = parse_source(HELLO_WORLD).expect("parse must succeed");
        let _errors = check(&mut module);
        let json = serde_json::to_string(&module).expect("serde must succeed");
        let back: ir::IrModule = serde_json::from_str(&json).expect("deserialise must succeed");
        assert_eq!(module, back);
    }
}

