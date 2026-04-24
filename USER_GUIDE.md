# oxidize User Guide

This guide covers everything you need to translate Java projects to native
executables using `jtrans`, the oxidize command-line tool.

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Installation](#2-installation)
3. [Quick Start](#3-quick-start)
4. [CLI Reference](#4-cli-reference)
5. [Pre-flight Compatibility Scan](#5-pre-flight-compatibility-scan)
6. [Maven Project to Native Executable](#6-maven-project-to-native-executable)
7. [Gradle Integration](#7-gradle-integration)
8. [Incremental Translation](#8-incremental-translation)
9. [Watch Mode](#9-watch-mode)
10. [Source Maps](#10-source-maps)
11. [The java-compat Runtime](#11-the-java-compat-runtime)
12. [Supported Java Features](#12-supported-java-features)
13. [Known Limitations](#13-known-limitations)
14. [Troubleshooting](#14-troubleshooting)
15. [Architecture Overview](#15-architecture-overview)

---

## 1. Introduction

**oxidize** is a source-to-source translator that converts Java source code
into idiomatic, memory-safe Rust. The resulting Rust code compiles to a native
binary with no JVM dependency — no Java runtime, no garbage collector, no
class files. The translated binary links against `java-compat`, a small Rust
crate that provides Java-compatible collection types, I/O wrappers, and
concurrency primitives.

**When to use oxidize:**

- You have a self-contained Java application (utilities, CLIs, batch jobs)
  and want a single native binary with fast startup and low memory use.
- You want to migrate a Java codebase to Rust incrementally, class by class.
- You want to eliminate JVM infrastructure from a deployment.

**When oxidize is not a good fit:**

- Applications that rely on runtime reflection, dynamic class loading, or JNI.
- Heavy framework code (Spring, Hibernate, JPA) that is annotation-driven.
- Programs that depend on Java modules (`module-info.java`).

See [Section 13: Known Limitations](#13-known-limitations) for the full list.

---

## 2. Installation

### Prerequisites

- **Rust stable toolchain** — install via [rustup](https://rustup.rs):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  source ~/.cargo/env
  ```

### Build from source

```bash
git clone https://github.com/your-org/oxidize.git
cd oxidize
cargo build --release
```

The `jtrans` binary is placed at `target/release/jtrans`.

### Add to PATH

```bash
# Linux / macOS
export PATH="$PATH:/path/to/oxidize/target/release"

# Or create a symlink
ln -s /path/to/oxidize/target/release/jtrans ~/.local/bin/jtrans
```

### Verify the installation

```bash
jtrans --version
jtrans --help
```

> **Note:** You do not need a JDK installed. `jtrans` uses its own embedded
> Java parser (tree-sitter-java) and does not invoke `javac` or the JVM.

---

## 3. Quick Start

### Translate a single Java file

Consider the classic example:

```java
// HelloWorld.java
public class HelloWorld {
    public static void main(String[] args) {
        System.out.println("Hello, World!");
    }
}
```

Translate it:

```bash
jtrans translate --input HelloWorld.java --output hello-rs/
```

This creates:

```
hello-rs/
  Cargo.toml
  src/
    helloworld.rs
    helloworld.jtrans-map
```

### Set up the runtime dependency

The generated `Cargo.toml` contains:

```toml
[dependencies]
java-compat = { path = "java-compat" }
```

You need to make `java-compat` available. The easiest way is to copy (or
symlink) the runtime crate from the oxidize source tree:

```bash
cp -r /path/to/oxidize/crates/runtime hello-rs/java-compat
```

### Build and run

```bash
cd hello-rs
cargo run
# → Hello, World!
```

To produce a release binary:

```bash
cargo build --release
./target/release/hello-rs
```

---

## 4. CLI Reference

All commands are invoked as `jtrans <subcommand> [OPTIONS]`.

### `jtrans translate`

The primary translation command.

```
jtrans translate [OPTIONS] --input <INPUT>

Options:
  -i, --input <INPUT>          Input directory or file(s) [required]
  -o, --output <OUTPUT>        Output directory [default: rust-out]
  -c, --classpath <CLASSPATH>  Classpath entries for type resolution
      --watch                  Watch input files and re-translate on changes
      --dump-ir                Print the IR as JSON after parsing (for debugging)
      --print                  Print generated Rust to stdout; do not write files
      --no-incremental         Disable incremental caching (translate all files every run)
      --no-source-map          Disable .jtrans-map source map generation
      --no-cargo-toml          Disable Cargo.toml generation in the output directory
```

#### Examples

**Translate a single file:**
```bash
jtrans translate --input MyApp.java
```

**Translate a source tree:**
```bash
jtrans translate --input src/main/java/ --output rust-out/
```

**Preview output without writing:**
```bash
jtrans translate --input MyApp.java --print
```

**Inspect the intermediate representation:**
```bash
jtrans translate --input MyApp.java --dump-ir
```

**Use a custom output directory:**
```bash
jtrans translate --input src/ --output /tmp/rust-preview/
```

**Force full retranslation (no caching):**
```bash
jtrans translate --input src/ --no-incremental
```

### `jtrans init-maven`

Generate a Maven plugin fragment to run `jtrans` as part of the Maven build:

```bash
jtrans init-maven --output .
```

Writes `jtrans-maven-plugin.xml`. See [Section 6](#6-maven-project-to-native-executable).

### `jtrans init-gradle`

Generate a Gradle build script fragment in Kotlin DSL:

```bash
jtrans init-gradle --output .
```

Writes `jtrans.gradle.kts`. See [Section 7](#7-gradle-integration).

### `jtrans scan`

Scan Java source files for compatibility issues before translation. See [Section
5](#5-pre-flight-compatibility-scan) for full details.

```
jtrans scan [OPTIONS] --input <INPUT>

Options:
  -i, --input <INPUT>  Input directory or file(s) to scan [required]
      --issues-only    Suppress ✓ lines; show only files that have issues
      --strict         Exit with code 1 if any blocking errors are found
```

### Legacy positional syntax

For backwards compatibility, a bare file argument is also accepted:

```bash
jtrans HelloWorld.java --output out/
```

---

## 5. Pre-flight Compatibility Scan

Before spending time on translation, run `jtrans scan` to identify Java
patterns that `jtrans` cannot handle. The scanner analyses every `.java` file
in two passes:

1. **Pattern pass** — line-by-line checks for known-bad constructs using fast
   regular expressions.
2. **Parser pass** — a full parse attempt; any file the parser rejects is
   flagged immediately.

Issues are classified as:

- **`error`** — will cause a parse or code-generation failure; the file cannot
  be translated as-is.
- **`warning`** — the file will translate, but the translated code may behave
  differently from the original Java (e.g., Spring annotations have no effect).

### Running a scan

```bash
# Scan an entire Maven source tree
jtrans scan --input src/main/java/

# Show only files with problems (skip the ✓ lines)
jtrans scan --input src/ --issues-only

# Exit with code 1 if any errors are found (CI gate)
jtrans scan --input src/ --strict
```

### Sample output

```
Scanning 42 Java files…

  ✓  src/main/java/com/example/App.java
  ✗  src/main/java/com/example/Loader.java  [2 errors]
       line 4: [error:reflection-import] java.lang.reflect import (reflection not supported)
       line 12: [error:reflection-class-for-name] Class.forName() — dynamic class loading not supported
  ⚠  src/main/java/com/example/Dao.java  [1 warning]
       line 1: [warning:spring-annotations] Spring/JPA annotation — framework injection will NOT work after translation

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

### Detected error codes

| Code | Meaning |
|---|---|
| `native-method` | `native` method declaration — JNI not supported |
| `reflection-import` | `import java.lang.reflect.*` |
| `reflection-class-for-name` | `Class.forName()` — dynamic class loading |
| `reflection-method-invoke` | `Method.invoke()` |
| `reflection-field-access` | `Field.get()` / `Field.set()` |
| `reflection-constructor` | `Constructor.newInstance()` |
| `reflection-declared-members` | `getDeclaredMethods/Fields/Constructors` |
| `reflection-set-accessible` | `setAccessible()` |
| `classloader` | `ClassLoader` usage |
| `load-library` | `System.loadLibrary()` / `System.load()` |
| `annotation-processing-import` | `javax.annotation.processing` |
| `rmi-import` | `java.rmi` |
| `nio-channels-import` | `java.nio.channels` |
| `object-streams` | `ObjectInputStream` / `ObjectOutputStream` |
| `instrument-import` | `java.lang.instrument` |
| `colon-form-pattern-switch` | `case Type var:` pattern label — use arrow form |
| `dynamic-proxy` | `Proxy.newProxyInstance()` |
| `module-info` | `module-info.java` file |
| `parse-error` | File rejected by the parser itself |

### Detected warning codes

| Code | Meaning |
|---|---|
| `serializable` | `implements Serializable` — compiles, but serialization has no effect |
| `spring-annotations` | Spring/JPA annotations (`@Autowired`, `@Entity`, etc.) — no injection |
| `runtime-getruntime` | `Runtime.getRuntime()` — only `.exec()` is supported |
| `externalizable` | `implements Externalizable` |

### Using scan in a CI pipeline

Add `jtrans scan --strict` as a pre-flight gate so translation failures surface
early:

```yaml
# GitHub Actions example
- name: jtrans compatibility scan
  run: jtrans scan --input src/main/java/ --strict
```

The exit code is `0` when no errors are found (warnings are permitted), and `1`
when `--strict` is set and at least one error is present.

---

## 6. Maven Project to Native Executable

This section walks through converting an existing Maven Java project into a
native binary. No JVM is required to run the result.

### Prerequisites

- `jtrans` is on your PATH (see [Section 2](#2-installation))
- Rust stable toolchain installed
- The `java-compat` crate is available (included in the oxidize source tree
  at `crates/runtime/`)

### Pre-translation checklist

Run the built-in compatibility scanner first — it is the fastest way to find
blockers:

```bash
jtrans scan --input src/main/java/ --strict
```

See [Section 5](#5-pre-flight-compatibility-scan) for a full description of
all issue codes and CI integration. Common blockers in Maven projects:

| Pattern | Action |
|---|---|
| Spring / Hibernate / JPA annotations | Refactor or stub out |
| `java.lang.reflect.*` usage | Remove or replace with compile-time alternatives |
| `native` methods | out of scope; must be rewritten |
| Third-party library calls | Replace with standard-library equivalents or stubs |
| `module-info.java` | Delete it — `jtrans` ignores module declarations |
| `ClassLoader.loadClass()` | Not supported; refactor to avoid dynamic loading |

### Step 1: Understand your project layout

A typical Maven project looks like this:

```
my-app/
  pom.xml
  src/
    main/
      java/
        com/example/
          App.java
          util/
            MathHelper.java
            StringUtils.java
    test/
      java/
        com/example/
          AppTest.java
```

Identify:
- The **source root**: `src/main/java/`
- The **main class** that contains `public static void main(String[] args)`
- Any **third-party dependencies** in `pom.xml` (see the note on classpath below)

### Step 2: Translate the source tree

From the project root, run:

```bash
jtrans translate \
  --input src/main/java/ \
  --output rust-out/
```

`jtrans` discovers all `.java` files recursively under `src/main/java/` and
translates them. The output is a complete Cargo project at `rust-out/`:

```
rust-out/
  Cargo.toml
  src/
    app.rs
    util/
      mathhelper.rs
      stringutils.rs
    *.jtrans-map   (source maps, one per .rs file)
```

### Step 3: Provide the java-compat runtime

The generated `Cargo.toml` depends on `java-compat` by path:

```toml
[dependencies]
java-compat = { path = "java-compat" }
```

Copy the runtime crate into your output directory:

```bash
cp -r /path/to/oxidize/crates/runtime rust-out/java-compat
```

Your output directory now looks like:

```
rust-out/
  Cargo.toml
  java-compat/       ← runtime crate
    Cargo.toml
    src/
  src/
    app.rs
    ...
```

### Step 4: Set the binary entry point

The generated `Cargo.toml` includes a `[[bin]]` section pointing at the main
class. Verify it matches your main class:

```toml
[[bin]]
name = "my-app"
path = "src/app.rs"
```

The translated `app.rs` will contain a `main()` function generated from the
Java `public static void main(String[] args)` method.

If your main class is in a subpackage, the `path` will reflect that, e.g.
`src/com/example/app.rs`.

### Step 5: Build the native binary

```bash
cd rust-out/
cargo build --release
```

Cargo compiles all translated Rust files and links them against `java-compat`.
The finished binary lands at:

```
rust-out/target/release/my-app
```

### Step 6: Run the binary

```bash
./target/release/my-app
```

The binary is fully self-contained — no JVM, no classpath, no `java` command.
You can copy it to any compatible Linux/macOS/Windows machine (same OS and
architecture) and run it directly.

### Step 7 (Optional): Automate with Maven

To run translation as part of your normal `mvn compile` cycle, generate the
Maven plugin fragment:

```bash
jtrans init-maven --output .
```

This writes `jtrans-maven-plugin.xml`. Open it and copy its contents into the
`<build><plugins>` section of your `pom.xml`. Then:

```bash
mvn compile exec:exec@jtrans
```

This calls `jtrans translate` automatically each time Maven compiles.

### Notes on third-party dependencies

`jtrans` translates Java source code, not bytecode. If your project uses
third-party libraries from Maven Central (e.g., Guava, Apache Commons), those
libraries' source is not included in `src/main/java/` and will not be
translated.

**Strategies for dependencies:**

- **Utility methods:** If the library is small and you only use a few methods,
  consider replacing those calls with equivalent standard-library calls before
  translation.
- **Interfaces / adapters:** If you use a library for its interface contract
  (e.g., SLF4J logging), stub out the interface using the `java-compat` types
  or write a small Rust equivalent.
- **Heavy frameworks:** Spring, Hibernate, and similar annotation-driven
  frameworks are not supported. This workflow is intended for self-contained
  application logic, not framework-managed beans.

Use `--classpath` to tell `jtrans` about additional source roots if your
project spans multiple Maven modules:

```bash
jtrans translate \
  --input module-a/src/main/java/ \
  --input module-b/src/main/java/ \
  --classpath shared-lib/src/main/java/ \
  --output rust-out/
```

### End-to-end example

Here is a minimal self-contained example. Given:

```java
// src/main/java/Greeter.java
public class Greeter {
    private String name;

    public Greeter(String name) {
        this.name = name;
    }

    public void greet() {
        System.out.println("Hello, " + name + "!");
    }

    public static void main(String[] args) {
        Greeter g = new Greeter(args.length > 0 ? args[0] : "World");
        g.greet();
    }
}
```

```bash
# Translate
jtrans translate --input src/main/java/ --output rust-out/

# Add runtime
cp -r /path/to/oxidize/crates/runtime rust-out/java-compat

# Build
cd rust-out && cargo build --release

# Run
./target/release/rust-out          # → Hello, World!
./target/release/rust-out Alice    # → Hello, Alice!
```

### Step 8 (Optional): Produce a portable, stripped binary

By default, `cargo build --release` produces a reasonably optimised binary, but
it still contains debug symbols. To reduce size and produce a self-contained
file ready for distribution:

```bash
cd rust-out/

# Build with link-time optimisation
RUSTFLAGS="-C lto=thin" cargo build --release

# Strip debug symbols (Linux / macOS)
strip target/release/<binary-name>
```

The resulting file is typically 60–80 % smaller than the unstripped build and
has no runtime dependencies on the JVM.

### Step 9 (Optional): Static linking with musl (Linux)

For a truly portable Linux binary that runs on any Linux distribution without
shared-library dependencies, compile against the `musl` libc target:

```bash
# Install the musl target once
rustup target add x86_64-unknown-linux-musl

# Build a statically linked binary
cd rust-out/
cargo build --release --target x86_64-unknown-linux-musl

# Strip it
strip target/x86_64-unknown-linux-musl/release/<binary-name>
```

The resulting binary links everything statically, including libc. You can copy
it to any `x86_64` Linux machine and run it directly.

### Step 10 (Optional): Rename the binary

By default the binary name in the generated `Cargo.toml` is derived from the
main Java class file. To use a custom name, edit `Cargo.toml` in the output
directory:

```toml
[[bin]]
name = "my-tool"          # ← change this to whatever you want
path = "src/greeter.rs"
```

Then rebuild:

```bash
cargo build --release
# binary is now at target/release/my-tool
```

---

## 7. Gradle Integration

Generate a Kotlin DSL fragment:

```bash
jtrans init-gradle --output .
```

Apply it in your `build.gradle.kts`:

```kotlin
apply(from = "jtrans.gradle.kts")
```

Then translate as part of your Gradle build:

```bash
./gradlew translateToRust
```

The generated task runs `jtrans translate --input src/main/java/` and places
the output in `rust-out/` by default.

---

## 8. Incremental Translation

By default, `jtrans` maintains a `.jtrans-cache` file in the output directory.
This file stores SHA-256 hashes of each input file. On subsequent runs, only
files whose hashes have changed are retranslated:

```bash
# First run: all files translated
jtrans translate --input src/ --output rust-out/

# Second run: skips unchanged files
jtrans translate --input src/ --output rust-out/
# → [cache] Skipping src/Foo.java (unchanged)
# → [translate] src/Bar.java
```

To disable caching and force a full retranslation:

```bash
jtrans translate --input src/ --output rust-out/ --no-incremental
```

To start fresh, delete the cache file:

```bash
rm rust-out/.jtrans-cache
```

---

## 9. Watch Mode

Use `--watch` to keep `jtrans` running and re-translate automatically whenever
a source file changes. This is useful during active development:

```bash
jtrans translate --input src/ --output rust-out/ --watch
```

`jtrans` uses filesystem notifications (inotify on Linux, FSEvents on macOS)
for efficient change detection. Only the modified file is retranslated.

Press **Ctrl+C** to stop.

Combine with `cargo watch` to also rebuild on translation:

```bash
# Terminal 1: translate on Java changes
jtrans translate --input src/ --output rust-out/ --watch

# Terminal 2: build on Rust changes
cd rust-out && cargo watch -x build
```

---

## 10. Source Maps

Every translated `.rs` file is accompanied by a `.jtrans-map` file that records
the Rust line → Java line correspondence:

```
# jtrans source map v1
# rust_line -> java_line
1 -> 0
5 -> 3
6 -> 4
12 -> 10
```

This is useful for mapping a Rust compiler error or panic back to the original
Java source line.

Disable source map generation with `--no-source-map`.

---

## 11. The java-compat Runtime

All translated programs link against `java-compat`, the oxidize runtime crate.
It provides Rust types that mirror Java's standard library behaviour.

### Key types

| Java type | java-compat type |
|---|---|
| `String` | `JString` |
| `T[]` | `JArray<T>` |
| `ArrayList<T>` / `List<T>` | `JList<T>` |
| `HashMap<K,V>` / `Map<K,V>` | `JMap<K,V>` |
| `HashSet<T>` / `Set<T>` | `JSet<T>` |
| `Optional<T>` | `JOptional<T>` |
| `Stream<T>` | `JStream<T>` |
| `StringBuilder` | `JStringBuilder` |
| `Thread` | `JThread` |
| `BigInteger` / `BigDecimal` | `JBigInteger` / `JBigDecimal` |
| `LocalDate`, `LocalTime`, `LocalDateTime` | `JLocalDate`, `JLocalTime`, `JLocalDateTime` |
| `File`, `Path`, `Files` | `JFile`, `JPath`, `JFiles` |
| `BufferedReader`, `PrintWriter`, etc. | `JBufferedReader`, `JPrintWriter`, etc. |
| `Socket`, `ServerSocket` | `JSocket`, `JServerSocket` |
| `Pattern`, `Matcher` | `JPattern`, `JMatcher` |

See the full type mapping table in [README.md](README.md) and the generated
rustdoc for method-level API documentation:

```bash
cargo doc -p java-compat --no-deps --open
```

### Path configuration

When you run `cargo build` in the generated output directory, Cargo needs to
find `java-compat`. The generated `Cargo.toml` uses a relative path:

```toml
[dependencies]
java-compat = { path = "java-compat" }
```

You have two options:

1. **Copy the crate** into the output directory (recommended for standalone projects):
   ```bash
   cp -r /path/to/oxidize/crates/runtime rust-out/java-compat
   ```

2. **Edit the path** in `Cargo.toml` to point at the oxidize source tree:
   ```toml
   java-compat = { path = "/path/to/oxidize/crates/runtime" }
   ```

---

## 12. Supported Java Features

### Core language

- All primitive types (`int`, `long`, `double`, `float`, `boolean`, `char`, `byte`, `short`)
- String literals and concatenation (including mixed primitive + String)
- All arithmetic, bitwise, comparison, and logical operators
- Compound assignment, pre/post-increment/decrement
- Control flow: `if/else`, `while`, `do-while`, `for`, `for-each`, `break`, `continue`, ternary, labeled `break`/`continue`
- Static methods with recursion
- Single-dimensional arrays
- `System.out.println`, `System.out.print`, `System.err.println`

### Object-oriented features

- Classes with fields, methods, and constructors
- Single inheritance (`extends`) with `super` delegation
- Interfaces (`implements`) with trait mapping
- Method overriding, `this`/`super` references, `instanceof`
- `toString()` → `Display`, `equals()`, `hashCode()`, `getClass()`
- Generic classes with bounded type parameters (`<T extends Comparable<T>>`)
- Multiple bounds (`<T extends Number & Comparable<T>>`)
- Wildcard types (`<?>`, `<? extends T>`, `<? super T>`) — erased to bounds
- Raw types (bare `List`, `Map`, `Set`)
- Records (Java 16+), sealed classes (Java 17+)
- Non-static inner classes, anonymous inner classes (with limitations)
  — including `OuterClass.this.field` / `OuterClass.this.method()` access
- `instanceof` pattern matching with binding variable (Java 16+)

### Enums

- Simple enums and enums with fields, constructors, and methods
- Built-in methods: `name()`, `ordinal()`, `values()`, `valueOf()`, `equals()`
- Enum switch statements, `EnumMap<K,V>`, `EnumSet<T>`
- Enums implementing interfaces, constant-specific class bodies

### Collections

`ArrayList`, `LinkedList`, `ArrayDeque`, `PriorityQueue`, `HashMap`, `TreeMap`,
`LinkedHashMap`, `EnumMap`, `HashSet`, `TreeSet`, `LinkedHashSet`, `EnumSet`,
`Iterator`, `Spliterator` (stub), `Collections` utility methods, `Arrays.asList()`,
map `keySet()`/`values()`/`entrySet()` iteration via `JMapEntry<K,V>`.

### Exception handling

`throw`, `try`/`catch`/`finally`, multi-catch, nested try blocks,
try-with-resources (desugared to `finally` + `close()`).

### Concurrency

`Thread`, `synchronized`, `volatile` (→ atomic), `AtomicInteger/Long/Boolean`,
`CountDownLatch`, `Semaphore`, `ReentrantLock`, `ReentrantReadWriteLock`,
`ConcurrentHashMap`, `CopyOnWriteArrayList`, `ThreadLocal`,
`ExecutorService`/`Executors`, `Future`, `CompletableFuture`, `TimeUnit`.

### Standard library

`Math`, `StringBuilder`, `Optional<T>`, `Stream<T>`, regex `Pattern`/`Matcher`,
`BigInteger`, `BigDecimal`, `MathContext`, `String.format()`, `String.join()`,
`System.out.printf()`, `System.exit/currentTimeMillis/nanoTime/getenv/getProperty`,
`LocalDate/Time/DateTime`, `Instant`, `Duration`, `Period`, `DateTimeFormatter`,
`ZonedDateTime`, `ZoneId`, `Clock`, `Properties`, `ResourceBundle`,
`ProcessBuilder`/`Process`, `Timer`/`TimerTask`, class literals (`Foo.class`),
lambda expressions, text blocks (Java 13+).

### I/O

`File`, `FileReader`, `FileWriter`, `BufferedReader`, `BufferedWriter`,
`PrintWriter`, `FileInputStream`, `FileOutputStream`, `Scanner`,
`StringWriter`, `StringReader`, `ByteArrayOutputStream`, `ByteArrayInputStream`,
`Path`, `Paths`, `Files` (read, write, copy, move, delete, create, exists, etc.).

### Networking

`URL`, `Socket`, `ServerSocket`, `HttpURLConnection`, `HttpClient`/`HttpRequest`/`HttpResponse` (Java 11+).

---

## 13. Known Limitations

### Not supported

| Feature | Reason |
|---|---|
| `java.lang.reflect` (Method.invoke, Field.get, Class.forName, etc.) | Incompatible with static Rust type system |
| Dynamic class loading (`ClassLoader.loadClass`) | Requires JVM-like runtime |
| `native` methods / JNI | Calls into C/C++ cannot be represented |
| Annotation processing / framework annotations | Spring, JPA, etc. not supported |
| `module-info.java` (Java 9+ modules) | Not parsed |
| `java.nio.channels` (NIO selectors) | Not implemented |
| Java serialization (`Serializable`, `ObjectInputStream`, `ObjectOutputStream`) | Not implemented |
| Colon-form pattern labels in switch statements (`case String s:`) | Use arrow form instead |
| Lambda closures sharing mutable state across executor tasks | Use `Runnable` implementations instead |

### Partially supported

| Feature | Limitation |
|---|---|
| Generic method type inference across call chains | May require explicit type annotations |
| Anonymous inner classes | Interface implementations only; captured outer-scope variables are cloned into `__cap_X` fields (effectively-final values only) |
| Local named classes in method bodies | Captures effectively-final locals via `__cap_X` fields, same as anonymous classes |
| Non-static inner classes | Outer reference is a snapshot clone taken at construction; mutations via inner class not reflected in original outer |
| `@Override`, `@Deprecated` | Tolerated syntactically; no code generation effect |
| Advanced enum features | Anonymous constant subclasses with fields not supported |
| `wait()`/`notify()` | Supported on `this` (in `synchronized` methods and `synchronized(this)` blocks); not supported when the monitor is a non-variable expression or a built-in type (String, array, collection) |

For more detail see [LIMITATIONS.md](LIMITATIONS.md).

---

## 14. Troubleshooting

### error: can't find crate for `java_compat`

The generated `Cargo.toml` has `java-compat = { path = "java-compat" }` but
the directory does not exist in your output folder.

**Fix:** Copy the runtime crate:
```bash
cp -r /path/to/oxidize/crates/runtime rust-out/java-compat
```

Or update the path in `Cargo.toml` to an absolute path:
```toml
java-compat = { path = "/path/to/oxidize/crates/runtime" }
```

### Translation produces a `TODO` stub

If `jtrans` encounters a Java pattern it does not support, it emits a
`todo!("...")` macro call:

```rust
fn some_method(&mut self) {
    todo!("unsupported: MethodInvoke on reflect")
}
```

**Fix:** Check [LIMITATIONS.md](LIMITATIONS.md). If the pattern is reflection,
dynamic dispatch, or another unsupported feature, you will need to refactor
the Java source to avoid it before retranslating.

### Rust compile errors after translation

The translated code may not compile if:

1. **Unsupported Java pattern**: An expression that `jtrans` lowered
   incorrectly. File an issue with the minimal Java reproducer.
2. **Type mismatch involving generics**: Advanced generic type inference may
   require manual annotations. Add explicit type parameters in the Rust output.
3. **Missing `use` import**: Some generated files may need additional `use`
   statements. These are typically quick fixes reported by the compiler.

Use `--dump-ir` to inspect the intermediate representation and narrow down
which part of the input is causing the problem:

```bash
jtrans translate --input Problem.java --dump-ir 2>&1 | less
```

### Panic at runtime: index out of bounds / unwrap on None

Java programs use exceptions for out-of-bounds and null pointer conditions.
The translated Rust code uses panics in the same situations (via `.unwrap()`,
`vec[i]`, etc.). If you see a panic at runtime:

1. Use the source map to find the original Java line.
2. Add explicit bounds/null checks to the Java source and retranslate.

### Watch mode does not pick up new files

`--watch` monitors files that existed at startup. If you add a new `.java`
file to the source tree, restart `jtrans --watch` to pick it up.

---

## 15. Architecture Overview

The translation pipeline has five stages:

```
Java source (.java)
       │
   Java Parser          (tree-sitter-java)
       │  Java CST
   IR Lowering          (walker → typed IR)
       │  IrModule  (IrClass / IrMethod / IrExpr / IrStmt)
   Type Checker         (symbol table, inheritance resolution)
       │  Annotated IrModule
   Rust Codegen         (proc-macro2 / quote)
       │  Rust token stream
   Post-process         (prettyplease)
       │
Rust source (.rs)
```

### Crates

| Crate | Role |
|---|---|
| `parser` | Wraps `tree-sitter-java`; converts the concrete syntax tree into typed IR nodes |
| `ir` | Defines all IR types: `IrModule`, `IrClass`, `IrMethod`, `IrExpr`, `IrStmt`, `IrType` |
| `typeck` | Walks the IR, fills in `ty: IrType` on every expression, resolves symbols and inheritance |
| `codegen` | Converts the annotated IR into Rust token streams using `proc-macro2` and `quote!` |
| `runtime` | `java-compat` crate — Rust implementations of Java standard library types |
| `cli` | The `jtrans` binary — argument parsing, watch loop, incremental cache, source map writer |
| `tests` | 179 differential integration tests (translate + compile + compare stdout) |

### IR design

The intermediate representation mirrors Java's AST closely:

- **`IrModule`** — a flat list of declarations from one `.java` file
- **`IrClass`** — fields, methods, constructors, generics, inheritance
- **`IrMethod`** — parameters, body (`Vec<IrStmt>`), modifiers
- **`IrExpr`** — typed expression nodes (`LitInt`, `BinOp`, `MethodCall`, `New`, `Lambda`, …)
- **`IrStmt`** — statement nodes (`If`, `For`, `TryCatch`, `LocalVar`, …)
- **`IrType`** — `Bool | Int | Long | String | Class(name) | Generic { base, args } | …`

All IR types derive `serde::Serialize` and `serde::Deserialize`, so you can
dump the IR to JSON for debugging:

```bash
jtrans translate --input Foo.java --dump-ir
```

### Codegen strategy

- Java **interfaces** → Rust **traits**
- Java **classes** → Rust **structs** + `impl` blocks
- Java **inheritance** (`extends`) → struct composition via a `_super: Parent` field
- Java **interface implementation** (`implements`) → `impl Trait for Struct`
- Java **volatile fields** → `Arc<Mutex<T>>` / atomic types with `SeqCst` ordering
- Java **synchronized methods** → mutex-guarded method bodies
- Java **generics** → Rust generic parameters with trait bounds

For the complete translation reference — every supported Java construct mapped
to its Rust equivalent — see [TRANSLATION_REFERENCE.md](TRANSLATION_REFERENCE.md).
