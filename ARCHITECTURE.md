# Architecture

This document describes how oxidize turns Java source code into Rust. It covers
the workspace layout, the compiler pipeline, and the design decisions behind each
pass.

## Workspace Layout

```
oxidize/
  crates/
    parser/      Java source -> typed IR (via tree-sitter-java)
    ir/          Shared intermediate representation (types, expressions, statements, declarations)
    typeck/      Type-checking and symbol-resolution pass
    codegen/     Typed IR -> Rust token stream (via proc-macro2 / quote)
    runtime/     java-compat crate: runtime types that translated programs depend on
    cli/         jtrans binary: CLI driver
    tests/       Differential integration test suite
```

Each crate has a single responsibility. Data flows in one direction through the
pipeline, and every crate boundary is defined by the IR types in `crates/ir`.

## Compiler Pipeline

```
  Java source (.java)
        |
   1. Parse              tree-sitter-java -> concrete syntax tree (CST)
        |
   2. Lower              CST walker -> IrModule (declarations, statements, expressions)
        |
   3. Type-check         Symbol resolution, type inference, annotation
        |
   4. Codegen            IrModule -> proc-macro2 TokenStream
        |
   5. Format             prettyplease -> final Rust source text
        |
   Rust source (.rs) + Cargo.toml
```

### 1. Parse (crates/parser)

The parser wraps `tree-sitter-java` (C library with Rust bindings) to produce a
concrete syntax tree from Java source text. tree-sitter is incremental and
error-tolerant, which means partial parses still produce usable nodes.

Entry points:

- `parse_source(source: &str) -> Result<Tree, ParseError>` -- parse Java text
  into a tree-sitter `Tree`. Returns `ParseError::SyntaxError` if any error
  nodes are found.
- `parse_to_ir(source: &str) -> Result<IrModule, ParseError>` -- full pipeline:
  parse, then walk the CST and lower every node into the typed IR.

The walker (`walker.rs`) traverses the CST depth-first and calls dedicated
lowering functions for each Java construct:

| CST node kind          | Lowering function    | IR output            |
|------------------------|----------------------|----------------------|
| `class_declaration`    | `lower_class()`      | `IrDecl::Class`      |
| `interface_declaration`| `lower_interface()`  | `IrDecl::Interface`  |
| `method_declaration`   | `lower_method()`     | `IrMethod`           |
| `constructor_declaration` | `lower_constructor()` | `IrConstructor`   |
| `field_declaration`    | `lower_field()`      | `IrField`            |
| Statement nodes        | `lower_stmt()`       | `IrStmt` variants    |
| Expression nodes       | `lower_expr()`       | `IrExpr` variants    |

Type conversion from tree-sitter node kinds to `IrType` is handled by helper
functions in `from_node.rs`.

### 2. Intermediate Representation (crates/ir)

The IR is a strongly-typed AST with four main categories:

**Module** (`IrModule`):
```
IrModule
  package: String
  imports: Vec<String>
  decls: Vec<IrDecl>
```

**Declarations** (`decl.rs`):
- `IrClass` -- name, visibility, abstract/final flags, type parameters,
  superclass, interfaces, fields, methods, constructors
- `IrInterface` -- name, visibility, type parameters, extended interfaces,
  method signatures
- `IrField` -- name, type, visibility, static/final/volatile flags, optional
  initializer
- `IrMethod` -- name, return type, parameters, body, modifiers (static,
  abstract, final, synchronized), throws declarations
- `IrConstructor` -- parameters, body, throws declarations

**Expressions** (`expr.rs`):

Every expression node carries a `ty: IrType` field that gets filled in by the
type checker. Expression variants include:

- Literals: `LitBool`, `LitInt`, `LitLong`, `LitFloat`, `LitDouble`,
  `LitChar`, `LitString`, `LitNull`
- Variables and field access: `Var`, `FieldAccess`
- Operators: `BinOp`, `UnOp`, `Ternary`
- Calls: `MethodCall`, `New`, `NewArray`
- Casts and tests: `Cast`, `InstanceOf`
- Arrays: `ArrayAccess`
- Assignments: `Assign`, `CompoundAssign`
- Lambdas: `Lambda`

**Statements** (`stmt.rs`):

- Control flow: `If`, `While`, `DoWhile`, `For`, `ForEach`, `Switch`,
  `Return`, `Break`, `Continue`
- Variables: `LocalVar`
- Exceptions: `TryCatch`, `Throw`
- Concurrency: `Synchronized`
- OOP: `SuperConstructorCall`
- Expression statements: `Expr(IrExpr)`
- Blocks: `Block(Vec<IrStmt>)`

**Types** (`types.rs`):

| Category   | Variants                                                       |
|------------|----------------------------------------------------------------|
| Primitives | `Bool`, `Byte`, `Short`, `Int`, `Long`, `Float`, `Double`, `Char`, `Void` |
| Reference  | `String`, `Array(T)`, `Class(name)`, `Nullable(T)`, `Generic { base, args }` |
| Special    | `Atomic(T)` (volatile), `TypeVar(name)` (generics), `Wildcard { bound }`, `Unknown`, `Null` |

All IR types derive `Serialize` and `Deserialize` (via serde) so the IR can be
dumped to JSON with `--dump-ir` for debugging.

### 3. Type Checker (crates/typeck)

The type checker walks the IR in-place and fills in every `ty: IrType` field on
expressions. It also performs symbol resolution: resolving bare variable names to
local variables, parameters, fields, or inherited fields.

Entry point:

```rust
pub fn type_check(module: IrModule) -> Result<IrModule, TypeckError>
```

The pass builds a `class_map: HashMap<String, IrClass>` for O(1) lookups, then
visits each class:

1. For each method, initialize a local environment (`HashMap<String, IrType>`)
   with `self` (for instance methods) and parameter bindings.
2. Walk statements. `LocalVar` declarations add entries to the environment.
3. For each expression, resolve names and compute the result type.

Key resolution rules:

- **Bare field references** in instance methods are rewritten to explicit
  `FieldAccess` nodes: `myField` becomes `self.myField`.
- **super keyword**: The parser lowers `super` to `_super`; the type checker
  rewrites it to `self._super` (a `FieldAccess` into the composition field).
- **Inheritance chain lookups**: `lookup_field_with_path()` walks the `_super`
  chain to find fields and methods declared in ancestor classes.
- **Type inference**: When a local variable has type `Unknown`, the type is
  inferred from its initializer expression.
- **Special methods**: `equals` is resolved to return `bool`; `hashCode` is
  resolved to return `i32`.

Error types: `UndefinedVariable`, `TypeMismatch`, `UndefinedClass`.

### 4. Codegen (crates/codegen)

Codegen converts the fully-typed IR into a Rust token stream using `proc-macro2`
and the `quote!` macro. The generated tokens are then formatted by
`prettyplease` into a clean Rust source string.

Entry point:

```rust
pub fn generate(module: &IrModule) -> Result<String, CodegenError>
```

The pass processes each declaration in the module:

**Interfaces** are emitted as Rust traits:
```rust
pub trait MyInterface {
    fn method_name(&mut self, params...) -> RetType;
}
```

**Classes** are emitted as a struct plus impl blocks:

1. **Struct definition** with `#[derive(Debug, Clone, Default)]`:
   - `pub _super: ParentClass` field if the class extends another
   - One `pub` field per instance field
2. **Constructor** (`pub fn new(...) -> Self`) with field initialization and
   optional `super()` delegation
3. **Instance and static methods** in an `impl ClassName { }` block
4. **Interface implementations** as `impl InterfaceTrait for ClassName { }`
5. **Auto-injected methods**:
   - `getClass(&self) -> JClass` returning a compile-time class descriptor
   - `_instanceof(&self, type_name: &str) -> bool` for runtime type checks
   - `__sync_monitor()` if the class has synchronized methods
6. **Delegation stubs** for inherited methods not overridden by the class
7. **Display impl** if the class defines `toString()`

**Main entry point**: The codegen finds the class with a static `main(String[])`
method and emits:
```rust
fn main() {
    let args = JArray::from_vec(
        std::env::args().skip(1).map(|s| JString::from(s.as_str())).collect(),
    );
    MainClass::main(args);
}
```

**Expression emission** maps IR nodes to Rust syntax:
- `System.out.println(x)` becomes `println!("{}", x)`
- `new ClassName(args)` becomes `ClassName::new(args)`
- `a + b` where either operand is a String becomes `format!("{}{}", a, b)`
- `obj.equals(other)` on String types passes `&other` (by reference)
- Java casts become Rust `as` casts for primitives

**Post-processing**: Tokens are parsed by `syn::parse_file` and formatted
through `prettyplease::unparse`. If parsing fails, the raw token stream
`.to_string()` is returned as a fallback.

### 5. CLI (crates/cli)

The `jtrans` binary ties all passes together. It supports three subcommands:

- `jtrans translate` -- the main translation command
- `jtrans init-maven` -- generate a Maven plugin fragment
- `jtrans init-gradle` -- generate a Gradle build script fragment

The translate pipeline per file is:

1. Read Java source
2. Parse to IR
3. (Optional) Dump IR as JSON (`--dump-ir`)
4. Type-check
5. Generate Rust code
6. Write to output directory (or stdout with `--print`)
7. Generate source map (`.jtrans-map`)

Additional features:

- **Incremental cache**: SHA-256 hash per input file stored in `.jtrans-cache`.
  Unchanged files are skipped on subsequent runs.
- **Watch mode**: Uses the `notify` crate to monitor input files and
  re-translate on change.
- **Cargo.toml generation**: Writes a `Cargo.toml` in the output directory with
  a `java-compat` dependency.

### 6. Runtime (crates/runtime / java-compat)

The `java-compat` crate provides Rust types that preserve Java semantics at
runtime. Translated programs depend on this crate. All shared mutable state uses
`Arc<RwLock<T>>` to avoid `unsafe`.

Core types:

| Type               | Java equivalent                     | Backing Rust type         |
|--------------------|-------------------------------------|---------------------------|
| `JString`          | `java.lang.String`                  | `Arc<str>`                |
| `JArray<T>`        | `T[]`                               | `Arc<RwLock<Vec<T>>>`     |
| `JList<T>`         | `java.util.ArrayList<T>`            | `Vec<T>`                  |
| `JMap<K,V>`        | `java.util.HashMap<K,V>`            | `HashMap<K,V>`            |
| `JSet<T>`          | `java.util.HashSet<T>`              | `HashSet<T>`              |
| `JOptional<T>`     | `java.util.Optional<T>`             | `Option<T>`               |
| `JStream<T>`       | `java.util.stream.Stream<T>`        | `Vec<T>` (eager)          |
| `JStringBuilder`   | `java.lang.StringBuilder`           | `String`                  |
| `JBigInteger`      | `java.math.BigInteger`              | `i128`                    |
| `JBigDecimal`      | `java.math.BigDecimal`              | `(i128, i32)` unscaled+scale |
| `JMathContext`     | `java.math.MathContext`              | `(i32, JRoundingMode)`    |
| `JRoundingMode`    | `java.math.RoundingMode`            | Rust enum                 |
| `JURL`             | `java.net.URL`                      | `String` (raw URL)        |
| `JSocket`          | `java.net.Socket`                   | `Option<TcpStream>`       |
| `JServerSocket`    | `java.net.ServerSocket`             | `Option<TcpListener>`     |
| `JHttpURLConnection` | `java.net.HttpURLConnection`      | Raw TCP HTTP/1.1          |
| `JAtomicInteger`   | `java.util.concurrent.AtomicInteger` | `Arc<AtomicI32>`         |
| `JAtomicLong`      | `java.util.concurrent.AtomicLong`   | `Arc<AtomicI64>`          |
| `JAtomicBoolean`   | `java.util.concurrent.AtomicBoolean`| `Arc<AtomicBool>`         |
| `JThread`          | `java.lang.Thread`                  | `std::thread::JoinHandle` |
| `JCountDownLatch`  | `CountDownLatch`                    | `Arc<(Mutex<i64>, Condvar)>` |
| `JSemaphore`       | `Semaphore`                         | `Arc<(Mutex<i32>, Condvar)>` |
| `JClass`           | `java.lang.Class<?>`                | Static `&str` name        |
| `JException`       | `java.lang.Exception`               | Panic payload encoding    |
| `JPattern/JMatcher`| `java.util.regex.Pattern/Matcher`   | `regex::Regex`            |
| `JLocalDate`       | `java.time.LocalDate`               | `(i32, u32, u32)` triple |
| `JLocalTime`       | `java.time.LocalTime`               | `(u32, u32, u32, u32)` hour/min/sec/nano |
| `JLocalDateTime`   | `java.time.LocalDateTime`           | `JLocalDate` + `JLocalTime` |
| `JInstant`         | `java.time.Instant`                 | `(i64, i32)` epoch_second + nano |
| `JDuration`        | `java.time.Duration`                | `(i64, i32)` seconds + nano |
| `JPeriod`          | `java.time.Period`                  | `(i32, i32, i32)` years/months/days |
| `JDateTimeFormatter`| `java.time.format.DateTimeFormatter`| Pattern `String`          |
| `JFile`            | `java.io.File`                      | `PathBuf`                 |

## Design Decisions

### Inheritance via Composition

Java's single inheritance is modelled as struct composition. A subclass contains
a `pub _super: ParentClass` field. Field and method lookups on the parent are
translated as accesses through this field:

```java
class Animal { String name; }
class Dog extends Animal { int age; }
```
becomes:
```rust
struct Animal { pub name: JString }
struct Dog { pub _super: Animal, pub age: i32 }
```

This avoids trait objects for basic inheritance while preserving field layout.

### Exceptions via Panics

Java `throw` is lowered to `panic!()` with a structured payload string
(`"JException:{ClassName}:{message}"`). `try/catch` becomes
`std::panic::catch_unwind` with pattern matching on the decoded payload.
`finally` blocks run unconditionally before any rethrow.

This preserves Java's stack-unwinding semantics without requiring `Result<T, E>`
return types throughout the call chain.

### Concurrency Mapping

- `synchronized` methods use a per-class `OnceLock<(Mutex<()>, Condvar)>` static
  monitor. The method body acquires the mutex guard and releases it on return.
- `synchronized(expr)` blocks use a global process-wide monitor via
  `__sync_block_monitor()`.
- `volatile` fields are wrapped in `IrType::Atomic` during parsing and emitted
  as `Arc<AtomicI32>` (or I64/Bool) with `SeqCst` ordering on all accesses.
- `wait()/notify()/notifyAll()` map to `Condvar` operations on the held guard.

### String Handling

Java strings are immutable and reference-counted. `JString` wraps `Arc<str>` for
cheap cloning and reference sharing. The `+` operator is overloaded via the
`Add` trait. When the codegen detects string concatenation involving non-String
operands, it emits `format!()` macro calls instead.

### Generic Classes

Java generic classes (`class Box<T>`) are emitted as Rust generic structs with
trait bounds `T: Clone + Default + Debug`. Type parameter bounds from Java's
`extends` clause are mapped to additional Rust traits:
`Comparable<T>` → `PartialOrd + Ord`, `Iterable<T>` → `IntoIterator`.

Type parameters are stored as `IrTypeParam { name, bounds }` (not bare strings),
preserving bound information through the IR.

Wildcard types (`?`, `? extends X`, `? super X`) are represented in the IR as
`IrType::Wildcard { bound }` and erased during code generation to the bound type
or `JavaObject` for unbounded wildcards.

Raw types (e.g. bare `List` without type parameters) are mapped to their runtime
collection type with `JavaObject` as the default type argument.

Boxed types (`Integer`, `Long`, etc.) are unboxed to their Rust primitive
equivalents during type resolution.

## Testing Strategy

The project uses differential testing as its primary correctness check. Each
integration test:

1. Reads a Java source file from `crates/tests/java/`
2. Runs the full pipeline: parse, type-check, codegen
3. Writes the generated Rust into a temporary Cargo project
4. Builds and runs the Rust program
5. Compares stdout against hardcoded expected output (obtained by running the
   same Java program with `javac` and `java`)

There are 76 integration tests covering all stages, plus unit tests in
individual crates:

- `codegen`: 40 unit tests
- `typeck`: 30 unit tests
- `java-compat` (runtime): 25 unit tests
- `ir`: 13 proptest property-based tests
- `parser`: 8 unit tests

Additional validation:

- `cargo-fuzz`: 2 fuzz targets covering parser and IR lowering
- `cargo miri`: runtime crate tested under Miri for undefined behaviour
  detection
- `cargo-audit`: 0 known vulnerabilities
- `cargo tarpaulin`: codegen 83.6% line coverage, typeck 87.6% line coverage
