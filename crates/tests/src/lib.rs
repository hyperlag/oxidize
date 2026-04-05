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
    use std::sync::OnceLock;
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
    fn test_labeled_break() {
        check(
            "LabeledBreak.java",
            "0,0\n0,1\n0,2\n0,3\n1,0\n1,1\n1,2\n1,3\n2,0\n2,1\ndone",
        );
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

    #[test]
    fn test_reference_cast() {
        check("ReferenceCast.java", "hello\n5\nworld");
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

    // Stage 6 — Reflection & Dynamic Dispatch
    #[test]
    fn test_get_class() {
        check("GetClassTest.java", "GetClassTest\nGetClassTest");
    }

    #[test]
    fn test_tostring_override() {
        check(
            "ToStringOverride.java",
            "Point(3, 4)\nThe point is: Point(3, 4)",
        );
    }

    #[test]
    fn test_equals_contract() {
        check("EqualsContract.java", "true\nfalse");
    }

    #[test]
    fn test_hashcode_consistency() {
        check("HashCodeConsistency.java", "true\nfalse");
    }

    #[test]
    fn test_annotation_basic() {
        check("AnnotationBasic.java", "LOG: hello\nFormatter(LOG)");
    }

    // ── Stage 7: Standard Library Coverage ────────────────────────────────

    #[test]
    fn test_math_methods() {
        check("MathMethods.java", "5\n7\n3\n4\n1024\n12");
    }

    #[test]
    fn test_string_builder() {
        check(
            "StringBuilderTest.java",
            "Hello World\n11\ndlroW olleH\nHello, World",
        );
    }

    #[test]
    fn test_optional() {
        check("OptionalTest.java", "true\nhello\nfalse\ndefault");
    }

    #[test]
    fn test_regex() {
        check("RegexTest.java", "true\n123\ntrue\nfalse");
    }

    #[test]
    fn test_local_date() {
        check("LocalDateTest.java", "2024\n3\n15\n2024-04-04\n2024-05-15");
    }

    #[test]
    fn test_big_integer() {
        check("BigIntegerTest.java", "300\n20000\n50\n-1");
    }

    #[test]
    fn test_stream() {
        check("StreamTest.java", "4\n1\n3\n5\n8");
    }

    // ── Stage 8: Build Integration & Tooling Tests ────────────────────────

    fn workspace_dir() -> PathBuf {
        let md = manifest_dir();
        md.parent().unwrap().parent().unwrap().to_path_buf()
    }

    fn jtrans_bin() -> PathBuf {
        let mut base = workspace_dir().join("target");
        if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
            base = PathBuf::from(dir);
        }
        let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
        let name = if cfg!(windows) {
            "jtrans.exe"
        } else {
            "jtrans"
        };
        base.join(profile).join(name)
    }

    static JTRANS_BUILT: OnceLock<()> = OnceLock::new();

    fn ensure_jtrans_built() {
        JTRANS_BUILT.get_or_init(|| {
            let ws = workspace_dir();
            let build = Command::new("cargo")
                .args(["build", "-p", "jtrans"])
                .current_dir(&ws)
                .env("RUSTFLAGS", "")
                .output()
                .expect("failed to build jtrans");
            assert!(
                build.status.success(),
                "jtrans build failed: {}",
                String::from_utf8_lossy(&build.stderr)
            );
        });
    }

    #[test]
    fn test_cli_translate_subcommand() {
        ensure_jtrans_built();

        let tmp = TempDir::new().unwrap();
        let output_dir = tmp.path().join("rust-out");
        let hello = java_dir().join("HelloWorld.java");

        let result = Command::new(jtrans_bin())
            .args([
                "translate",
                "--input",
                hello.to_str().unwrap(),
                "--output",
                output_dir.to_str().unwrap(),
                "--no-incremental",
                "--no-source-map",
                "--no-cargo-toml",
            ])
            .output()
            .expect("failed to run jtrans");

        assert!(
            result.status.success(),
            "jtrans translate failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(output_dir.join("src").join("helloworld.rs").exists());
    }

    #[test]
    fn test_cli_cargo_toml_generation() {
        ensure_jtrans_built();

        let tmp = TempDir::new().unwrap();
        let output_dir = tmp.path().join("rust-out");
        let hello = java_dir().join("HelloWorld.java");

        let result = Command::new(jtrans_bin())
            .args([
                "translate",
                "--input",
                hello.to_str().unwrap(),
                "--output",
                output_dir.to_str().unwrap(),
                "--no-incremental",
                "--no-source-map",
            ])
            .output()
            .expect("failed to run jtrans");

        assert!(result.status.success());
        let cargo_toml = output_dir.join("Cargo.toml");
        assert!(cargo_toml.exists(), "Cargo.toml should be generated");
        let content = fs::read_to_string(&cargo_toml).unwrap();
        assert!(content.contains("[package]"));
        assert!(content.contains("java-compat"));
    }

    #[test]
    fn test_cli_source_map_generation() {
        ensure_jtrans_built();

        let tmp = TempDir::new().unwrap();
        let output_dir = tmp.path().join("rust-out");
        let hello = java_dir().join("HelloWorld.java");

        let result = Command::new(jtrans_bin())
            .args([
                "translate",
                "--input",
                hello.to_str().unwrap(),
                "--output",
                output_dir.to_str().unwrap(),
                "--no-incremental",
                "--no-cargo-toml",
            ])
            .output()
            .expect("failed to run jtrans");

        assert!(result.status.success());
        let map_file = output_dir.join("src").join("helloworld.jtrans-map");
        assert!(map_file.exists(), "source map should be generated");
        let content = fs::read_to_string(&map_file).unwrap();
        assert!(content.contains("# jtrans source map v1"));
        assert!(content.contains(" -> "));
    }

    #[test]
    fn test_cli_incremental_cache() {
        ensure_jtrans_built();

        let tmp = TempDir::new().unwrap();
        let output_dir = tmp.path().join("rust-out");
        let hello = java_dir().join("HelloWorld.java");
        let rs_out = output_dir.join("src").join("helloworld.rs");

        // First translation should produce the output file and create a cache.
        let result1 = Command::new(jtrans_bin())
            .args([
                "translate",
                "--input",
                hello.to_str().unwrap(),
                "--output",
                output_dir.to_str().unwrap(),
                "--no-source-map",
                "--no-cargo-toml",
            ])
            .output()
            .expect("failed to run jtrans");
        assert!(
            result1.status.success(),
            "first run failed: {}",
            String::from_utf8_lossy(&result1.stderr)
        );

        // Output file and cache must exist after first run.
        assert!(rs_out.exists(), "output file should exist after first run");
        let cache_file = output_dir.join(".jtrans-cache");
        assert!(
            cache_file.exists(),
            "cache file should exist after first run"
        );

        // Record the modification time of the output file after the first run.
        let mtime_after_first = fs::metadata(&rs_out)
            .expect("cannot stat output file")
            .modified()
            .expect("mtime not available on this platform");

        // Second translation should skip the unchanged file: the output must
        // not be rewritten (mtime should stay the same).
        let result2 = Command::new(jtrans_bin())
            .args([
                "translate",
                "--input",
                hello.to_str().unwrap(),
                "--output",
                output_dir.to_str().unwrap(),
                "--no-source-map",
                "--no-cargo-toml",
            ])
            .output()
            .expect("failed to run jtrans");
        assert!(
            result2.status.success(),
            "second run failed: {}",
            String::from_utf8_lossy(&result2.stderr)
        );

        let mtime_after_second = fs::metadata(&rs_out)
            .expect("cannot stat output file")
            .modified()
            .expect("mtime not available on this platform");

        assert_eq!(
            mtime_after_first, mtime_after_second,
            "output file should not be rewritten when source is unchanged"
        );
    }

    #[test]
    fn test_cli_init_maven() {
        ensure_jtrans_built();

        let tmp = TempDir::new().unwrap();

        let result = Command::new(jtrans_bin())
            .args(["init-maven", "--output", tmp.path().to_str().unwrap()])
            .output()
            .expect("failed to run jtrans");

        assert!(result.status.success());
        let pom = tmp.path().join("jtrans-maven-plugin.xml");
        assert!(pom.exists(), "Maven plugin fragment should be generated");
        let content = fs::read_to_string(&pom).unwrap();
        assert!(content.contains("exec-maven-plugin"));
        assert!(content.contains("jtrans"));
    }

    #[test]
    fn test_cli_init_gradle() {
        ensure_jtrans_built();

        let tmp = TempDir::new().unwrap();

        let result = Command::new(jtrans_bin())
            .args(["init-gradle", "--output", tmp.path().to_str().unwrap()])
            .output()
            .expect("failed to run jtrans");

        assert!(result.status.success());
        let gradle = tmp.path().join("jtrans.gradle.kts");
        assert!(
            gradle.exists(),
            "Gradle plugin fragment should be generated"
        );
        let content = fs::read_to_string(&gradle).unwrap();
        assert!(content.contains("translateToRust"));
        assert!(content.contains("jtrans"));
    }

    #[test]
    fn test_cli_directory_input() {
        ensure_jtrans_built();

        // Create a temp dir with two Java files.
        let tmp = TempDir::new().unwrap();
        let input_dir = tmp.path().join("java-src");
        fs::create_dir_all(&input_dir).unwrap();
        fs::copy(
            java_dir().join("HelloWorld.java"),
            input_dir.join("HelloWorld.java"),
        )
        .unwrap();
        fs::copy(
            java_dir().join("Arithmetic.java"),
            input_dir.join("Arithmetic.java"),
        )
        .unwrap();

        let output_dir = tmp.path().join("rust-out");
        let result = Command::new(jtrans_bin())
            .args([
                "translate",
                "--input",
                input_dir.to_str().unwrap(),
                "--output",
                output_dir.to_str().unwrap(),
                "--no-incremental",
                "--no-source-map",
                "--no-cargo-toml",
            ])
            .output()
            .expect("failed to run jtrans");

        assert!(
            result.status.success(),
            "jtrans should translate a directory: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(output_dir.join("src").join("helloworld.rs").exists());
        assert!(output_dir.join("src").join("arithmetic.rs").exists());
    }

    // ── Stage 9: Real-world validation programs ───────────────────────────

    #[test]
    fn test_json_parser() {
        check(
            "JsonParser.java",
            "STRING:hello world
NUM:42
NUM:-17
BOOL:true
BOOL:false
NULL
OBJ:{name=STRING:Alice, age=NUM:30}
ARR:[NUM:1, NUM:2, NUM:3]
OBJ:{person=OBJ:{name=STRING:Bob}, active=BOOL:true}
ARR:[STRING:hello, NUM:42, BOOL:true, NULL]
STRING:line1
line2
JSON parser tests complete",
        );
    }

    #[test]
    fn test_csv_parser() {
        check(
            "CsvParser.java",
            "Rows: 5
Cols: 3
Header[0]: name
Header[1]: age
Header[2]: city
Cell[1,0]: Alice
Cell[1,1]: 30
Cell[2,0]: Bob
Cell[4,2]: Chicago
Sum of ages: 120
--- Filter city=NYC ---
name,age,city
Alice,30,NYC
Carol,30,NYC
Sum of x: 9
Sum of y: 12
CSV parser tests complete",
        );
    }

    #[test]
    fn test_expr_calc() {
        check(
            "ExprCalc.java",
            "2 + 3 = 5
10 - 4 = 6
3 * 7 = 21
20 / 4 = 5
2 + 3 * 4 = 14
(2 + 3) * 4 = 20
((2 + 3) * (4 - 1)) = 15
-5 + 3 = -2
100 - 2 * 3 * 4 + 5 = 81
RPN 3 4 + = 7
RPN 5 3 - 4 * = 8
RPN 2 3 + 4 5 + * = 45
fib(10) = 55
fib(20) = 6765
gcd(12, 8) = 4
lcm(12, 8) = 24
2^10 = 1024
3^5 = 243
Primes up to 30: 2 3 5 7 11 13 17 19 23 29
Expression calculator tests complete",
        );
    }

    // ── Advanced Collections ──────────────────────────────────────────────

    #[test]
    fn test_linked_list_basic() {
        check("LinkedListBasic.java", "5\n5\n35\n5\n35\n3\n10\n20\n30");
    }

    #[test]
    fn test_priority_queue_basic() {
        check("PriorityQueueBasic.java", "4\n5\n5\n10\n20\n30\ntrue");
    }

    #[test]
    fn test_tree_map_basic() {
        check("TreeMapBasic.java", "3\nten\ntrue\n10\n30\n2\nfalse");
    }

    #[test]
    fn test_tree_set_basic() {
        check("TreeSetBasic.java", "3\ntrue\n10\n30\n10\n20\n30");
    }

    #[test]
    fn test_linked_hash_map_basic() {
        check("LinkedHashMapBasic.java", "3\n1\ntrue\n2\nfalse");
    }

    #[test]
    fn test_linked_hash_set_basic() {
        check("LinkedHashSetBasic.java", "3\ntrue\n30\n10\n20");
    }

    #[test]
    fn test_collections_sort() {
        check("CollectionsSort.java", "5\n10\n20\n30\n30\n20\n10\n5");
    }

    // ── Enum tests ────────────────────────────────────────────────────────

    #[test]
    fn test_enum_basic() {
        check("EnumBasic.java", "GREEN\n1\nGREEN\n3");
    }

    #[test]
    fn test_enum_switch() {
        check("EnumSwitch.java", "Weekday\nWeekday\nWeekend\nWeekend");
    }

    #[test]
    fn test_enum_fields() {
        check("EnumFields.java", "1\n25\nDIME\n4");
    }

    #[test]
    fn test_enum_compare() {
        check("EnumCompare.java", "true\nfalse\ntrue\nFALL");
    }

    #[test]
    fn test_enum_map_basic() {
        check("EnumMapBasic.java", "3\nMonday\ntrue\nfalse\n2\nfalse");
    }

    #[test]
    fn test_enum_set_basic() {
        check(
            "EnumSetBasic.java",
            "2\ntrue\nfalse\n1\nfalse\n2\ntrue\nfalse",
        );
    }

    #[test]
    fn test_enum_interface() {
        check(
            "EnumInterface.java",
            "Shape:CIRCLE\nShape:SQUARE\nShape:TRIANGLE",
        );
    }

    #[test]
    fn test_enum_constant_body() {
        check("EnumConstantBody.java", "7\n7\n30");
    }

    #[test]
    fn test_buffered_reader_writer() {
        check(
            "BufferedReaderWriter.java",
            "Hello from BufferedWriter\nSecond line\nThird line\nExists: true\nDeleted: true",
        );
    }

    #[test]
    fn test_print_writer() {
        check(
            "PrintWriterTest.java",
            "Line one\nLine two\nLine three\nFrom FileWriter\nDone",
        );
    }

    #[test]
    fn test_nio_files() {
        check(
            "NioFilesTest.java",
            "File exists: true\nIs regular file: true\nContent: Hello NIO\nSize: 9\nalpha\nbeta\ngamma\nAbsolute path ends with test_nio.txt: true\nFile name: test_nio.txt\nDeleted: true",
        );
    }

    #[test]
    fn test_scanner() {
        check("ScannerTest.java", "Hello World\n42\n3.14\nLast line\nDone");
    }

    #[test]
    fn test_generic_bounded() {
        check("GenericBounded.java", "42\nbounded");
    }

    #[test]
    fn test_generic_multi_bound() {
        check("GenericMultiBound.java", "99");
    }

    #[test]
    fn test_generic_wildcard() {
        check("GenericWildcard.java", "0\n0");
    }

    #[test]
    fn test_generic_raw_type() {
        check("GenericRawType.java", "0\ntrue");
    }

    #[test]
    fn test_big_decimal() {
        check(
            "BigDecimalTest.java",
            "a = 3.14159\n\
             b = 2.71828\n\
             a + b = 5.85987\n\
             a - b = 0.42331\n\
             a * b = 8.5397212652\n\
             10 / 3 (scale=4, HALF_UP) = 3.3333\n\
             2.0 compareTo 2.00 = 0\n\
             2.0 equals 2.00 = false\n\
             ZERO = 0\n\
             ONE = 1\n\
             TEN = 10\n\
             pi rounded to 2 = 3.14\n\
             abs(-5.5) = 5.5\n\
             negate(-5.5) = 5.5\n\
             valueOf(42) = 42\n\
             intValue = 123\n\
             longValue = 123\n\
             doubleValue = 123.456\n\
             signum(3.14) = 1\n\
             signum(-5.5) = -1\n\
             signum(0) = 0\n\
             2^10 = 1024\n\
             max(a, b) = 3.14159\n\
             min(a, b) = 2.71828",
        );
    }

    #[test]
    fn test_url() {
        check(
            "URLTest.java",
            "URL: http://example.com:8080/path/to/resource?key=value&foo=bar#section\n\
             Protocol: http\n\
             Host: example.com\n\
             Port: 8080\n\
             Path: /path/to/resource\n\
             Query: key=value&foo=bar\n\
             Ref: section\n\
             File: /path/to/resource?key=value&foo=bar\n\
             DefaultPort: 80\n\
             Protocol2: https\n\
             Host2: www.example.com\n\
             Port2: -1\n\
             Path2: /index.html\n\
             DefaultPort2: 443\n\
             Host3: localhost\n\
             Port3: -1\n\
             Path3: /test\n\
             toString: http://example.com:8080/path/to/resource?key=value&foo=bar#section\n\
             toExternalForm: http://example.com:8080/path/to/resource?key=value&foo=bar#section",
        );
    }

    // ── java.time tests ───────────────────────────────────────────────────

    #[test]
    fn test_local_time() {
        check(
            "LocalTimeTest.java",
            "t1 = 10:30\n\
             t2 = 14:45:30\n\
             hour = 14\n\
             minute = 45\n\
             second = 30\n\
             t1 + 3h = 13:30\n\
             t1 + 45m = 11:15\n\
             t2 - 2h = 12:45:30\n\
             t1 before t2 = true\n\
             t2 after t1 = true\n\
             23:00 + 3h = 02:00\n\
             parsed = 08:15:30\n\
             secondOfDay = 29730\n\
             t1 withHour(20) = 20:30",
        );
    }

    #[test]
    fn test_local_date_time() {
        check(
            "LocalDateTimeTest.java",
            "dt1 = 2025-03-15T10:30\n\
             dt2 = 2025-12-25T18:00\n\
             year = 2025\n\
             month = 3\n\
             day = 15\n\
             hour = 10\n\
             minute = 30\n\
             date = 2025-03-15\n\
             time = 10:30\n\
             dt1 + 10d = 2025-03-25T10:30\n\
             dt1 + 3mo = 2025-06-15T10:30\n\
             dt1 + 5h = 2025-03-15T15:30\n\
             dt1 before dt2 = true\n\
             date.atTime = 2025-06-01T09:30\n\
             parsed = 2025-07-04T12:00",
        );
    }

    #[test]
    fn test_duration_period() {
        check(
            "DurationPeriodTest.java",
            "d1 = PT1H1M1S\n\
             d1 seconds = 3661\n\
             d1 toMinutes = 61\n\
             d1 toHours = 1\n\
             d2 toMillis = 2500\n\
             d3 = PT2H\n\
             d4 = PT1H30M\n\
             zero.isZero = true\n\
             d1.isZero = false\n\
             d1.isNegative = false\n\
             d3 * 3 = PT6H\n\
             p1 = P1Y2M3D\n\
             p1 years = 1\n\
             p1 months = 2\n\
             p1 days = 3\n\
             p2 = P30D\n\
             p3 = P6M\n\
             p4 = P14D\n\
             pz.isZero = true\n\
             between = P2M14D",
        );
    }

    #[test]
    fn test_date_time_formatter() {
        check(
            "DateTimeFormatterTest.java",
            "formatted date = 2025/03/15\n\
             formatted time = 14:30:45\n\
             formatted dt = 2025-12-25 18:00\n\
             euro format = 15/03/2025",
        );
    }

    #[test]
    fn test_string_format() {
        check(
            "StringFormatTest.java",
            "Hello, World!\n\
             Count: 42\n\
             Pi: 3.14\n\
             Cart has 5 items\n\
             Hex: ff\n\
             Oct: 10\n\
             100%\n\
             [     right]\n\
             [left      ]\n\
             joined = a, b, c\n\
             printf: test 99",
        );
    }

    #[test]
    fn test_system() {
        // Non-deterministic values (millis, nanos) checked as >0
        let expected = format!(
            "millis > 0: true\n\
             nanos > 0: true\n\
             lineSep length: 1\n\
             fileSep = {file_sep}\n\
             osName empty: false\n\
             missing = default_val",
            file_sep = std::path::MAIN_SEPARATOR,
        );
        check("SystemTest.java", &expected);
    }

    #[test]
    fn test_reentrant_lock() {
        check(
            "ReentrantLockTest.java",
            "counter = 42\n\
             tryLock = true\n\
             condition created\n\
             done",
        );
    }

    #[test]
    fn test_read_write_lock() {
        check(
            "ReadWriteLockTest.java",
            "after write: 100\n\
             after read: 100\n\
             tryLock read = true\n\
             tryLock write = true\n\
             done",
        );
    }

    #[test]
    fn test_concurrent_hash_map() {
        check(
            "ConcurrentHashMapTest.java",
            "size = 3\n\
             has a = true\n\
             has z = false\n\
             size after putIfAbsent = 4\n\
             getOrDefault z = -1\n\
             size after remove = 3\n\
             isEmpty = false\n\
             size after clear = 0\n\
             isEmpty after clear = true",
        );
    }

    #[test]
    fn test_copy_on_write_array_list() {
        check(
            "CopyOnWriteArrayListTest.java",
            "size = 3\n\
             get(1) = b\n\
             contains b = true\n\
             contains z = false\n\
             indexOf c = 2\n\
             indexOf z = -1\n\
             replaced = b\n\
             get(1) after set = B\n\
             removed = a\n\
             size after remove = 2\n\
             isEmpty = false\n\
             isEmpty after clear = true",
        );
    }

    #[test]
    fn test_thread_local() {
        check(
            "ThreadLocalTest.java",
            "initial = 0\n\
             after set = 42\n\
             after remove = 0\n\
             done",
        );
    }

    #[test]
    fn test_executor_service() {
        check(
            "ExecutorServiceTest.java",
            "pool created\n\
             task ran\n\
             terminated = true\n\
             isShutdown = true",
        );
    }

    #[test]
    fn test_completable_future() {
        check(
            "CompletableFutureTest.java",
            "cf1 = hello\n\
             cf2 = 42\n\
             cf2 isDone = true\n\
             cf3 = value=10\n\
             done",
        );
    }

    #[test]
    fn test_lambda_block() {
        check(
            "LambdaBlock.java",
            "cf = hello world\n\
             cf2 = 42\n\
             cf3 = computed=50\n\
             done",
        );
    }

    #[test]
    fn test_text_block() {
        check(
            "TextBlock.java",
            "simple=hello\nworld\nindented=line1\n    line2\nline3\nnoTrailing=abc\ndef\ndone",
        );
    }

    #[test]
    fn test_map_iteration() {
        check(
            "MapIteration.java",
            "keys:\n\
             \x20\x20alpha\n\
             \x20\x20beta\n\
             \x20\x20gamma\n\
             sum=6\n\
             entries:\n\
             \x20\x20alpha=1\n\
             \x20\x20beta=2\n\
             \x20\x20gamma=3\n\
             done",
        );
    }

    #[test]
    fn test_process_builder() {
        check("ProcessBuilderTest.java", "output=hello\nexit=0");
    }

    #[test]
    fn test_process_env() {
        check("ProcessEnvTest.java", "wd=/tmp\nexit=0");
    }

    // ── Stage 3 ──────────────────────────────────────────────────────────────

    #[test]
    fn test_varargs_basic() {
        check("VarargsBasic.java", "6\n30\n0\n8");
    }

    #[test]
    fn test_static_counter() {
        check("StaticCounter.java", "3\n13");
    }

    #[test]
    fn test_multi_dim_array() {
        check("MultiDimArray.java", "0\n6\n11\nfalse\ntrue\ntrue");
    }

    #[test]
    fn test_static_initializer() {
        check("StaticInitializer.java", "42\n84");
    }

    // ── Stage 4 ──────────────────────────────────────────────────────────────

    #[test]
    fn test_properties() {
        check(
            "PropertiesTest.java",
            "Alice\n\
             30\n\
             default\n\
             size=2\n\
             has_name=true\n\
             empty=false\n\
             localhost\n\
             8080\n\
             p2size=2",
        );
    }

    #[test]
    fn test_timer() {
        check("TimerTest.java", "count >= 4: true\ndone");
    }

    #[test]
    fn test_zoned_date_time() {
        check(
            "ZonedDateTimeTest.java",
            "year=2024\n\
             month=3\n\
             day=15\n\
             hour=10\n\
             zone=UTC\n\
             nextDay=16\n\
             laterHour=13\n\
             before=true\n\
             after=true\n\
             zoneId=+05:00",
        );
    }

    #[test]
    fn test_http_client() {
        check(
            "HttpClientTest.java",
            "client created\nrequest built\nmethod=GET",
        );
    }

    // ── Stage 6: Records, Pattern Instanceof, Sealed Classes ─────────────

    #[test]
    fn test_record_basic() {
        check("RecordBasic.java", "3\n4\nPoint[x=3, y=4]\n(3,4)");
    }

    #[test]
    fn test_pattern_instanceof() {
        check("PatternInstanceof.java", "42\ndone");
    }

    #[test]
    fn test_sealed_class() {
        check("SealedClass.java", "10\n14");
    }

    #[test]
    fn test_anon_inner_class() {
        check("AnonInnerClass.java", "10\n15");
    }

    #[test]
    fn test_inner_class() {
        check("InnerClass.java", "3");
    }

    #[test]
    fn test_local_class() {
        check("LocalClass.java", "3,4\ndone");
    }
}
