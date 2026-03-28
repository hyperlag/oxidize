# oxidize

A source-to-source translator that ingests Java code and produces idiomatic, memory-safe Rust.

## Goals

- Translate arbitrary Java programs to Rust that compiles without `unsafe` blocks
- Preserve functional equivalence: translated programs produce identical output to the original Java
- Emit clean, formatted Rust (`rustfmt` + `clippy`-clean output)

## Architecture

```
Java source (.java)
      |
  Java Parser          (tree-sitter-java)
      |  Java AST
  IR Lowering          (walker -> typed IR)
      |  Typed IR
  Type Checker         (symbol table, inheritance resolution)
      |  Annotated IR
  Rust Codegen         (trait mapping, struct composition)
      |  Rust tokens
  Post-process         (rustfmt)
      |
Rust source (.rs)
```

## Crates

| Crate | Purpose |
|---|---|
| `parser` | Wraps `tree-sitter-java`; parses `.java` source into typed IR |
| `ir` | Core intermediate representation: `IrType`, `IrExpr`, `IrStmt`, `IrDecl` |
| `typeck` | Type-checking and symbol-resolution pass over the IR |
| `codegen` | Lowers annotated IR to Rust token streams via `proc-macro2` / `quote` |
| `runtime` | `java-compat` crate: runtime types (`JString`, `JArray`, `JList`, `JMap`, `JOptional`, `JStream`, `JThread`, etc.) |
| `cli` | `jtrans` binary: CLI driver with `translate`, `init-maven`, `init-gradle` subcommands, watch mode, incremental cache, and source map generation |
| `tests` | Differential test suite (76 tests: translated Rust output vs. expected output) |

## Requirements

- Rust stable toolchain (`rustup` recommended)

## Building

```bash
cargo build --release
```

The `jtrans` binary will be placed at `target/release/jtrans`. You can also run it directly without installing:

```bash
cargo run --bin jtrans -- [OPTIONS] <file.java>
```

## Usage

### `jtrans translate` — translate Java to Rust

The primary command for translating Java source files or directories:

```
jtrans translate [OPTIONS] --input <INPUT>

Options:
  -i, --input <INPUT>          Input directory or file(s) containing Java source code
  -o, --output <OUTPUT>        Output directory for the generated Rust project [default: rust-out]
  -c, --classpath <CLASSPATH>  Classpath entries for type resolution
      --watch                  Watch input files and re-translate on changes
      --dump-ir                Print the IR as JSON after parsing
      --print                  Print generated Rust to stdout instead of writing to disk
      --no-incremental         Disable incremental caching (translate all files every run)
      --no-source-map          Disable .jtrans-map source map generation
      --no-cargo-toml          Disable Cargo.toml generation in the output directory
```

#### Translate a single file

```bash
jtrans translate --input HelloWorld.java
```

This writes a Cargo project skeleton to `rust-out/` containing:
- `src/helloworld.rs` — the translated Rust source
- `Cargo.toml` — a manifest that declares a local `java-compat` dependency (`java-compat = { path = "java-compat" }`). You must either place the `java-compat` crate at that path relative to the output directory or edit the dependency to point at your runtime crate before `cargo build` will succeed.
- `src/helloworld.jtrans-map` — a source map (Rust line → Java line)

#### Translate an entire directory

```bash
jtrans translate --input src/main/java/ --output rust-out/
```

All `.java` files are discovered recursively and translated.

#### Build and run the translated program

```bash
# Translate
jtrans translate --input HelloWorld.java --output hello-rs/

# Point java-compat to the runtime crate (edit the Cargo.toml path if needed)
# Then build and run
cd hello-rs && cargo run
```

#### Incremental mode (default)

By default, `jtrans` maintains a `.jtrans-cache` file in the output directory containing
SHA-256 hashes of each input file. On subsequent runs, unchanged files are skipped:

```bash
# First run: translates everything
jtrans translate --input src/ --output rust-out/

# Second run: skips unchanged files
jtrans translate --input src/ --output rust-out/
```

Disable with `--no-incremental` to force a full retranslation every time.

#### Watch mode

Use `--watch` to continuously monitor input files and re-translate on changes:

```bash
jtrans translate --input src/ --output rust-out/ --watch
```

Press Ctrl+C to stop.

#### Source maps

Each translated `.rs` file is accompanied by a `.jtrans-map` file that maps Rust line
numbers back to the original Java line numbers, useful for debugging:

```
# jtrans source map v1
# rust_line -> java_line
1 -> 0
5 -> 3
6 -> 4
```

Disable with `--no-source-map`.

#### Preview without writing files

```bash
jtrans translate --input HelloWorld.java --print
```

#### Debug the IR

```bash
jtrans translate --input HelloWorld.java --dump-ir
```

### `jtrans init-maven` — Maven integration

Generate a Maven plugin fragment using `exec-maven-plugin`:

```bash
jtrans init-maven --output .
```

This writes `jtrans-maven-plugin.xml`. Copy its contents into your `pom.xml`
`<build><plugins>` section. Then run:

```bash
mvn compile exec:exec@jtrans
```

### `jtrans init-gradle` — Gradle integration

Generate a Gradle build script fragment in Kotlin DSL:

```bash
jtrans init-gradle --output .
```

This writes `jtrans.gradle.kts`. Apply it in your `build.gradle.kts`:

```kotlin
apply(from = "jtrans.gradle.kts")
```

Then run:

```bash
./gradlew translateToRust
```

### Legacy mode

For backwards compatibility, positional arguments are still supported:

```bash
jtrans HelloWorld.java --output out/ --print
```

## Running Tests

Build and run the full workspace unit tests:

```bash
cargo test
```

The differential integration tests in `crates/tests` compile and run each translated Rust
program, then assert that stdout matches the expected output. No JDK is required to run
the tests. The suite currently contains **76 differential tests** covering Stages 1-9:

```bash
cargo test -p tests -- --test-threads=4
```

## Project Status

The project follows a staged delivery plan:

| Stage | Description | Status |
|---|---|---|
| 0 | Foundation and tooling: workspace, CI, tree-sitter smoke test | Complete |
| 1 | Core language: primitives, control flow, static methods, arrays | Complete (32/32 tests) |
| 2 | Object-oriented core: classes, inheritance, interfaces, `instanceof` | Complete (43/43 tests) |
| 3 | Generics and collections: `List`, `Map`, `Set`, generic classes | Complete (43/43 tests) |
| 4 | Exception handling: `try`/`catch`/`finally`/`throw`, multi-catch, try-with-resources, `throws` | Complete (49/49 tests) |
| 5 | Concurrency: `synchronized`, `Thread`, `java.util.concurrent` | Complete (54/54 tests) |
| 6 | Reflection and dynamic dispatch | Complete (59/59 tests) |
| 7 | Standard library coverage: `Math`, `Optional`, `Stream`, `regex`, `BigInteger`, `LocalDate`, `StringBuilder`, `File` | Complete (66/66 tests) |
| 8 | Build integration and tooling: `translate` subcommand, `--watch`, incremental cache, source maps, Cargo.toml generation, Maven/Gradle plugins | Complete (73/73 tests) |
| 9 | Validation, fuzzing, and hardening: cargo-fuzz, cargo miri, proptest, real-world Java ports, 80%+ coverage | Complete (76/76 tests) |
| 10 | Documentation and release | Complete |

### Stage 1: Supported Java features

- Primitive types: `int`, `long`, `double`, `float`, `boolean`, `char`, `byte`, `short`
- `String` literals and concatenation (including mixed primitive + String)
- All arithmetic, bitwise, comparison, and logical operators
- Compound assignment (`+=`, `-=`, `*=`, `/=`, `%=`, etc.)
- Pre- and post-increment/decrement (`++`, `--`)
- `if / else if / else`, `while`, `do-while`, `for` (including `break` and `continue`)
- Static methods with recursion
- Single-dimensional arrays (`int[]`, etc.)
- Single-class programs with `public static void main(String[] args)`
- `System.out.println` / `System.out.print` / `System.err.println`
- Ternary expressions

### Stage 2: Supported Java features

- Instance fields and instance methods
- Constructors (default and parameterized)
- Single inheritance (`extends`) with `super(args)` constructor delegation
- Inherited field and method access through the superclass chain
- Interface declarations (`interface`) and `implements`
- Method overriding
- `this` reference in instance methods and constructors
- `super` reference for delegating to parent class methods and constructors
- `instanceof` operator
- Classes are translated to Rust `struct`s with a `pub _super: ParentClass` composition field
- Interface methods are translated to Rust `trait` methods
- Delegation methods are auto-generated for inherited non-overridden methods

### Stage 3: Supported Java features

- Generic classes: `class Wrapper<T>` → `struct Wrapper<T>` with `impl<T: Clone + Default + Debug>`
- Boxed type mapping: `Integer` → `i32`, `Long` → `i64`, `Double` → `f64`, `Boolean` → `bool`, etc.
- `List<T>` / `ArrayList<T>` → `java_compat::JList<T>` (wraps `Vec<T>`)
- `Map<K,V>` / `HashMap<K,V>` → `java_compat::JMap<K,V>` (wraps `HashMap<K,V>`)
- `Set<T>` / `HashSet<T>` → `java_compat::JSet<T>` (wraps `HashSet<T>`)
- Collection constructors: `new ArrayList<>()` → `JList::new()`, etc.
- Enhanced `for` loop over collections: `for (T x : list)` → `for x in list.iter()`
- Collection methods: `size()`, `add()`, `get()`, `put()`, `contains()`, `isEmpty()`, `remove()`, `clear()`

### Stage 4: Supported Java features

- `throw new SomeException("message")` → `panic!("JException:SomeException:message")`
- `try { ... } catch (E e) { ... }` → `catch_unwind` + decoded `JException` match
- `finally { ... }` → always-executed block (before rethrow if exception is not caught)
- Multi-catch `catch (A | B e)` → OR-chained `is_instance_of` conditions
- Nested `try/catch/finally` blocks
- Try-with-resources `try (R r = new R()) { ... }` → desugared to `LocalVar` + `TryCatch` with `r.close()` in `finally`
- `throws` declarations parsed and stored in IR; exceptions propagate through method boundaries via panics (semantically equivalent to Java runtime behaviour)
- Exception hierarchy: `ArithmeticException`, `RuntimeException`, `IllegalArgumentException`, `IllegalStateException`, `NullPointerException`, `IndexOutOfBoundsException`, and others all recognised
- Unhandled exceptions (not matched by any catch) are rethrown via `resume_unwind`
- `e.getMessage()` on a caught exception returns the message string

### Stage 6: Supported Java features

- `obj.getClass()` returns a `JClass` descriptor injected into every generated class
- `JClass` supports `.getName()`, `.getSimpleName()`, `.getCanonicalName()` (all return the simple class name; no package prefix)
- `toString()` overrides automatically generate `impl std::fmt::Display` for the class, enabling `println!("{}", obj)` and string concatenation with `+`
- `equals(SameType)` type-checked with return type `bool`; `hashCode()` type-checked with return type `i32`
- `@Override` and `@Deprecated` annotations parsed and silently tolerated on any method
- Limitations: `Method.invoke`, `Field.get/set`, `Class.forName`, `getDeclaredMethods/Fields`, and `Proxy` are out of scope (runtime-only dynamic dispatch cannot be represented in a static Rust translation)

### Stage 5: Supported Java features

- `Thread` + `Runnable`: `new Thread(runnable)` → `JThread::new(move || { r.run(); })`, with `.start()` and `.join()`
- `Thread.sleep(ms)` → `JThread::sleep(ms)`
- `synchronized` methods → per-class `static OnceLock<(Mutex<()>, Condvar)>` monitor; body prefixed with lock acquisition
- `synchronized(expr) { ... }` blocks → global process-wide monitor via `java_compat::__sync_block_monitor()`
- `wait()` inside synchronized → `Condvar::wait` on the held guard (rebind pattern)
- `notify()` / `notifyAll()` inside synchronized → `Condvar::notify_one` / `notify_all`
- `volatile` primitive fields → `Arc<AtomicI32>` / `Arc<AtomicI64>` / `Arc<AtomicBool>`; reads emit `.load(SeqCst)`, writes emit `.store(SeqCst)`
- `AtomicInteger` / `AtomicLong` / `AtomicBoolean` → `JAtomicInteger` / `JAtomicLong` / `JAtomicBoolean` with full method support: `get`, `set`, `incrementAndGet`, `getAndIncrement`, `decrementAndGet`, `addAndGet`, `getAndAdd`, `compareAndSet`
- `CountDownLatch` → `JCountDownLatch` with `countDown()`, `await()`, `getCount()`
- `Semaphore` → `JSemaphore` with `acquire()`, `release()`, `availablePermits()`

### Java to Rust type mapping

| Java | Rust |
|---|---|
| `int` | `i32` |
| `long` | `i64` |
| `double` | `f64` |
| `float` | `f32` |
| `boolean` | `bool` |
| `char` | `char` |
| `byte` | `i8` |
| `short` | `i16` |
| `Integer` | `i32` |
| `Long` | `i64` |
| `Double` | `f64` |
| `Float` | `f32` |
| `Boolean` | `bool` |
| `Character` | `char` |
| `String` | `java_compat::JString` |
| `T[]` | `java_compat::JArray<T>` |
| `List<T>` / `ArrayList<T>` | `java_compat::JList<T>` |
| `Map<K,V>` / `HashMap<K,V>` | `java_compat::JMap<K,V>` |
| `Set<T>` / `HashSet<T>` | `java_compat::JSet<T>` |
| `Optional<T>` | `java_compat::JOptional<T>` |
| `Stream<T>` | `java_compat::JStream<T>` |
| `StringBuilder` | `java_compat::JStringBuilder` |
| `BigInteger` | `java_compat::JBigInteger` |
| `Pattern` / `Matcher` | `java_compat::JPattern` / `JMatcher` |
| `LocalDate` | `java_compat::JLocalDate` |
| `File` | `java_compat::JFile` |
| `AtomicInteger` | `java_compat::JAtomicInteger` |
| `CountDownLatch` | `java_compat::JCountDownLatch` |
| `Semaphore` | `java_compat::JSemaphore` |
| `Thread` | `java_compat::JThread` |
| `ClassName` | `ClassName` (generated struct) |

### Stage 7: Standard library coverage

- `Math` static methods: `abs`, `max`, `min`, `pow`, `sqrt`, `floor`, `ceil`, `round`, `log`, `sin`, `cos`, `tan`, `exp`, `random`
- `StringBuilder`: `append`, `toString`, `length`, `charAt`, `reverse`, `insert`, `delete`
- `Optional<T>`: `of`, `empty`, `ofNullable`, `isPresent`, `get`, `orElse`, `ifPresent`, `filter`, `map`
- `Stream<T>` API: `filter`, `map`, `sorted`, `distinct`, `collect(Collectors.toList())`, lambda expression support
- `Pattern` / `Matcher`: `compile`, `matcher`, `find`, `group`, `matches`, `lookingAt`
- `LocalDate`: `of`, `now`, `getYear`, `getMonthValue`, `getDayOfMonth`, `plusDays`, `plusMonths`, `plusYears`
- `BigInteger`: `valueOf`, construction from string, `add`, `subtract`, `multiply`, `divide`, `mod`, `pow`, `abs`, `gcd`, `compareTo`
- `File`: `exists`, `isFile`, `isDirectory`, `length`, `delete`, `mkdir`, `mkdirs`
- Lambda expressions: `(params) -> { body }` → `|params| { body }` closures

### Stage 8: Build integration and tooling

- `jtrans translate` subcommand with `--input`, `--output`, `--classpath`, `--watch`, `--print`, `--dump-ir`
- Incremental translation cache: SHA-256 hashing with `.jtrans-cache` file, skip unchanged files
- `--watch` mode: filesystem monitoring via `notify` crate, auto re-translate on `.java` file changes
- Source map generation: `.jtrans-map` files mapping Rust output lines back to Java source lines
- Cargo.toml auto-generation in the output directory with `java-compat` dependency
- Maven integration: `jtrans init-maven` generates an `exec-maven-plugin` fragment
- Gradle integration: `jtrans init-gradle` generates a Kotlin DSL build script with `translateToRust` task
- Recursive directory input: `--input src/` discovers all `.java` files
- Legacy positional CLI mode preserved for backwards compatibility

## Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) -- IR design, pass ordering, codegen strategy, and testing approach
- [TRANSLATION_REFERENCE.md](TRANSLATION_REFERENCE.md) -- every supported Java construct and its Rust equivalent
- [LIMITATIONS.md](LIMITATIONS.md) -- unsupported Java features and known gaps
- [PROJECT_PLAN.md](PROJECT_PLAN.md) -- staged delivery plan with task checklists

### Rustdoc

Generate API documentation for the `java-compat` runtime crate:

```bash
cargo doc -p java-compat --no-deps --open
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for branching strategy, commit conventions, and coding guidelines.

## License

Licensed under the [GNU General Public License v3.0](LICENSE).

