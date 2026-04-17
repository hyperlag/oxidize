# Limitations

This document lists Java features and patterns that oxidize does not support. If
the translator encounters one of these patterns, it will either report a parse
error, emit a `TODO` stub, or produce code that does not compile.

## Runtime Reflection

Java's `java.lang.reflect` package allows programs to inspect and manipulate
classes, methods, and fields at runtime. This is fundamentally incompatible with
Rust's static type system. The following reflection APIs are **not supported**:

- `Method.invoke(obj, args)` -- dynamic method dispatch
- `Field.get(obj)` / `Field.set(obj, value)` -- dynamic field access
- `Constructor.newInstance(args)` -- reflective construction
- `Class.forName(name)` -- loading classes by name at runtime
- `Class.getDeclaredMethods()` / `getDeclaredFields()` / `getDeclaredConstructors()`
- `java.lang.reflect.Proxy` -- dynamic proxy generation
- `AccessibleObject.setAccessible(true)` -- bypassing access modifiers

**What is supported:** `obj.getClass()` returns a compile-time `JClass`
descriptor with `getName()`, `getSimpleName()`, and `getCanonicalName()`.

## Dynamic Class Loading

- `ClassLoader.loadClass(name)` and custom classloaders
- `Class.forName(name)` with runtime-determined class names
- OSGi-style module systems
- Java agent bytecode instrumentation (`java.lang.instrument`)

These patterns require a JVM-like runtime. They cannot be represented in
statically compiled Rust.

## Native Methods and JNI

- `native` method declarations
- JNI (`Java Native Interface`) calls
- `System.loadLibrary()` / `System.load()`

Native methods call into C/C++ shared libraries through JNI. There is no
general way to translate these to Rust.

## Annotations (Beyond Syntax)

Annotations are parsed and tolerated syntactically (`@Override`, `@Deprecated`,
custom annotations), but they have no effect on code generation. The following
annotation-driven features are not supported:

- Annotation processing (`javax.annotation.processing`)
- Runtime annotation queries (`method.getAnnotation(...)`)
- Framework-specific annotations (Spring `@Autowired`, JPA `@Entity`, etc.)
- `@Retention(RUNTIME)` annotations that expect reflective access

## Generics (Advanced)

Basic generic classes and methods work, including:

- **Bounded type parameters**: `<T extends Comparable<T>>` – bounds are parsed
  and mapped to Rust trait bounds (e.g. `Comparable` → `PartialOrd + Ord`,
  `Iterable` → `IntoIterator`).
- **Multiple bounds**: `<T extends Number & Comparable<T>>` – all applicable
  bounds are combined.
- **Wildcard types**: `List<?>`, `List<? extends Number>`,
  `List<? super Integer>` – parsed and erased to their bound type (or
  `JavaObject` for unbounded `?`), since Rust has no wildcard generics.
- **Raw types**: Bare collection names like `List`, `Map`, `Set` without type
  parameters are mapped to their Rust equivalents with a `JavaObject` default
  type argument (e.g. `List` → `JList<JavaObject>`).

The following advanced generics features are **not** supported:

- Generic method type inference across call chains
- Higher-kinded types or type-constructor polymorphism
- Translating `compareTo()` / `equals()` from generic bound methods

## Concurrency (Advanced)

Basic threading (`Thread`, `synchronized`, `volatile`, `AtomicInteger`,
`CountDownLatch`, `Semaphore`) is supported. The following higher-level
concurrency utilities are also supported:

- `ReentrantLock` / `Condition` (lock, unlock, tryLock, newCondition,
  await, signal, signalAll)
- `ReentrantReadWriteLock` / `ReadLock` / `WriteLock` (read/write lock
  separation with lock, unlock, tryLock)
- `ConcurrentHashMap` (put, get, containsKey, remove, size, isEmpty, clear,
  putIfAbsent, getOrDefault)
- `CopyOnWriteArrayList` (add, get, set, remove, size, isEmpty, contains,
  clear, indexOf)
- `ThreadLocal` (get, set, remove, withInitial)
- `ExecutorService` / `Executors` (newFixedThreadPool,
  newSingleThreadExecutor, newCachedThreadPool, execute, submit, shutdown,
  awaitTermination, isShutdown)
- `Future` (get, isDone)
- `CompletableFuture` (supplyAsync, runAsync, completedFuture, get, join,
  isDone, thenApply, thenAccept, thenCompose)
- `TimeUnit` (NANOSECONDS through DAYS, conversion methods)
- `ForkJoinPool` / `RecursiveTask<T>` / `RecursiveAction` (fork, join,
  compute, invoke, commonPool)
- `StampedLock` (writeLock, unlockWrite, readLock, unlockRead,
  tryOptimisticRead, validate)
- `synchronized(obj)` blocks on arbitrary objects (each user class gets a
  per-object `JMonitor`; nested synchronized blocks on distinct objects work)
- `obj.wait()` / `obj.notify()` / `obj.notifyAll()` when `obj` is the exact
  variable used as the monitor in the enclosing `synchronized(obj)` block

The following are **not** supported:

- Lambda-based closures capturing shared mutable state across multiple
  executor tasks (use `Runnable` implementations instead)
- `this.wait()` / `this.notify()` inside a `synchronized(this)` block
  (unqualified `wait()`/`notify()` inside a `synchronized` instance method do work)
- `wait()`/`notify()` when the receiver is a non-variable expression or when
  the monitor is a built-in type (String, collection, array)

## Collections (Advanced)

The core collections (`ArrayList`, `HashMap`, `HashSet`) are supported, along
with `LinkedList`, `ArrayDeque`, `PriorityQueue`, `TreeMap`, `TreeSet`,
`LinkedHashMap`, `LinkedHashSet`, `EnumMap`, `EnumSet`,
`Collections.sort()`, `Collections.reverse()`,
`Collections.unmodifiableList/Map/Set()`, `Collections.emptyList/Map/Set()`,
`Collections.singletonList()`, `Arrays.asList()`, and `Iterator` with
`hasNext()`/`next()`/`remove()`.

**Stage 10 additions:**
- `Arrays`: `sort()`, `fill()`, `copyOf()`, `copyOfRange()`, `binarySearch()`,
  `toString()`, `equals()`, `stream()`, `asList()`.
- `Collections`: added `min()`, `max()`, `frequency()`, `nCopies()`, `fill()`,
  `swap()`, `disjoint()`, `binarySearch()`. `shuffle()` is currently a
  deterministic stub/no-op and should not be relied on for randomization.

**Stage 11 additions:**
- `Collectors`: `toList()`, `toSet()`, `toUnmodifiableList()`, `counting()`,
  `joining()`, `joining(sep)`, `joining(sep, prefix, suffix)`,
  `toMap(keyFn, valFn)` (duplicate keys currently overwrite earlier values
  instead of throwing `IllegalStateException` as in Java),
  `groupingBy(classifier)`.
- `Stream` static factories: `Stream.of(...)`, `IntStream.range(a,b)`,
  `IntStream.rangeClosed(a,b)`.
- Stream terminal ops: `anyMatch()`, `allMatch()`, `noneMatch()`, `peek()`,
  `IntStream.sum()`.
- `StringBuilder`: added `replace(start, end, str)`, `lastIndexOf(s)` (in
  addition to previously supported `append`, `toString`, `length`, `charAt`,
  `reverse`, `insert`, `delete`, `deleteCharAt`, `indexOf`, `setCharAt`,
  `substring`).

Map `keySet()`/`values()`/`entrySet()` iteration is supported on `HashMap`,
`TreeMap`, and `LinkedHashMap` via `JMapEntry<K,V>` for entry pairs.
`Spliterator` has a minimal stub (`trySplit`, `estimateSize`,
`characteristics`, `forEachRemaining`, `tryAdvance`).

**Modern String API (Stage 10):** `strip()`, `stripLeading()`, `stripTrailing()`,
`isBlank()`, `repeat(n)`, `lines()` (returns `Stream<String>`), `chars()` (returns
`Stream<Character>`), `toCharArray()` are now supported in addition to the
previously supported `trim()`, `substring()`, `contains()`, `startsWith()`,
`endsWith()`, `indexOf()`, `replace()`, `split()`, `toUpperCase()`,
`toLowerCase()`, `charAt()`, `length()`, `isEmpty()`, `getBytes()`.

### Standard Library Gaps

### java.lang

- `Runtime.getRuntime()` (JVM runtime queries beyond subprocess execution)
- `ClassLoader`
- `SecurityManager`

**Supported:** `System.exit()`, `System.currentTimeMillis()`, `System.nanoTime()`,
`System.getenv()`, `System.getProperty()` (1- and 2-arg forms with inline
known-property resolution), `System.lineSeparator()`, `System.arraycopy()`,
`ProcessBuilder` / `Process` (subprocess spawn, stdout/stderr capture, exit
code, working-directory override; `Runtime.getRuntime().exec(String)` is a
convenience alias). Class literals (`Foo.class`) produce a `JClass` descriptor
with `getSimpleName()`, `getName()`, and `getCanonicalName()` methods;
`getClass()` is also supported on all generated classes.

**Boxed types and `Objects` utility (Stage 10):**
- `Integer`: `MAX_VALUE`, `MIN_VALUE`, `SIZE`, `BYTES` constants; `parseInt(s)`,
  `parseInt(s, radix)`, `valueOf()`, `toString()`, `toBinaryString()`,
  `toHexString()`, `toOctalString()`, `bitCount()`, `highestOneBit()`,
  `numberOfLeadingZeros()`, `numberOfTrailingZeros()`, `compare()`, `signum()`,
  `max()`, `min()`, `sum()`.
- `Long`: `MAX_VALUE`, `MIN_VALUE` constants; `parseLong()`, `valueOf()`,
  `toString()`, `toBinaryString()`, `toHexString()`, `toOctalString()`,
  `bitCount()`, `compare()`, `max()`, `min()`.
- `Double`: `MAX_VALUE`, `MIN_VALUE`, `NaN`, `POSITIVE_INFINITY`,
  `NEGATIVE_INFINITY` constants; `parseDouble()`, `valueOf()`, `toString()`,
  `isNaN()`, `isInfinite()`, `compare()`, `max()`, `min()`.
- `Float`: `MAX_VALUE`, `MIN_VALUE` constants.
- `Character`: `isDigit()`, `isLetter()`, `isAlphabetic()`, `isLetterOrDigit()`,
  `isWhitespace()`, `isSpaceChar()`, `isUpperCase()`, `isLowerCase()`,
  `toUpperCase()`, `toLowerCase()`, `getNumericValue()`, `digit()`, `forDigit()`,
  `toString()`.
- `Objects`: `requireNonNull()`, `requireNonNullElse()`, `isNull()`, `nonNull()`,
  `equals()`, `deepEquals()`, `hash()`, `hashCode()`, `toString()`, `compare()`.
- `Math`: `PI`, `E`, `TAU` constants; added `signum()`, `hypot()`, `atan2()`,
  `asin()`, `acos()`, `atan()`, `sinh()`, `cosh()`, `tanh()`, `toDegrees()`,
  `toRadians()`, `cbrt()`, `copySign()` (in addition to the previously supported
  `abs`, `ceil`, `floor`, `pow`, `sqrt`, `log`, `log10`, `round`, `max`, `min`,
  `random`).

### java.io / java.nio

The following java.io classes are supported: `File`, `FileReader`, `FileWriter`,
`BufferedReader`, `BufferedWriter`, `PrintWriter`, `FileInputStream`,
`FileOutputStream`, `Scanner`, `StringWriter`, `StringReader`,
`ByteArrayOutputStream`, and `ByteArrayInputStream`. Abstract I/O base types
(`InputStream`, `OutputStream`, `Reader`, `Writer`) are supported as polymorphic
enum types — concrete constructors are automatically wrapped when assigned to
an abstract-typed variable. `BufferedReader(new StringReader(...))` is supported.
The following java.nio.file classes are supported: `Path`, `Paths`, and `Files`
(with `readString`, `writeString`, `readAllLines`, `write`, `exists`,
`isDirectory`, `isRegularFile`, `size`, `delete`, `deleteIfExists`,
`createDirectory`, `createDirectories`, `copy`, `move`).

The following are **not** supported:

- `java.nio.channels` (NIO channels and selectors)
- Serialization (`Serializable`, `ObjectInputStream`, `ObjectOutputStream`)

### java.net

**Supported:** `URL` (parsing, component accessors), `Socket`, `ServerSocket`
(TCP wrappers), `HttpURLConnection` (basic HTTP/1.1 GET/POST),
`HttpClient` / `HttpRequest` / `HttpResponse` (Java 11+ HTTP client builder
pattern; real HTTP/1.1 over raw TCP).

### java.util

**Supported:** `ResourceBundle` / `PropertyResourceBundle` — backed by a
`HashMap<String, String>`. `ResourceBundle.getBundle(name)` loads
`<name>.properties` from the current working directory. `new PropertyResourceBundle(ByteArrayInputStream)`
reads `.properties`-formatted content. Methods: `getString(key)`,
`getObject(key)`, `containsKey(key)`, `keySet()`.

Additionally supported: `String.format()` (specifiers: `%s`, `%d`, `%f`, `%e`, `%x`, `%o`,
`%b`, `%n`, `%%`; limited support for width, precision, and flags `-` and `0`),
`String.join()`, `System.out.printf()`, `Properties` (`load_string`, `getProperty`,
`getProperty` with default, `setProperty`, `containsKey`, `stringPropertyNames`, `size`,
`isEmpty`), `Timer` / `TimerTask` (one-shot and
repeating scheduled tasks with cancel/purge).

### java.time

**Supported:** `LocalDate`, `LocalTime`, `LocalDateTime`, `Instant`, `Duration`,
`Period`, `DateTimeFormatter` (pattern-based formatting with `ofPattern()`),
`ZonedDateTime`, `ZoneId` (UTC and `±HH:MM` offsets), `Clock`
(`systemUTC`, `systemDefaultZone`, `instant`, `millis`, `getZone`).
Covers construction, arithmetic, comparison, parsing, and formatting for all
supported types.

### java.math

**Supported:** `BigDecimal` (arithmetic, comparison, rounding, scale operations),
`MathContext` (precision + rounding context), `RoundingMode`, and `BigInteger`.

## Language Features

### Enums (Advanced)

Basic Java enums are supported, including:

- Simple unit enums (e.g., `enum Color { RED, GREEN, BLUE }`)
- Enums with fields, constructors, and methods
- Built-in methods: `name()`, `ordinal()`, `values()`, `valueOf()`, `equals()`
- Enum switch statements
- Enum equality comparisons (`==`, `.equals()`)

The following advanced enum features are **not** supported:

- Anonymous constant subclasses with fields (beyond method overrides)

### Records (Java 16+)

Basic `record` declarations are supported. The parser generates an `IrClass`
with `is_record: true`, a canonical constructor, public final fields, and
accessor methods (`x()`, `y()`, etc.). `Display` is implemented with the
`Name[f1=V1, f2=V2]` format. Compact constructors are supported — the compact
body runs before the implicit field assignments, matching Java semantics.
Custom `equals`/`hashCode`/`toString` methods in a record body are parsed and
emitted as regular methods.

### Pattern Matching in Switch (Java 21)

Arrow-form type-pattern **switch statements** (`case String s -> ...`) are
supported. The parser transforms them into a nested if-else chain using
`instanceof` with binding variables.

The following are currently not supported:

- Colon-form pattern labels in statements (`case String s:`)
- Pattern switch expressions (`var x = switch (obj) { ... }`)

### Sealed Classes (Java 17+)

Sealed class/interface hierarchies are supported. The `sealed` modifier and
`permits` clause are accepted by the parser (the constraint is enforced
only at the Java level; no Rust-level restriction is emitted).

### `var` Keyword (Java 10+)

`var` in local variable declarations and for-each loops is supported.
The declared type is inferred from the initializer expression (mapped to
`IrType::Unknown`, which Rust infers automatically).

### Switch Expressions (Java 14+)

Arrow-form switch expressions (`int x = switch(y) { case A -> 1; default -> 0; }`)
are supported and lowered to Rust `match` expressions. Arrow-form switch statements
(`switch (x) { case A -> stmt; }`) are also supported.

Switch expressions with patterns (Java 21) are not yet supported.

### Method References (Java 8+)

Static method references (`ClassName::method`) and constructor references
(`ClassName::new`) are supported. Bound instance method references
(`obj::method`) are not yet supported. Supported method references lower to
single-argument Rust closures. Multi-argument method references are not yet
supported.

### Interface Default Methods (Java 8+)

`default` methods in interface bodies are supported. A class that does not
override a default method automatically inherits the interface's implementation.

### Pattern Matching (Java 16+)

- `instanceof` with a single binding variable is supported:
  `if (obj instanceof Box x) { ... }` injects `let mut x: Box = obj.clone();`
  at the start of the then-block.
- Switch expressions with patterns (Java 21) are not yet supported.

### Text Blocks (Java 13+)

Multi-line string literals (`"""..."""`) are supported. The parser strips
the common leading indentation per the Java spec (JEP 378) and preserves
relative indentation within the block.

### Modules (Java 9+)

The `module-info.java` module system is not supported. All translation operates
on individual `.java` files or flat directories.

### Inner Classes

Non-static inner classes, anonymous inner classes, and local classes are now
supported with the following limitations:

- **Anonymous inner classes:** supported when implementing an interface with
  method overrides; outer-scope variable capture is not supported.
- **Non-static inner classes:** hoisted to top-level structs with a mangled
  name (`Outer$Inner`); outer field and method access via `OuterClass.this.field`
  and `OuterClass.this.method()` is supported. The outer reference is a snapshot
  clone taken at construction time, so mutations to outer state made via the inner
  class reference are not reflected back in the original outer object.
- **Local classes:** classes declared inside a method body are hoisted to
  top-level structs using a mangled name (`Outer__loc__Local`); outer-scope
  variable capture is not supported.

Static nested classes are supported as top-level classes.

### Multiple Classes Per File

Each `.java` file should contain one public class. Multiple top-level class
declarations in a single file may not translate correctly.

### Package-Level Features

- Package declarations are parsed but not used for code organization
- Import statements are parsed but not resolved against a classpath
- Fully-qualified type names (e.g., `java.util.List`) are not resolved

### Varargs

```java
void log(String... messages) { ... }
```

Varargs methods are supported. Parameters are emitted as `JArray<T>` and
call sites automatically bundle trailing arguments into an array.

### Multi-Dimensional Arrays

```java
int[][] matrix = new int[3][4];
```

Multi-dimensional arrays are supported. `new T[r][c]` is emitted as
`JArray::<JArray<T>>::new_with(r, |_| JArray::<T>::new_default(c))` and
array accesses chain `.get(i).get(j)` calls.

### Reference Casting

Same-type reference casts (e.g. `(String) str`) are handled as identity
expressions. True runtime downcasts (e.g. `(Dog) animal` where `animal` is
an `Animal` reference) are not checked at runtime — the transpiled Rust
will fail to compile if the declared types differ.

### Static Initializers

```java
class Foo {
    static { System.out.println("loaded"); }
}
```

Static initializer blocks are supported in a limited form. They are lowered to a
`std::sync::Once`-guarded `__run_static_init()` method that is called at the
start of every translated static method, constructor, and instance method in the
class. Constructors and instance methods participate in this scheme, but direct
static field reads/writes that do not go through any method do not currently
trigger class initialization, so full Java class initialization semantics are not
guaranteed.

### Try-with-resources (Custom AutoCloseable)

Try-with-resources works for the built-in desugaring (`close()` call in
`finally`), but custom `AutoCloseable` implementations are not verified.

## Generated Code Quality

Generated Rust code compiles and passes `cargo clippy`, but it may contain
patterns that a human Rust developer would write differently:

- Unnecessary parentheses around some expressions
- `mut` on parameters that are not actually mutated
- Method parameters passed by value where a reference would suffice
- `_super` field naming for inheritance composition
- All fields are `pub` regardless of Java visibility modifiers
- Access modifiers (`private`, `protected`, `package-private`) are not enforced
  in the generated Rust

## Translation Performance

Translation speed is 2 to 8 milliseconds per file in release builds. However,
the generated Rust programs may have slower runtime performance than the original
Java in cases where:

- `Arc<RwLock<T>>` introduces overhead compared to Java's garbage collector
- `JString` (reference-counted) has different allocation characteristics than
  JVM string interning
- `panic!` + `catch_unwind` for exceptions is heavier than JVM exception handling

## Reporting Issues

If you encounter a Java program that oxidize should support but does not, please
open an issue with the Java source code and the error message or incorrect output.
