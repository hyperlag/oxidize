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
| `runtime` | `java-compat` crate: `JObject`, `JString`, `JArray<T>`, `JList<T>`, `JMap<K,V>`, `JSet<T>`, `JException` |
| `cli` | `jtrans` binary: command-line entry point |
| `tests` | Differential test suite (translated Rust output vs. expected output) |

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

```
jtrans [OPTIONS] <INPUTS>...

Arguments:
  <INPUTS>...  Java source file(s) to translate

Options:
  -o, --output <OUTPUT>  Output directory for generated Rust source [default: out]
      --print            Print generated Rust to stdout instead of writing to disk
      --dump-ir          Print the IR as JSON after parsing (debugging aid)
  -h, --help             Print help
  -V, --version          Print version
```

### Translate a single file

```bash
jtrans HelloWorld.java
```

This writes `out/helloworld.rs`: a self-contained Rust source file that depends only on the
`java-compat` runtime crate (part of this repository).

### Build and run the translated program

The generated file is not a full Cargo project on its own. The easiest way to run it
is to create a minimal wrapper:

```bash
# 1. Translate
jtrans HelloWorld.java --output out/

# 2. Create a Cargo project pointing at java-compat
mkdir -p hello-rs/src
cp out/helloworld.rs hello-rs/src/main.rs
cat > hello-rs/Cargo.toml << 'EOF'
[package]
name = "hello-rs"
version = "0.1.0"
edition = "2021"

[dependencies]
java-compat = { path = "/path/to/oxidize/crates/runtime" }
EOF

# 3. Run
cd hello-rs && cargo run
```

### Preview without writing files

Use `--print` to inspect the generated Rust without touching disk:

```bash
jtrans --print HelloWorld.java
```

### Translate multiple files at once

```bash
jtrans Foo.java Bar.java Baz.java --output out/
```

### Debug the IR

Use `--dump-ir` to print the typed intermediate representation as JSON, useful when
diagnosing translation problems:

```bash
jtrans --dump-ir HelloWorld.java
```

## Running Tests

Build and run the full workspace unit tests:

```bash
cargo test
```

The differential integration tests in `crates/tests` compile and run each translated Rust
program, then assert that stdout matches the expected output. No JDK is required to run
the tests. The suite currently contains **54 differential tests** covering Stages 1-5:

```bash
cargo test -p tests --test-threads=1
```

## Project Status

The project follows a staged delivery plan:

| Stage | Description | Status |
|---|---|---|
| 0 | Foundation and tooling: workspace, CI, tree-sitter smoke test | Complete |
| 1 | Core language: primitives, control flow, static methods, arrays | Complete (32/32 differential tests pass) |
| 2 | Object-oriented core: classes, inheritance, interfaces, `instanceof` | Complete (43/43 differential tests pass) |
| 3 | Generics and collections: `List`, `Map`, `Set`, generic classes | Complete (43/43 differential tests pass) |
| 4 | Exception handling: `try`/`catch`/`finally`/`throw`, multi-catch, try-with-resources, `throws` | Complete (49/49 differential tests pass) |
| 5 | Concurrency: `synchronized`, `Thread`, `java.util.concurrent` | Complete (54/54 differential tests pass) |
| 6 | Reflection and dynamic dispatch | Planned |
| 7 | Standard library coverage | Planned |

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
| `ClassName` | `ClassName` (generated struct) |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for branching strategy, commit conventions, and coding guidelines.

## License

Licensed under the [GNU General Public License v3.0](LICENSE).

