//! Differential integration tests for the Java→Rust translator.
//!
//! Each test translates a Java source file through the full pipeline
//! (parse → type-check → codegen), compiles the generated Rust binary, runs
//! it, and asserts that its stdout matches the output produced by running the
//! original Java program with `javac` / `java`.

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use tempfile::TempDir;

    fn manifest_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn runtime_path() -> PathBuf {
        manifest_dir().parent().unwrap().join("runtime")
    }

    fn java_dir() -> PathBuf {
        manifest_dir().join("java")
    }

    /// Translate a Java source file through the full pipeline, compile the
    /// generated Rust, run it, and return its stdout.
    fn translate_and_run(java_file: &str) -> Result<String, String> {
        let java_path = java_dir().join(java_file);
        let source =
            fs::read_to_string(&java_path).map_err(|e| format!("Cannot read {java_file}: {e}"))?;

        // ── Translate ─────────────────────────────────────────────────────
        let module =
            parser::parse_to_ir(&source).map_err(|e| format!("Parse error in {java_file}: {e}"))?;
        let module = typeck::type_check(module)
            .map_err(|e| format!("Type-check error in {java_file}: {e}"))?;
        let rust_code =
            codegen::generate(&module).map_err(|e| format!("Codegen error in {java_file}: {e}"))?;

        // ── Build temporary Cargo project ─────────────────────────────────
        let tmp = TempDir::new().map_err(|e| e.to_string())?;
        let src_dir = tmp.path().join("src");
        fs::create_dir_all(&src_dir).map_err(|e| e.to_string())?;
        fs::write(src_dir.join("main.rs"), &rust_code).map_err(|e| e.to_string())?;

        let cargo_toml = format!(
            "[package]\nname = \"jtest\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
             [dependencies]\njava-compat = {{ path = \"{}\" }}\n",
            runtime_path().display()
        );
        fs::write(tmp.path().join("Cargo.toml"), &cargo_toml).map_err(|e| e.to_string())?;

        // ── Run ───────────────────────────────────────────────────────────
        let output = Command::new("cargo")
            .args(["run", "--quiet"])
            .current_dir(tmp.path())
            .env("RUSTFLAGS", "") // don't propagate -D warnings into generated code
            .output()
            .map_err(|e| format!("Failed to spawn cargo: {e}"))?;

        if !output.status.success() {
            return Err(format!(
                "cargo run failed for {java_file}.\n\
                 --- Generated Rust ---\n{rust_code}\n\
                 --- stderr ---\n{}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn check(java_file: &str, expected: &str) {
        match translate_and_run(java_file) {
            Ok(actual) => assert_eq!(
                actual.trim_end(),
                expected.trim_end(),
                "Output mismatch for {java_file}"
            ),
            Err(e) => panic!("{e}"),
        }
    }

    // ── Tests ─────────────────────────────────────────────────────────────

    #[test]
    fn test_hello_world() {
        check("HelloWorld.java", "Hello, World!");
    }

    #[test]
    fn test_arithmetic() {
        check("Arithmetic.java", "13\n7\n30\n3\n1");
    }

    #[test]
    fn test_arithmetic_double() {
        check("ArithmeticDouble.java", "4.0\n3.75");
    }

    #[test]
    fn test_arithmetic_long() {
        check("ArithmeticLong.java", "3000000000\n3000000000");
    }

    #[test]
    fn test_boolean_ops() {
        check("BooleanOps.java", "false\ntrue\nfalse\nfalse\ntrue");
    }

    #[test]
    fn test_bitwise_ops() {
        check("BitwiseOps.java", "8\n14\n6\n5\n20\n6");
    }

    #[test]
    fn test_comparison() {
        check("Comparison.java", "true\nfalse\ntrue\ntrue\ntrue\ntrue");
    }

    #[test]
    fn test_if_else() {
        check("IfElse.java", "big\npositive");
    }

    #[test]
    fn test_ternary_op() {
        check("TernaryOp.java", "big\n10");
    }

    #[test]
    fn test_while_loop() {
        check("WhileLoop.java", "0\n1\n2\n3\n4");
    }

    #[test]
    fn test_do_while_loop() {
        check("DoWhileLoop.java", "0\n1\n2");
    }

    #[test]
    fn test_for_loop() {
        check("ForLoop.java", "0\n1\n2\n3\n4");
    }

    #[test]
    fn test_for_loop_sum() {
        check("ForLoopSum.java", "55");
    }

    #[test]
    fn test_nested_loops() {
        check("NestedLoops.java", "1\n2\n3\n2\n4\n6\n3\n6\n9");
    }

    #[test]
    fn test_break_continue() {
        check("BreakContinue.java", "1\n3");
    }

    #[test]
    fn test_compound_assign() {
        check("CompoundAssign.java", "15\n12\n24\n6\n0");
    }

    #[test]
    fn test_pre_post_increment() {
        check("PrePostIncrement.java", "5\n6\n7\n7\n7\n6");
    }

    #[test]
    fn test_static_method() {
        check("StaticMethod.java", "9\n49");
    }

    #[test]
    fn test_multiple_static_methods() {
        check("MultipleStaticMethods.java", "7\n12\n7");
    }

    #[test]
    fn test_recursion() {
        check("Recursion.java", "120\n3628800");
    }

    #[test]
    fn test_fibonacci() {
        check("Fibonacci.java", "0\n1\n1\n2\n3\n5\n8\n13\n21");
    }

    #[test]
    fn test_gcd() {
        check("GCD.java", "6\n25");
    }

    #[test]
    fn test_power() {
        check("Power.java", "1024\n243");
    }

    #[test]
    fn test_max_min() {
        check("MaxMin.java", "7\n3\n-1");
    }

    #[test]
    fn test_multi_return() {
        check("MultiReturn.java", "negative\nzero\npositive");
    }

    #[test]
    fn test_string_concat() {
        check("StringConcat.java", "Hello, World!");
    }

    #[test]
    fn test_string_primitive_concat() {
        check("StringPrimitiveConcat.java", "The answer is: 42");
    }

    #[test]
    fn test_string_methods() {
        check("StringMethods.java", "13\nfalse");
    }

    #[test]
    fn test_int_array() {
        check("IntArray.java", "0\n1\n4\n9\n16");
    }

    #[test]
    fn test_fizz_buzz() {
        check(
            "FizzBuzz.java",
            "1\n2\nFizz\n4\nBuzz\nFizz\n7\n8\nFizz\nBuzz\n11\nFizz\n13\n14\nFizzBuzz",
        );
    }

    #[test]
    fn test_is_prime() {
        check(
            "IsPrime.java",
            "false\nfalse\ntrue\ntrue\nfalse\ntrue\nfalse\ntrue\nfalse\nfalse\nfalse\ntrue\nfalse\ntrue\nfalse\nfalse",
        );
    }

    #[test]
    fn test_reverse_int() {
        check("ReverseInt.java", "54321\n1");
    }

    // ── Stage 2: OOP ──────────────────────────────────────────────────────

    #[test]
    fn test_oop_counter() {
        check("OopCounter.java", "2");
    }

    #[test]
    fn test_oop_bank_account() {
        check("OopBankAccount.java", "Alice\n1300.0");
    }

    #[test]
    fn test_oop_inheritance() {
        check("OopInheritance.java", "Rex\nWoof\nWhiskers\nMeow");
    }

    #[test]
    fn test_oop_shapes() {
        check("OopShapes.java", "Circle\n78.53975\nRectangle\n24.0");
    }

    #[test]
    fn test_oop_multi_level() {
        check(
            "OopMultiLevel.java",
            "Toyota\n2020\n4\nToyota car\nTesla\n400\nTesla electric",
        );
    }

    #[test]
    fn test_oop_interface() {
        check(
            "OopInterface.java",
            "Good day, Alice\nFarewell, Alice\nHey Bob!",
        );
    }

    #[test]
    fn test_oop_super_field() {
        check("OopSuperField.java", "3\n4\nred\nred(3,4)\nred(4,6)");
    }

    #[test]
    fn test_oop_instanceof() {
        check("OopInstanceof.java", "true\ntrue\nfalse\ntrue");
    }

    // ── Stage 3: Generics & Collections ───────────────────────────────────

    #[test]
    fn test_list_basic() {
        check("ListBasic.java", "3\n20\n60");
    }

    #[test]
    fn test_map_basic() {
        check("MapBasic.java", "2\n95\ntrue\nfalse");
    }

    #[test]
    fn test_generic_class() {
        check("GenericClass.java", "42\nhello");
    }

    #[test]
    fn test_exception_basic() {
        check("ExceptionBasic.java", "caught: divide by zero\ndone");
    }

    #[test]
    fn test_exception_finally() {
        check("ExceptionFinally.java", "caught: oops\nfinally\nafter");
    }

    #[test]
    fn test_exception_multi_catch() {
        check(
            "ExceptionMultiCatch.java",
            "caught: bad arg\ncaught: bad state",
        );
    }

    #[test]
    fn test_exception_nested() {
        check(
            "ExceptionNested.java",
            "inner finally\nouter catch: inner\ndone",
        );
    }

    #[test]
    fn test_try_with_resources() {
        check(
            "TryWithResources.java",
            "open: file.txt\ndata from file.txt\nclose: file.txt\ndone",
        );
    }

    #[test]
    fn test_throws_decl() {
        check("ThrowsDecl.java", "caught: negative\nok: 5");
    }

    // ── Stage 5: Concurrency ──────────────────────────────────────────────

    #[test]
    fn test_atomic_counter() {
        check("AtomicCounter.java", "7\n7\n8\ntrue\n10");
    }

    #[test]
    fn test_count_down_latch() {
        check("CountDownLatchTest.java", "3\n1\nlatch done");
    }

    #[test]
    fn test_semaphore() {
        check("SemaphoreTest.java", "3\n1\n2\n0\n2");
    }

    #[test]
    fn test_synchronized_counter() {
        check("SynchronizedCounter.java", "3");
    }

    #[test]
    fn test_thread_join() {
        check("ThreadJoin.java", "hello from t1\nhello from t2\nmain done");
    }
}
