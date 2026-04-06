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

The following are **not** supported:

- `ForkJoinPool`
- `java.util.concurrent.locks.StampedLock`
- Lambda-based closures capturing shared mutable state across multiple
  executor tasks (use `Runnable` implementations instead)
- `wait()` / `notify()` on arbitrary objects (only supported inside
  `synchronized` blocks on `this`)

## Collections (Advanced)

The core collections (`ArrayList`, `HashMap`, `HashSet`) are supported, along
with `LinkedList`, `ArrayDeque`, `PriorityQueue`, `TreeMap`, `TreeSet`,
`LinkedHashMap`, `LinkedHashSet`, `EnumMap`, `EnumSet`,
`Collections.sort()`, `Collections.reverse()`,
`Collections.unmodifiableList/Map/Set()`, `Collections.emptyList/Map/Set()`,
`Collections.singletonList()`, `Arrays.asList()`, and `Iterator` with
`hasNext()`/`next()`/`remove()`.

Map `keySet()`/`values()`/`entrySet()` iteration is supported on `HashMap`,
`TreeMap`, and `LinkedHashMap` via `JMapEntry<K,V>` for entry pairs.
`Spliterator` has a minimal stub (`trySplit`, `estimateSize`,
`characteristics`, `forEachRemaining`, `tryAdvance`).

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

### java.io / java.nio

The following java.io classes are supported: `File`, `FileReader`, `FileWriter`,
`BufferedReader`, `BufferedWriter`, `PrintWriter`, `FileInputStream`,
`FileOutputStream`, `Scanner`, `StringWriter`, `StringReader`,
`ByteArrayOutputStream`, and `ByteArrayInputStream`. The following java.nio.file
classes are supported: `Path`, `Paths`, and `Files` (with `readString`,
`writeString`, `readAllLines`, `write`, `exists`, `isDirectory`, `isRegularFile`,
`size`, `delete`, `deleteIfExists`, `createDirectory`, `createDirectories`,
`copy`, `move`).

The following are **not** supported:

- Abstract `InputStream` / `OutputStream` / `Reader` / `Writer` base classes as polymorphic types
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
`Name[f1=V1, f2=V2]` format. Advanced record features (compact constructors,
custom `equals`/`hashCode`) are not yet supported.

### Sealed Classes (Java 17+)

Sealed class/interface hierarchies are supported. The `sealed` modifier and
`permits` clause are accepted by the parser (the constraint is enforced
only at the Java level; no Rust-level restriction is emitted).

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
  name (`Outer$Inner`); implicit `this` reference to the outer instance is not
  supported — inner class methods cannot access outer fields.
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
