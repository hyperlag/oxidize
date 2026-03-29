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
| `tests` | Differential test suite (89 tests: translated Rust output vs. expected output) |

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
the tests. The suite currently contains **93 differential tests**:

```bash
cargo test -p tests -- --test-threads=4
```

## Supported Features

### Core Language

- Primitive types: `int`, `long`, `double`, `float`, `boolean`, `char`, `byte`, `short`
- `String` literals and concatenation (including mixed primitive + String)
- All arithmetic, bitwise, comparison, and logical operators
- Compound assignment, pre/post-increment/decrement
- Control flow: `if/else`, `while`, `do-while`, `for`, `break`, `continue`, ternary
- Static methods with recursion
- Single-dimensional arrays
- `System.out.println` / `System.out.print` / `System.err.println`

### Object-Oriented

- Classes with fields, methods, and constructors
- Single inheritance (`extends`) with `super` delegation
- Interfaces (`implements`) with trait mapping
- Method overriding, `this`/`super` references, `instanceof`
- `toString()` → `Display`, `equals()`, `hashCode()`, `getClass()`
- `@Override` and `@Deprecated` annotations (silently tolerated)
- Generic classes with `Clone + Default + Debug` bounds

### Enums

- Simple enums and enums with fields/constructors/methods
- Built-in methods: `name()`, `ordinal()`, `values()`, `valueOf()`, `equals()`
- Enum switch statements, equality via `==` and `.equals()`
- `EnumMap<K,V>` and `EnumSet<T>` collections

### Collections

- `ArrayList` / `LinkedList` / `ArrayDeque` / `PriorityQueue` → `JList<T>` and friends
- `HashMap` / `TreeMap` / `LinkedHashMap` / `EnumMap` → `JMap<K,V>` / `JEnumMap<K,V>`
- `HashSet` / `TreeSet` / `LinkedHashSet` / `EnumSet` → `JSet<T>` / `JEnumSet<T>`
- `Collections.sort()`, `Collections.reverse()`, `Collections.unmodifiableList/Map/Set()`, `Collections.emptyList/Map/Set()`, `Collections.singletonList()`, `Arrays.asList()`
- `Iterator` with `hasNext()`/`next()`/`remove()`

### Exception Handling

- `throw` / `try` / `catch` / `finally` / multi-catch / nested try blocks
- Try-with-resources (desugared to `finally` + `close()`)
- `throws` declarations; exceptions propagate via panics

### Concurrency

- `Thread` creation, `.start()`, `.join()`, `Thread.sleep()`
- `synchronized` methods and blocks
- `wait()` / `notify()` / `notifyAll()`
- `volatile` fields → atomic types with `SeqCst` ordering
- `AtomicInteger` / `AtomicLong` / `AtomicBoolean`
- `CountDownLatch`, `Semaphore`

### Standard Library

- `Math` static methods, `StringBuilder`, `Optional<T>`, `Stream<T>` API
- `Pattern` / `Matcher` regex, `BigInteger`, `LocalDate`, `File`
- Lambda expressions → Rust closures

### I/O and NIO

- `FileReader`, `FileWriter`, `BufferedReader`, `BufferedWriter`, `PrintWriter`
- `FileInputStream`, `FileOutputStream`
- `Scanner` (from `File` or `String`: `nextLine`, `next`, `nextInt`, `nextDouble`, `nextLong`, `hasNextLine`, `hasNext`, `hasNextInt`)
- `java.nio.file.Path`, `Paths.get()`, `Files` (`readString`, `writeString`, `readAllLines`, `write`, `exists`, `isDirectory`, `isRegularFile`, `size`, `delete`, `deleteIfExists`, `createDirectory`, `createDirectories`, `copy`, `move`)

### Java to Rust Type Mapping

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
| `EnumMap<K,V>` | `java_compat::JEnumMap<K,V>` |
| `EnumSet<T>` | `java_compat::JEnumSet<T>` |
| `Optional<T>` | `java_compat::JOptional<T>` |
| `Stream<T>` | `java_compat::JStream<T>` |
| `StringBuilder` | `java_compat::JStringBuilder` |
| `BigInteger` | `java_compat::JBigInteger` |
| `Pattern` / `Matcher` | `java_compat::JPattern` / `JMatcher` |
| `LocalDate` | `java_compat::JLocalDate` |
| `File` | `java_compat::JFile` |
| `FileReader` | `java_compat::JFileReader` |
| `FileWriter` | `java_compat::JFileWriter` |
| `BufferedReader` | `java_compat::JBufferedReader` |
| `BufferedWriter` | `java_compat::JBufferedWriter` |
| `PrintWriter` | `java_compat::JPrintWriter` |
| `FileInputStream` | `java_compat::JFileInputStream` |
| `FileOutputStream` | `java_compat::JFileOutputStream` |
| `Scanner` | `java_compat::JScanner` |
| `Path` | `java_compat::JPath` |
| `Files` | `java_compat::JFiles` |
| `AtomicInteger` | `java_compat::JAtomicInteger` |
| `CountDownLatch` | `java_compat::JCountDownLatch` |
| `Semaphore` | `java_compat::JSemaphore` |
| `Thread` | `java_compat::JThread` |
| `ClassName` | `ClassName` (generated struct) |
| `enum EnumName` | `enum EnumName` (generated Rust enum) |

## Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) -- IR design, pass ordering, codegen strategy, and testing approach
- [TRANSLATION_REFERENCE.md](TRANSLATION_REFERENCE.md) -- every supported Java construct and its Rust equivalent
- [LIMITATIONS.md](LIMITATIONS.md) -- unsupported Java features and known gaps

### Rustdoc

Generate API documentation for the `java-compat` runtime crate:

```bash
cargo doc -p java-compat --no-deps --open
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for branching strategy, commit conventions, and coding guidelines.

## License

Licensed under the [GNU General Public License v3.0](LICENSE).

