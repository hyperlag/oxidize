# oxidize

A source-to-source translator that ingests Java code and produces idiomatic, memory-safe Rust.

## Goals

- Translate arbitrary Java programs to Rust that compiles without `unsafe` blocks
- Preserve functional equivalence — translated programs pass the original Java test suite
- Emit clean, formatted Rust (`rustfmt` + `clippy`-clean output)

## Architecture

```
Java source (.java)
      │
  Java Parser          (tree-sitter-java)
      │  Java AST
  Type Resolver        (symbol table, type inference)
      │  Typed IR
  IR Lowering          (reflection, generics, lambdas)
      │  Normalised IR
  Rust Codegen         (trait mapping, lifetime elision)
      │  Rust tokens
  Post-process         (rustfmt, clippy auto-fix)
      │
Rust source (.rs) + Cargo.toml
```

## Crates

| Crate | Purpose |
|---|---|
| `parser` | Wraps `tree-sitter-java`; parses `.java` source into a typed IR |
| `ir` | Core intermediate representation — `IrType`, `IrExpr`, `IrStmt`, `IrDecl` |
| `typeck` | Type-checking and symbol-resolution pass over the IR |
| `codegen` | Lowers normalised IR to Rust token streams via `proc-macro2` / `quote` |
| `runtime` | `java-compat` crate: `JObject`, `JString`, `JArray<T>`, collection wrappers |
| `cli` | `jtrans` binary — command-line entry point |
| `tests` | Differential test suite (Java output vs. translated Rust output) |

## Requirements

- Rust stable toolchain (`rustup` recommended)
- Java 17+ (for running differential tests)

## Building

```bash
cargo build --release
```

The `jtrans` binary will be placed at `target/release/jtrans`. You can also run it directly without installing:

```bash
cargo run -p jtrans -- [OPTIONS] <file.java>
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

This writes `out/helloworld.rs` — a self-contained Rust source file that depends only on the
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

Use `--dump-ir` to print the typed intermediate representation as JSON — useful when
diagnosing translation problems:

```bash
jtrans --dump-ir HelloWorld.java
```

## Running Tests

Build and run the full workspace unit tests:

```bash
cargo test
```

The differential integration tests in `crates/tests` compile and run each Java program
with `javac`/`java`, then compile and run the translated Rust, and assert the stdout
matches exactly. They require a JDK on `PATH`:

```bash
# Run only the differential tests, one at a time to keep system load low
cargo test -p tests --test-threads=1
```

## Project Status

The project follows a staged delivery plan:

| Stage | Description | Status |
|---|---|---|
| 0 | Foundation & tooling — workspace, CI, tree-sitter smoke test | ✅ Complete |
| 1 | Core language — primitives, control flow, static methods, arrays | ✅ Complete (32/32 differential tests pass) |
| 2 | Object-oriented core — classes, inheritance, interfaces | Planned |
| 3 | Generics & collections | Planned |
| 4 | Concurrency — `synchronized`, `Thread`, `java.util.concurrent` | Planned |
| 5 | Reflection & dynamic dispatch | Planned |

### Stage 1 — Supported Java features

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

### Java → Rust type mapping

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
| `String` | `java_compat::JString` |
| `T[]` | `java_compat::JArray<T>` |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for branching strategy, commit conventions, and coding guidelines.

## License

Licensed under the [GNU General Public License v3.0](LICENSE).

