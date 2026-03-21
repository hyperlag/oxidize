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
cargo build
```

## Usage

```bash
jtrans <input.java> [--output <dir>] [--dump-ir]
```

| Flag | Description |
|---|---|
| `<input.java>` | One or more Java source files to translate |
| `-o`, `--output` | Output directory for the generated Rust crate (default: `out/`) |
| `--dump-ir` | Print the IR as JSON after parsing (debug aid) |

**Example:**

```bash
jtrans HelloWorld.java --output hello_rs
cd hello_rs && cargo run
```

## Running Tests

```bash
cargo test
```

For the differential test suite (requires Java on `PATH`):

```bash
cargo nextest run -p tests
```

## Project Status

The project follows a staged delivery plan:

| Stage | Description | Status |
|---|---|---|
| 0 | Foundation & tooling — workspace, CI, tree-sitter smoke test | In progress |
| 1 | Core language — primitives, control flow, static methods | Planned |
| 2 | Object-oriented core — classes, inheritance, interfaces | Planned |
| 3 | Generics & collections | Planned |
| 4 | Concurrency — `synchronized`, `Thread`, `java.util.concurrent` | Planned |
| 5 | Reflection & dynamic dispatch | Planned |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for branching strategy, commit conventions, and coding guidelines.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.
