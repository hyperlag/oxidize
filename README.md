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
| `cli` | `jtrans` binary: CLI driver with `translate`, `scan`, `init-maven`, `init-gradle` subcommands, watch mode, incremental cache, and source map generation |
| `tests` | Differential test suite (181 tests: translated Rust output vs. expected output) |

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

### `jtrans scan` — pre-flight compatibility check

Analyse a Java project for patterns that `jtrans` cannot translate before
attempting a full conversion. Reports blocking errors (reflection, native
methods, unsupported syntax) and warnings (Spring annotations, serialization)
per file, then prints a project-level summary.

```
jtrans scan [OPTIONS] --input <INPUT>

Options:
  -i, --input <INPUT>  Input directory or file(s) to scan [required]
      --issues-only    Suppress the ✓ lines; show only files with problems
      --strict         Exit with code 1 if any blocking errors are found
```

#### Scan a whole Maven source tree

```bash
jtrans scan --input src/main/java/
```

Example output:

```
Scanning 42 Java files…

  ✓  src/main/java/com/example/App.java
  ✗  src/main/java/com/example/Loader.java  [2 errors]
       line 4: [error:reflection-import] java.lang.reflect import (reflection not supported)
       line 12: [error:reflection-class-for-name] Class.forName() — dynamic class loading not supported
  ⚠  src/main/java/com/example/Dao.java  [1 warning]
       line 1: [warning:spring-annotations] Spring/JPA annotation — framework injection will NOT work after translation
  ✓  ...

══════════════════════════════════════════════════
Summary
══════════════════════════════════════════════════
  Files scanned            : 42
  Files fully compatible   : 39
  Files with warnings only : 1
  Files with errors        : 2

  Total errors             : 2
  Total warnings           : 1

  Issue breakdown:
     1×  [error:reflection-class-for-name]
     1×  [error:reflection-import]
     1×  [warning:spring-annotations]

2 file(s) have blocking errors that must be addressed before translation.
```

#### CI pre-flight gate

```bash
# Fail CI if any file has blocking errors
jtrans scan --input src/main/java/ --strict
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
the tests. The suite currently contains **181 differential tests**:

```bash
cargo test -p tests -- --test-threads=4
```

## Supported Features

### Core Language

- Primitive types: `int`, `long`, `double`, `float`, `boolean`, `char`, `byte`, `short`
- `String` literals and concatenation (including mixed primitive + String)
- All arithmetic, bitwise, comparison, and logical operators
- Compound assignment, pre/post-increment/decrement
- Control flow: `if/else`, `while`, `do-while`, `for`, `break`, `continue`, ternary, labeled `break`/`continue`
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
- Bounded type parameters: `<T extends Comparable<T>>` → `PartialOrd + Ord`
- Multiple bounds: `<T extends Number & Comparable<T>>`
- Wildcard types: `<?>`, `<? extends T>`, `<? super T>` (erased to bounds)
- Raw types: bare `List`, `Map`, `Set` (mapped with `JavaObject` defaults)

### Enums

- Simple enums and enums with fields/constructors/methods
- Multiple enum constructors (including overloads by arity and parameter type)
- Built-in methods: `name()`, `ordinal()`, `values()`, `valueOf()`, `equals()`
- Enum switch statements, equality via `==` and `.equals()`
- `EnumMap<K,V>` and `EnumSet<T>` collections
- Enums implementing interfaces (generates `impl Trait for Enum`)
- Constant-specific class bodies (per-variant method overrides via `match` dispatch)

### Collections

- `ArrayList` / `LinkedList` / `ArrayDeque` / `PriorityQueue` → `JList<T>` and friends
- `HashMap` / `TreeMap` / `LinkedHashMap` / `EnumMap` → `JMap<K,V>` / `JEnumMap<K,V>`
- `HashSet` / `TreeSet` / `LinkedHashSet` / `EnumSet` → `JSet<T>` / `JEnumSet<T>`
- `Collections.sort()`, `Collections.reverse()`, `Collections.unmodifiableList/Map/Set()`, `Collections.emptyList/Map/Set()`, `Collections.singletonList()`, `Arrays.asList()`
- `Iterator` with `hasNext()`/`next()`/`remove()`
- Map `keySet()`/`values()`/`entrySet()` iteration with `JMapEntry<K,V>`
- Map mutation API: `putIfAbsent()`, `computeIfAbsent()`, `compute()`, `merge()`, `forEach()`, `replace()`, `replaceAll()`
- Immutable factory methods: `List.of()`, `Set.of()`, `Map.of()`, `Map.entry()`, `Map.ofEntries()`, `List.copyOf()`, `Set.copyOf()`, `Map.copyOf()`
- `Spliterator` stub (`trySplit`, `estimateSize`, `forEachRemaining`, `tryAdvance`, `characteristics()`)

### Exception Handling

- `throw` / `try` / `catch` / `finally` / multi-catch / nested try blocks
- Try-with-resources (desugared to `finally` + `close()`)
- `throws` declarations; exceptions propagate via panics

### Concurrency

- `Thread` creation, `.start()`, `.join()`, `Thread.sleep()`
- `synchronized` methods and `synchronized(obj)` blocks (per-object monitors)
- `wait()` / `notify()` / `notifyAll()` (unqualified and `this.wait()`/`this.notify()` forms)
- `volatile` fields → atomic types with `SeqCst` ordering
- `AtomicInteger` / `AtomicLong` / `AtomicBoolean`
- `CountDownLatch`, `Semaphore`
- `ReentrantLock` / `Condition`, `ReentrantReadWriteLock` / `ReadLock` / `WriteLock`
- `StampedLock` (writeLock, readLock, tryOptimisticRead, validate)
- `ConcurrentHashMap`, `CopyOnWriteArrayList`
- `ThreadLocal` (with `withInitial`)
- `ExecutorService` / `Executors` (thread pools, execute, submit, shutdown)
- `Future`, `CompletableFuture` (supplyAsync, thenApply, join)
- `TimeUnit`
- `ForkJoinPool` / `RecursiveTask<T>` / `RecursiveAction` (fork, join, invoke)

### Standard Library

- `Math` static methods, `StringBuilder`, `Optional<T>`, `Stream<T>` API
- `Pattern` / `Matcher` regex, `BigInteger`, `BigDecimal`, `MathContext`, `LocalDate`, `File`
- `LocalTime`, `LocalDateTime`, `Instant`, `Duration`, `Period`, `DateTimeFormatter`
- `ZonedDateTime`, `ZoneId`, `Clock`
- `Properties` (`load_string`, `getProperty`, `getProperty` with default, `setProperty`, `stringPropertyNames`, `containsKey`, `size`, `isEmpty`)
- `Timer` / `TimerTask` (one-shot and repeating scheduled tasks)
- `String.format()`, `String.join()`, `System.out.printf()`
- `System.exit()`, `System.currentTimeMillis()`, `System.nanoTime()`, `System.getenv()`, `System.getProperty()`, `System.lineSeparator()`
- Lambda expressions → Rust closures (including multi-statement block bodies)
- Method references: static (`Class::method`), constructor (`Class::new`), bound instance (`obj::method`, `System.out::println`), multi-argument (`Integer::sum`, `Math::max`, user-defined binary method refs)
- Text blocks (Java 13+ `"""..."""`) with indent stripping per JEP 378
- Switch expressions (Java 14+) with arrow syntax, multi-label arms, and `yield` blocks
- Pattern switch expressions (Java 21+): `switch(obj) { case Type binding -> expr; }` used as a value; emits Rust if-else chain block
- Multiple classes per file: package-private helper classes alongside the public class; cross-class static method calls (`Helper.method(args)` → `Helper::method(args)`)
- Local class variable capture: named classes declared inside method bodies can reference effectively-final locals from the enclosing scope (hoisted as `__cap_X` fields, mirroring anonymous class capture)

### Networking

- `URL` (parsing, component accessors)
- `Socket` / `ServerSocket` (TCP client/server)
- `HttpURLConnection` (basic HTTP/1.1 GET/POST)
- `HttpClient` / `HttpRequest` / `HttpResponse` (Java 11+ HTTP client)

### I/O and NIO

- `FileReader`, `FileWriter`, `BufferedReader`, `BufferedWriter`, `PrintWriter`
- `FileInputStream`, `FileOutputStream`
- Abstract I/O base types as polymorphic enums: `InputStream`, `OutputStream`, `Reader`, `Writer`
- `BufferedReader(new StringReader(...))` wrapping
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
| `BigDecimal` | `java_compat::JBigDecimal` |
| `MathContext` | `java_compat::JMathContext` |
| `RoundingMode` | `java_compat::JRoundingMode` |
| `URL` | `java_compat::JURL` |
| `Socket` | `java_compat::JSocket` |
| `ServerSocket` | `java_compat::JServerSocket` |
| `HttpURLConnection` | `java_compat::JHttpURLConnection` |
| `HttpClient` | `java_compat::JHttpClient` |
| `HttpRequest` | `java_compat::JHttpRequest` |
| `HttpResponse<T>` | `java_compat::JHttpResponse` |
| `ZonedDateTime` | `java_compat::JZonedDateTime` |
| `ZoneId` | `java_compat::JZoneId` |
| `Clock` | `java_compat::JClock` |
| `Properties` | `java_compat::JProperties` |
| `Timer` | `java_compat::JTimer` |
| `TimerTask` | `java_compat::JTimerTask` |
| `Pattern` / `Matcher` | `java_compat::JPattern` / `JMatcher` |
| `LocalDate` | `java_compat::JLocalDate` |
| `LocalTime` | `java_compat::JLocalTime` |
| `LocalDateTime` | `java_compat::JLocalDateTime` |
| `Instant` | `java_compat::JInstant` |
| `Duration` | `java_compat::JDuration` |
| `Period` | `java_compat::JPeriod` |
| `DateTimeFormatter` | `java_compat::JDateTimeFormatter` |
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

