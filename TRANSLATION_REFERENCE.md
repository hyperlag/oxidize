# Translation Reference

This document lists every Java construct that oxidize supports and shows its Rust
equivalent. When a construct is not supported, the translator will report an error
or emit a `TODO` stub.

## Primitive Types

| Java       | Rust   | Notes                                      |
|------------|--------|--------------------------------------------|
| `boolean`  | `bool` |                                            |
| `byte`     | `i8`   |                                            |
| `short`    | `i16`  |                                            |
| `int`      | `i32`  |                                            |
| `long`     | `i64`  |                                            |
| `float`    | `f32`  |                                            |
| `double`   | `f64`  |                                            |
| `char`     | `char` |                                            |
| `void`     | `()`   |                                            |

## Boxed Types (Auto-unboxing)

| Java          | Rust   |
|---------------|--------|
| `Integer`     | `i32`  |
| `Long`        | `i64`  |
| `Double`      | `f64`  |
| `Float`       | `f32`  |
| `Boolean`     | `bool` |
| `Character`   | `char` |
| `Byte`        | `i8`   |
| `Short`       | `i16`  |

## Reference Types

| Java                          | Rust                         |
|-------------------------------|------------------------------|
| `String`                      | `JString` (`Arc<str>`)       |
| `T[]`                         | `JArray<T>`                  |
| `List<T>` / `ArrayList<T>`   | `JList<T>`                   |
| `Map<K,V>` / `HashMap<K,V>`  | `JMap<K,V>`                  |
| `Set<T>` / `HashSet<T>`      | `JSet<T>`                    |
| `EnumMap<K,V>`                | `JEnumMap<K,V>`              |
| `EnumSet<T>`                  | `JEnumSet<T>`                |
| `Optional<T>`                 | `JOptional<T>`               |
| `Stream<T>`                   | `JStream<T>`                 |
| `StringBuilder`               | `JStringBuilder`             |
| `BigInteger`                  | `JBigInteger`                |
| `Pattern`                     | `JPattern`                   |
| `Matcher`                     | `JMatcher`                   |
| `LocalDate`                   | `JLocalDate`                 |
| `LocalTime`                   | `JLocalTime`                 |
| `LocalDateTime`               | `JLocalDateTime`             |
| `Instant`                     | `JInstant`                   |
| `Duration`                    | `JDuration`                  |
| `Period`                      | `JPeriod`                    |
| `DateTimeFormatter`           | `JDateTimeFormatter`         |
| `File`                        | `JFile`                      |
| `FileReader`                  | `JFileReader`                |
| `FileWriter`                  | `JFileWriter`                |
| `BufferedReader`              | `JBufferedReader`            |
| `BufferedWriter`              | `JBufferedWriter`            |
| `PrintWriter`                 | `JPrintWriter`               |
| `FileInputStream`             | `JFileInputStream`           |
| `FileOutputStream`            | `JFileOutputStream`          |
| `Scanner`                     | `JScanner`                   |
| `Path`                        | `JPath`                      |
| `Files`                       | `JFiles`                     |
| `Thread`                      | `JThread`                    |
| `AtomicInteger`               | `JAtomicInteger`             |
| `AtomicLong`                  | `JAtomicLong`                |
| `AtomicBoolean`               | `JAtomicBoolean`             |
| `CountDownLatch`              | `JCountDownLatch`            |
| `Semaphore`                   | `JSemaphore`                 |
| `ReentrantLock`               | `JReentrantLock`             |
| `Condition`                   | `JCondition`                 |
| `ReentrantReadWriteLock`      | `JReentrantReadWriteLock`    |
| `ConcurrentHashMap<K,V>`      | `JConcurrentHashMap<K,V>`    |
| `CopyOnWriteArrayList<T>`     | `JCopyOnWriteArrayList<T>`   |
| `ThreadLocal<T>`              | `JThreadLocal<T>`            |
| `ExecutorService`             | `JExecutorService`           |
| `Executors`                   | `JExecutors`                 |
| `Future<T>`                   | `JFuture<T>`                 |
| `CompletableFuture<T>`        | `JCompletableFuture<T>`      |
| `TimeUnit`                    | `JTimeUnit`                  |
| User-defined class `Foo`      | `struct Foo`                 |

## Literals

| Java                  | Rust                          |
|-----------------------|-------------------------------|
| `true` / `false`      | `true` / `false`              |
| `42`                  | `42`                          |
| `42L`                 | `42_i64`                      |
| `3.14`                | `3.14_f64`                    |
| `3.14f`               | `3.14_f32`                    |
| `'a'`                 | `'a'`                         |
| `"hello"`             | `JString::from("hello")`      |
| `null`                | `None` (inside `Option<T>`)   |

## Operators

### Arithmetic

| Java      | Rust     |
|-----------|----------|
| `a + b`   | `a + b`  |
| `a - b`   | `a - b`  |
| `a * b`   | `a * b`  |
| `a / b`   | `a / b`  |
| `a % b`   | `a % b`  |

### Comparison

| Java       | Rust      |
|------------|-----------|
| `a == b`   | `a == b`  |
| `a != b`   | `a != b`  |
| `a < b`    | `a < b`   |
| `a <= b`   | `a <= b`  |
| `a > b`    | `a > b`   |
| `a >= b`   | `a >= b`  |

### Logical

| Java        | Rust       |
|-------------|------------|
| `a && b`    | `a && b`   |
| `a \|\| b`  | `a \|\| b`  |
| `!a`        | `!a`       |

### Bitwise

| Java       | Rust      |
|------------|-----------|
| `a & b`    | `a & b`   |
| `a \| b`   | `a \| b`   |
| `a ^ b`    | `a ^ b`   |
| `~a`       | `!a`      |
| `a << b`   | `a << b`  |
| `a >> b`   | `a >> b`  |
| `a >>> b`  | `(a as u32) >> b` (unsigned right shift) |

### Assignment

| Java        | Rust       |
|-------------|------------|
| `a = b`     | `a = b`    |
| `a += b`    | `a += b`   |
| `a -= b`    | `a -= b`   |
| `a *= b`    | `a *= b`   |
| `a /= b`    | `a /= b`   |
| `a %= b`    | `a %= b`   |
| `a &= b`    | `a &= b`   |
| `a \|= b`   | `a \|= b`   |
| `a ^= b`    | `a ^= b`   |
| `a <<= b`   | `a <<= b`  |
| `a >>= b`   | `a >>= b`  |

### Increment / Decrement

| Java    | Rust                                  |
|---------|---------------------------------------|
| `++x`   | `{ x += 1; x }` (pre-increment)      |
| `x++`   | `{ let tmp = x; x += 1; tmp }` (post-increment) |
| `--x`   | `{ x -= 1; x }` (pre-decrement)      |
| `x--`   | `{ let tmp = x; x -= 1; tmp }` (post-decrement) |

### Ternary

| Java              | Rust                          |
|-------------------|-------------------------------|
| `cond ? a : b`    | `if cond { a } else { b }`   |

### String Concatenation

| Java                        | Rust                                 |
|-----------------------------|--------------------------------------|
| `"a" + "b"`                 | `JString::from("a") + JString::from("b")` |
| `"val=" + x` (mixed types)  | `format!("val={}", x)` via `JString::from(...)` |

## Control Flow

### If / Else

```java
if (cond) { ... }
else if (cond2) { ... }
else { ... }
```
```rust
if cond { ... }
else if cond2 { ... }
else { ... }
```

### While

```java
while (cond) { ... }
```
```rust
while cond { ... }
```

### Do-While

```java
do { ... } while (cond);
```
```rust
loop {
    ...
    if !cond { break; }
}
```

### For Loop

```java
for (int i = 0; i < n; i++) { ... }
```
```rust
let mut i: i32 = 0;
while i < n {
    ...
    i += 1;
}
```

### Enhanced For (For-Each)

```java
for (String s : list) { ... }
```
```rust
for s in list.iter() { ... }
```

### Switch

```java
switch (x) {
    case 1: ...; break;
    case 2: ...; break;
    default: ...;
}
```
```rust
match x {
    1 => { ... }
    2 => { ... }
    _ => { ... }
}
```

### Multi-Label Switch (Java 14+)

```java
switch (day) {
    case 1, 2, 3, 4, 5 -> "Weekday";
    case 6, 7 -> "Weekend";
    default -> "Unknown";
}
```
```rust
match day {
    1 => JString::from("Weekday"),
    2 => JString::from("Weekday"),
    3 => JString::from("Weekday"),
    4 => JString::from("Weekday"),
    5 => JString::from("Weekday"),
    6 => JString::from("Weekend"),
    7 => JString::from("Weekend"),
    _ => JString::from("Unknown"),
}
```

### Pattern Switch (Java 21)

```java
switch (obj) {
    case MyType x -> System.out.println(x.field);
    default -> System.out.println("other");
}
```
```rust
// Transformed to if-else chain with instanceof checks:
{
    let __instanceof_tmp__ = obj;
    if __instanceof_tmp__._instanceof("MyType") {
        let mut x: MyType = __instanceof_tmp__.clone();
        println!("{}", (x).field);
    } else {
        println!("{}", "other");
    }
}
```

### Break / Continue

```java
break;
continue;
```
```rust
break;
continue;
```

## Classes

### Basic Class

```java
public class Point {
    int x;
    int y;
    public Point(int x, int y) {
        this.x = x;
        this.y = y;
    }
    public int getX() { return x; }
}
```
```rust
#[derive(Debug, Clone, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}
impl Point {
    pub fn new(mut x: i32, mut y: i32) -> Self {
        let mut __self__ = Self { x: 0, y: 0 };
        __self__.x = x;
        __self__.y = y;
        __self__
    }
    pub fn getX(&mut self) -> i32 {
        return (self).x;
    }
}
```

### Inheritance

```java
class Animal {
    String name;
    Animal(String name) { this.name = name; }
}
class Dog extends Animal {
    int age;
    Dog(String name, int age) {
        super(name);
        this.age = age;
    }
}
```
```rust
#[derive(Debug, Clone, Default)]
pub struct Animal {
    pub name: JString,
}
impl Animal {
    pub fn new(mut name: JString) -> Self { ... }
}

#[derive(Debug, Clone, Default)]
pub struct Dog {
    pub _super: Animal,
    pub age: i32,
}
impl Dog {
    pub fn new(mut name: JString, mut age: i32) -> Self {
        let mut __self__ = Self {
            _super: Animal::new(name.clone()),
            age: 0,
        };
        __self__.age = age;
        __self__
    }
}
```

Parent fields are accessed through the `_super` composition field:
`dog._super.name`.

### Interfaces

```java
interface Greetable {
    String greet();
}
class Person implements Greetable {
    public String greet() { return "Hello"; }
}
```
```rust
pub trait Greetable {
    fn greet(&mut self) -> JString;
}
impl Greetable for Person {
    fn greet(&mut self) -> JString {
        return JString::from("Hello");
    }
}
```

### Static Methods and Fields

```java
class MathUtil {
    static int add(int a, int b) { return a + b; }
}
```
```rust
impl MathUtil {
    pub fn add(mut a: i32, mut b: i32) -> i32 {
        return a + b;
    }
}
```

Static methods have no `&self` or `&mut self` parameter.

### instanceof

```java
if (obj instanceof Dog) { ... }
```
```rust
if obj._instanceof("Dog") { ... }
```

Every generated class includes an `_instanceof` method that checks the type name
and the names of all ancestor classes.

### getClass()

```java
obj.getClass().getName()
```
```rust
obj.getClass().getName()
// Returns JString with the class name
```

Every generated class includes a `getClass()` method returning a `JClass`
compile-time descriptor.

### toString() / Display

```java
class Foo {
    public String toString() { return "Foo!"; }
}
System.out.println(foo); // prints "Foo!"
```
```rust
impl std::fmt::Display for Foo {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.clone().toString())
    }
}
println!("{}", foo); // prints "Foo!"
```

Classes that define `toString()` automatically get a `Display` impl.

### equals() / hashCode()

```java
public boolean equals(Box other) { return this.value == other.value; }
public int hashCode() { return value; }
```
```rust
pub fn equals(&mut self, other: &Box) -> bool {
    return (self).value == (other).value;
}
pub fn hashCode(&mut self) -> i32 {
    return (self).value;
}
```

When `equals()` is called on a String-typed receiver, the argument is
automatically passed by reference (`&`).

## Generics

```java
class Wrapper<T> {
    T value;
    Wrapper(T value) { this.value = value; }
    T getValue() { return value; }
}
```
```rust
#[derive(Debug, Clone, Default)]
pub struct Wrapper<T> {
    pub value: T,
}
impl<T: Clone + Default + std::fmt::Debug> Wrapper<T> {
    pub fn new(mut value: T) -> Self { ... }
    pub fn getValue(&mut self) -> T { ... }
}
```

Generic type parameters get `Clone + Default + Debug` bounds.

### Bounded Type Parameters

```java
class SortedPair<T extends Comparable<T>> {
    T first, second;
}
```
```rust
pub struct SortedPair<T> { pub first: T, pub second: T }
impl<T: Clone + Default + std::fmt::Debug + PartialOrd + Ord> SortedPair<T> { ... }
```

Java `Comparable<T>` → Rust `PartialOrd + Ord`.  
Java `Iterable<T>` → Rust `IntoIterator`.  
Other bounds (e.g. `Cloneable`, `Serializable`, `Number`) are silently ignored.

### Wildcard Types

```java
void process(List<?> list) { ... }
void addAll(List<? extends Number> nums) { ... }
void insert(List<? super Integer> dest) { ... }
```
```rust
fn process(list: JList<JavaObject>) { ... }   // <?> → JavaObject
fn addAll(nums: JList<JavaObject>) { ... }    // <? extends Number> → JavaObject
fn insert(dest: JList<JavaObject>) { ... }   // <? super Integer> → JavaObject
```

Wildcards are erased: all wildcard types (`?`, `? extends X`, `? super X`) map to `JavaObject`.
Unmapped JDK bound types (e.g. `Number`, `Serializable`) are not available in the runtime, so
erasing to `JavaObject` avoids referencing undefined identifiers in the generated Rust code.

### Raw Types

```java
List list = new ArrayList();    // bare List without type parameter
Map  map  = new HashMap();
```
```rust
let list: JList<JavaObject> = JList::new();
let map:  JMap<JavaObject, JavaObject> = JMap::new();
```

Raw collection types are mapped to their runtime equivalents with `JavaObject` as the default type argument.

## Enums

### Simple Enum

```java
enum Color { RED, GREEN, BLUE }
```
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color { RED, GREEN, BLUE }
impl Color {
    pub fn name(&self) -> JString { ... }
    pub fn ordinal(&self) -> i32 { ... }
    pub fn values() -> Vec<Color> { ... }
    pub fn valueOf(s: JString) -> Color { ... }
    pub fn equals(&self, other: Color) -> bool { ... }
}
impl std::fmt::Display for Color { ... }
```

### Enum with Fields

```java
enum Coin {
    PENNY(1), NICKEL(5), DIME(10), QUARTER(25);
    private final int cents;
    Coin(int cents) { this.cents = cents; }
    int getCents() { return cents; }
}
```
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Coin { PENNY, NICKEL, DIME, QUARTER }
impl Coin {
    fn __data(&self) -> (i32,) {
        match self {
            Self::PENNY => (1,),
            Self::NICKEL => (5,),
            Self::DIME => (10,),
            Self::QUARTER => (25,),
        }
    }
    pub fn cents(&self) -> i32 { self.__data().0 }
    pub fn getCents(&self) -> i32 { return self.cents(); }
    // ... name(), ordinal(), values(), valueOf(), equals()
}
```

Constructor arguments are stored via a `__data()` method that maps each variant
to a tuple. Fields become accessor methods indexing into the tuple.

### Enum Switch

```java
switch (color) {
    case RED: System.out.println("red"); break;
    case GREEN: System.out.println("green"); break;
}
```
```rust
match color {
    Color::RED => { println!("{}", JString::from("red")); }
    Color::GREEN => { println!("{}", JString::from("green")); }
    _ => {}
}
```

Bare case labels (e.g., `case RED`) are qualified with the enum type name.
Trailing `break` statements are stripped (Rust match arms do not fall through).

## Collections

### List

| Java                          | Rust                          |
|-------------------------------|-------------------------------|
| `new ArrayList<>()`           | `JList::new()`                |
| `list.add(x)`                | `list.add(x)`                |
| `list.get(i)`                | `list.get(i)`                |
| `list.set(i, x)`             | `list.set(i, x)`             |
| `list.size()`                | `list.size()`                |
| `list.remove(i)`             | `list.remove(i)`             |
| `list.contains(x)`           | `list.contains(&x)`          |
| `list.indexOf(x)`            | `list.indexOf(&x)`           |
| `list.isEmpty()`             | `list.isEmpty()`             |
| `list.clear()`               | `list.clear()`               |

### Map

| Java                          | Rust                          |
|-------------------------------|-------------------------------|
| `new HashMap<>()`            | `JMap::new()`                 |
| `map.put(k, v)`              | `map.put(k, v)`              |
| `map.get(k)`                 | `map.get(k)`                 |
| `map.containsKey(k)`         | `map.containsKey(k)`         |
| `map.remove(k)`              | `map.remove(k)`              |
| `map.size()`                 | `map.size()`                 |
| `map.isEmpty()`              | `map.isEmpty()`              |

### Set

| Java                          | Rust                          |
|-------------------------------|-------------------------------|
| `new HashSet<>()`            | `JSet::new()`                 |
| `set.add(x)`                | `set.add(x)`                 |
| `set.contains(x)`           | `set.contains(x)`            |
| `set.remove(x)`             | `set.remove(x)`              |
| `set.size()`                | `set.size()`                 |
| `set.isEmpty()`             | `set.isEmpty()`              |

### EnumMap

| Java                                  | Rust                          |
|---------------------------------------|-------------------------------|
| `new EnumMap<>(Key.class)`            | `JEnumMap::new()`             |
| `map.put(k, v)`                      | `map.put(k, v)`              |
| `map.get(k)`                         | `map.get(k)`                 |
| `map.containsKey(k)`                 | `map.containsKey(k)`         |
| `map.remove(k)`                      | `map.remove(k)`              |
| `map.size()`                         | `map.size()`                 |
| `map.isEmpty()`                      | `map.isEmpty()`              |

### EnumSet

| Java                                  | Rust                          |
|---------------------------------------|-------------------------------|
| `EnumSet.noneOf(Color.class)`        | `JEnumSet::new()`             |
| `EnumSet.of(Color.RED, Color.BLUE)`  | `JEnumSet::of(vec![...])`     |
| `set.add(x)`                         | `set.add(x)`                 |
| `set.contains(x)`                    | `set.contains(x)`            |
| `set.remove(x)`                      | `set.remove(x)`              |
| `set.size()`                         | `set.size()`                 |
| `set.isEmpty()`                      | `set.isEmpty()`              |

## Arrays

| Java                          | Rust                               |
|-------------------------------|--------------------------------------|
| `new int[10]`                | `JArray::new_default(10)`            |
| `new int[r][c]`              | `JArray::<JArray<i32>>::new_with(r, \|_\| JArray::<i32>::new_default(c))` |
| `arr[i]`                    | `arr.get(i)`                         |
| `arr[i] = x`                | `arr.set(i, x)`                     |
| `arr.length`                 | `arr.length()`                       |
| `int[] a`                   | `JArray<i32>`                        |
| `int[][] a`                 | `JArray<JArray<i32>>`                |

## Varargs

```java
int sum(int... nums) { return nums.length; }
sum(1, 2, 3);
```
```rust
fn sum(mut nums: JArray<i32>) -> i32 { return nums.length(); }
sum(JArray::from_vec(vec![1_i32, 2, 3]));
```

## Static Fields and Initializers

```java
class Foo {
    static int count = 0;
    static { count = 42; }
    static void inc() { count++; }
}
```
```rust
use ::std::sync::Once;
use ::std::sync::atomic::{AtomicI32, Ordering::SeqCst};
static Foo_count: AtomicI32 = AtomicI32::new(0);
static __STATIC_INIT_ONCE_Foo: Once = Once::new();
impl Foo {
    fn __run_static_init() {
        __STATIC_INIT_ONCE_Foo.call_once(|| {
            Foo_count.store(42, SeqCst);
        });
    }
    fn inc() {
        Self::__run_static_init();
        Foo_count.fetch_add(1, SeqCst);
    }
}
```

## Exception Handling

### throw

```java
throw new IllegalArgumentException("bad input");
```
```rust
panic!("JException:IllegalArgumentException:bad input");
```

### try / catch

```java
try {
    riskyOp();
} catch (ArithmeticException e) {
    System.out.println(e.getMessage());
}
```
```rust
{
    let __result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        riskyOp();
    }));
    match __result {
        Ok(()) => {}
        Err(__e) => {
            let __exc = JException::from_panic(&__e);
            if __exc.is_instance_of("ArithmeticException") {
                let mut e = __exc.clone();
                println!("{}", (e).getMessage());
            } else {
                std::panic::resume_unwind(__e);
            }
        }
    }
}
```

### Multi-catch

```java
catch (IOException | ParseException e) { ... }
```
```rust
if __exc.is_instance_of("IOException") || __exc.is_instance_of("ParseException") {
    ...
}
```

### Finally

`finally` blocks are emitted as code that runs after both the success and error
paths, before any rethrow.

### Try-with-resources

```java
try (FileReader r = new FileReader("f")) { ... }
```

Desugared during parsing into a `LocalVar` + `TryCatch` where the `finally`
block calls `r.close()`.

### throws Declarations

```java
void riskyMethod() throws IOException { ... }
```

The `throws` clause is parsed and stored in the IR. Since exceptions are
implemented using panics, they propagate through method boundaries without
requiring `Result` return types.

## Concurrency

### Thread Creation

```java
Thread t = new Thread(() -> { work(); });
t.start();
t.join();
```
```rust
let mut t = JThread::new(move || { work(); });
t.start();
t.join();
```

### Thread.sleep

```java
Thread.sleep(1000);
```
```rust
JThread::sleep(1000);
```

### synchronized Methods

```java
public synchronized void increment() { count++; }
```
```rust
pub fn increment(&mut self) {
    let __guard = Self::__sync_monitor().0.lock().unwrap();
    (self).count += 1;
}
```

Each class with synchronized methods gets a per-class
`OnceLock<(Mutex<()>, Condvar)>` static monitor.

### synchronized Blocks

`synchronized (this)` and `synchronized (obj)` are both supported.  Each user
class is injected with a `pub __monitor: JMonitor` field that provides its own
`(Mutex, Condvar)` pair.  Subclasses share the same underlying `Arc` as their
`_super` so all composition levels use a single per-object monitor.

`synchronized (obj)` on a non-user-defined type (e.g. `String`, a collection,
or an array) falls back to the process-global `__sync_block_monitor()`.

```java
synchronized (this) { ... }
```
```rust
{
    let __sync_arc = self.__monitor.pair();
    let (__sync_lock, __sync_cond) = &*__sync_arc;
    let mut __sync_guard = __sync_lock.lock().unwrap();
    ...
    drop(__sync_guard);
}
```

```java
synchronized (someObj) { ... }
```
```rust
{
    let __sync_arc = (someObj).__monitor.pair();
    let (__sync_lock, __sync_cond) = &*__sync_arc;
    let mut __sync_guard = __sync_lock.lock().unwrap();
    ...
    drop(__sync_guard);
}
```

Nested `synchronized` blocks on **different** objects work correctly because
each object owns its own mutex.

### volatile Fields

```java
volatile int counter;
```
```rust
pub counter: Arc<AtomicI32>,
// Reads: counter.load(Ordering::SeqCst)
// Writes: counter.store(value, Ordering::SeqCst)
```

### Atomic Types

| Java                          | Rust                               |
|-------------------------------|--------------------------------------|
| `AtomicInteger`              | `JAtomicInteger`                     |
| `AtomicLong`                 | `JAtomicLong`                        |
| `AtomicBoolean`              | `JAtomicBoolean`                     |
| `.get()`                     | `.get()`                             |
| `.set(v)`                    | `.set(v)`                            |
| `.incrementAndGet()`         | `.incrementAndGet()`                 |
| `.getAndIncrement()`         | `.getAndIncrement()`                 |
| `.compareAndSet(expect, update)` | `.compareAndSet(expect, update)` |

### CountDownLatch / Semaphore

| Java                          | Rust                               |
|-------------------------------|--------------------------------------|
| `new CountDownLatch(n)`      | `JCountDownLatch::new(n)`            |
| `latch.countDown()`          | `latch.countDown()`                  |
| `latch.await()`              | `latch.await_latch()`                |
| `new Semaphore(n)`           | `JSemaphore::new(n)`                 |
| `sem.acquire()`              | `sem.acquire()`                      |
| `sem.release()`              | `sem.release()`                      |

### ReentrantLock / Condition

| Java                          | Rust                               |
|-------------------------------|--------------------------------------|
| `new ReentrantLock()`        | `JReentrantLock::new()`              |
| `lock.lock()`               | `lock.lock()`                        |
| `lock.unlock()`             | `lock.unlock()`                      |
| `lock.tryLock()`            | `lock.tryLock()`                     |
| `lock.newCondition()`       | `lock.newCondition()`                |
| `cond.await()`              | `cond.await_()`                      |
| `cond.signal()`             | `cond.signal()`                      |
| `cond.signalAll()`          | `cond.signalAll()`                   |

`JReentrantLock` implements full reentrant semantics: the owning thread may call
`lock()` multiple times and must call a matching number of `unlock()` calls.
`JCondition.await_()` atomically releases all holds on the associated lock,
waits for a `signal`/`signalAll`, and re-acquires the lock before returning.

### ReentrantReadWriteLock

| Java                          | Rust                               |
|-------------------------------|--------------------------------------|
| `new ReentrantReadWriteLock()` | `JReentrantReadWriteLock::new()`   |
| `rwl.readLock()`            | `rwl.readLock()` → `JReadLock`       |
| `rwl.writeLock()`           | `rwl.writeLock()` → `JWriteLock`     |
| `rl.lock()` / `rl.unlock()` | `rl.lock()` / `rl.unlock()`         |
| `wl.lock()` / `wl.unlock()` | `wl.lock()` / `wl.unlock()`         |

### ConcurrentHashMap

| Java                          | Rust                               |
|-------------------------------|--------------------------------------|
| `new ConcurrentHashMap<>()`  | `JConcurrentHashMap::new()`          |
| `map.put(k, v)`             | `map.put(k, v)`                      |
| `map.get(k)`                | `map.get(&k)`                        |
| `map.containsKey(k)`        | `map.containsKey(&k)`               |
| `map.remove(k)`             | `map.remove(&k)`                     |
| `map.putIfAbsent(k, v)`     | `map.putIfAbsent(k, v)`             |
| `map.getOrDefault(k, def)`  | `map.getOrDefault(&k, def)`          |

### CopyOnWriteArrayList

| Java                          | Rust                               |
|-------------------------------|--------------------------------------|
| `new CopyOnWriteArrayList<>()` | `JCopyOnWriteArrayList::new()`     |
| `list.add(e)`               | `list.add(e)`                        |
| `list.get(i)`               | `list.get(i)`                        |
| `list.set(i, e)`            | `list.set(i, e)`                     |
| `list.remove(i)`            | `list.remove_at(i)`                  |
| `list.contains(e)`          | `list.contains(&e)`                  |
| `list.indexOf(e)`           | `list.indexOf(&e)`                   |

### ThreadLocal

| Java                          | Rust                               |
|-------------------------------|--------------------------------------|
| `ThreadLocal.withInitial(() -> v)` | `JThreadLocal::withInitial(\|\| v)` |
| `tl.get()`                  | `tl.get()`                           |
| `tl.set(v)`                 | `tl.set(v)`                          |
| `tl.remove()`               | `tl.remove()`                        |

### ExecutorService / Executors

| Java                          | Rust                               |
|-------------------------------|--------------------------------------|
| `Executors.newFixedThreadPool(n)` | `JExecutors::newFixedThreadPool(n)` |
| `Executors.newSingleThreadExecutor()` | `JExecutors::newSingleThreadExecutor()` |
| `executor.execute(task)`    | `executor.execute(move \|\| task.run())` |
| `executor.submit(task)`     | `executor.submit_runnable(move \|\| task.run())` |
| `executor.shutdown()`       | `executor.shutdown()`                |
| `executor.awaitTermination(t, unit)` | `executor.awaitTermination(unit.toMillis(t))` |
| `executor.isShutdown()`     | `executor.isShutdown()`              |

### Future / CompletableFuture

| Java                          | Rust                               |
|-------------------------------|--------------------------------------|
| `CompletableFuture.supplyAsync(() -> v)` | `JCompletableFuture::supplyAsync(\|\| v)` |
| `CompletableFuture.completedFuture(v)` | `JCompletableFuture::completedFuture(v)` |
| `cf.get()`                  | `cf.get()`                           |
| `cf.join()`                 | `cf.join()`                          |
| `cf.isDone()`               | `cf.isDone()`                        |
| `cf.thenApply(x -> f(x))`  | `cf.thenApply(\|x\| f(x))`          |

### TimeUnit

| Java                          | Rust                               |
|-------------------------------|--------------------------------------|
| `TimeUnit.SECONDS`          | `JTimeUnit::SECONDS`                 |
| `TimeUnit.MILLISECONDS`     | `JTimeUnit::MILLISECONDS`            |
| `unit.toMillis(n)`          | `unit.toMillis(n)`                   |

### StampedLock

| Java                              | Rust                                  |
|-----------------------------------|---------------------------------------|
| `new StampedLock()`              | `JStampedLock::new()`                 |
| `sl.writeLock()`                 | `sl.writeLock()` → `i64` stamp        |
| `sl.unlockWrite(stamp)`          | `sl.unlockWrite(stamp)`               |
| `sl.readLock()`                  | `sl.readLock()` → `i64` stamp         |
| `sl.unlockRead(stamp)`           | `sl.unlockRead(stamp)`                |
| `sl.tryOptimisticRead()`         | `sl.tryOptimisticRead()` → `i64`      |
| `sl.validate(stamp)`             | `sl.validate(stamp)` → `bool`         |

`StampedLock` is backed by a `Mutex<StampedLockState>` + `Condvar`.
`writeLock()` blocks until no readers or writers hold the lock and
increments an internal stamp counter on acquisition.  `tryOptimisticRead()`
returns `0` if a write lock is currently held.

> **Limitation:** `return` inside a `try { ... } finally { sl.unlockWrite(stamp); }`
> block does not work because codegen wraps the body in a `catch_unwind` closure.
> Place `unlockWrite`/`unlockRead` after the try-block instead.

### ForkJoinPool / RecursiveTask

```java
class SumTask extends RecursiveTask<Integer> {
    protected Integer compute() { ... }
}
ForkJoinPool pool = new ForkJoinPool();
int result = pool.invoke(new SumTask(...));
```
```rust
// codegen injects fork()/join() and __fork_handle into SumTask;
// pool.invoke(task) becomes { let mut __fjp_t = task; __fjp_t.compute() }
```

| Java                              | Rust                                  |
|-----------------------------------|---------------------------------------|
| `new ForkJoinPool()`             | `JForkJoinPool::new()`                |
| `ForkJoinPool.commonPool()`      | `JForkJoinPool::commonPool()`         |
| `pool.invoke(task)`              | `task.compute()` (inlined by codegen) |
| `task.fork()`                    | spawns a thread; stores handle        |
| `task.join()`                    | blocks until forked result available  |

`RecursiveAction` (void compute) is also supported; `fork()`/`join()` omit
the result-carrying handle.

## Standard Library

### Math

| Java                    | Rust                              |
|-------------------------|-----------------------------------|
| `Math.abs(x)`           | `(x as f64).abs()` or `x.abs()`  |
| `Math.max(a, b)`        | `a.max(b)` or `f64::max(a, b)`   |
| `Math.min(a, b)`        | `a.min(b)` or `f64::min(a, b)`   |
| `Math.pow(a, b)`        | `(a as f64).powf(b as f64)`      |
| `Math.sqrt(x)`          | `(x as f64).sqrt()`              |
| `Math.floor(x)`         | `(x).floor()`                    |
| `Math.ceil(x)`          | `(x).ceil()`                     |
| `Math.round(x)`         | `(x).round() as i64`             |
| `Math.log(x)`           | `(x).ln()`                       |
| `Math.sin(x)`           | `(x).sin()`                      |
| `Math.cos(x)`           | `(x).cos()`                      |
| `Math.random()`         | `rand::random::<f64>()`          |

### StringBuilder

| Java                            | Rust                               |
|---------------------------------|--------------------------------------|
| `new StringBuilder()`           | `JStringBuilder::new()`             |
| `sb.append(x)`                 | `sb.append(x)`                       |
| `sb.toString()`                | `sb.toString()`                      |
| `sb.length()`                  | `sb.length()`                        |
| `sb.reverse()`                 | `sb.reverse()`                       |

### Optional

| Java                            | Rust                               |
|---------------------------------|--------------------------------------|
| `Optional.of(x)`               | `JOptional::of(x)`                  |
| `Optional.empty()`             | `JOptional::empty()`                |
| `Optional.ofNullable(x)`       | `JOptional::ofNullable(x)`          |
| `opt.isPresent()`              | `opt.isPresent()`                    |
| `opt.get()`                    | `opt.get()`                          |
| `opt.orElse(default)`          | `opt.orElse(default)`                |
| `opt.ifPresent(consumer)`      | `opt.ifPresent(consumer)`            |

### Stream API

| Java                            | Rust                               |
|---------------------------------|--------------------------------------|
| `list.stream()`                 | `JStream::of_list(&list)`           |
| `.filter(predicate)`           | `.filter(predicate)`                 |
| `.map(function)`               | `.map(function)`                     |
| `.sorted()`                    | `.sorted()`                          |
| `.distinct()`                  | `.distinct()`                        |
| `.limit(n)`                    | `.limit(n)`                          |
| `.collect(Collectors.toList())` | `.collect_to_list()`                |
| `.forEach(consumer)`           | `.forEach(consumer)`                 |

### Regex (Pattern / Matcher)

| Java                            | Rust                               |
|---------------------------------|--------------------------------------|
| `Pattern.compile(regex)`        | `JPattern::compile(regex)`           |
| `pattern.matcher(input)`       | `pattern.matcher(input)`             |
| `matcher.find()`               | `matcher.find()`                     |
| `matcher.group()`              | `matcher.group()`                    |
| `matcher.matches()`            | `matcher.matches()`                  |

### BigInteger

| Java                            | Rust                               |
|---------------------------------|--------------------------------------|
| `BigInteger.valueOf(n)`         | `JBigInteger::from_long(n)`         |
| `new BigInteger("123")`        | `JBigInteger::from_string("123")`    |
| `a.add(b)`                     | `a.add(&b)`                          |
| `a.subtract(b)`                | `a.subtract(&b)`                     |
| `a.multiply(b)`                | `a.multiply(&b)`                     |
| `a.divide(b)`                  | `a.divide(&b)`                       |
| `a.mod(b)`                     | `a.mod_(&b)`                         |
| `a.pow(n)`                     | `a.pow(n)`                           |
| `a.compareTo(b)`               | `a.compareTo(&b)`                    |

### LocalDate

| Java                            | Rust                               |
|---------------------------------|--------------------------------------|
| `LocalDate.of(y, m, d)`        | `JLocalDate::of(y, m, d)`           |
| `LocalDate.now()`              | `JLocalDate::now()`                  |
| `date.getYear()`               | `date.getYear()`                     |
| `date.plusDays(n)`              | `date.plusDays(n)`                   |
| `date.plusMonths(n)`            | `date.plusMonths(n)`                 |

### LocalTime

| Java                            | Rust                               |
|---------------------------------|--------------------------------------|
| `LocalTime.of(h, m)`           | `JLocalTime::of_hm(h, m)`           |
| `LocalTime.of(h, m, s)`        | `JLocalTime::of_hms(h, m, s)`       |
| `LocalTime.now()`              | `JLocalTime::now()`                  |
| `LocalTime.parse(s)`           | `JLocalTime::parse(&s)`              |
| `t.getHour()`                  | `t.getHour()`                        |
| `t.plusHours(n)`               | `t.plusHours(n)`                     |
| `t.isBefore(other)`            | `t.isBefore(&other)`                 |

### LocalDateTime

| Java                            | Rust                               |
|---------------------------------|--------------------------------------|
| `LocalDateTime.of(y,m,d,h,mn)` | `JLocalDateTime::of_ymd_hm(y,m,d,h,mn)` |
| `LocalDateTime.now()`          | `JLocalDateTime::now()`              |
| `LocalDateTime.parse(s)`       | `JLocalDateTime::parse(&s)`          |
| `dt.toLocalDate()`             | `dt.toLocalDate()`                   |
| `dt.toLocalTime()`             | `dt.toLocalTime()`                   |
| `dt.plusDays(n)`               | `dt.plusDays(n)`                     |
| `dt.isBefore(other)`           | `dt.isBefore(&other)`                |

### Instant

| Java                            | Rust                               |
|---------------------------------|--------------------------------------|
| `Instant.now()`                | `JInstant::now()`                    |
| `Instant.ofEpochSecond(s)`     | `JInstant::ofEpochSecond(s)`         |
| `Instant.ofEpochMilli(ms)`     | `JInstant::ofEpochMilli(ms)`         |
| `i.toEpochMilli()`             | `i.toEpochMilli()`                   |
| `i.plusSeconds(n)`             | `i.plusSeconds(n)`                   |

### Duration / Period

| Java                            | Rust                               |
|---------------------------------|--------------------------------------|
| `Duration.ofSeconds(s)`        | `JDuration::ofSeconds(s)`            |
| `Duration.ofMillis(ms)`        | `JDuration::ofMillis(ms)`            |
| `Duration.between(a, b)`       | `JDuration::between(&a, &b)`         |
| `d.getSeconds()`               | `d.getSeconds()`                     |
| `d.toMillis()`                 | `d.toMillis()`                       |
| `d.plus(other)`                | `d.plus(&other)`                     |
| `Period.of(y, m, d)`           | `JPeriod::of(y, m, d)`              |
| `Period.between(d1, d2)`       | `JPeriod::between(&d1, &d2)`         |
| `p.getYears()`                 | `p.getYears()`                       |

### DateTimeFormatter

| Java                            | Rust                               |
|---------------------------------|--------------------------------------|
| `DateTimeFormatter.ofPattern(p)`| `JDateTimeFormatter::ofPattern(&p)`  |
| `date.format(formatter)`       | `date.format(&formatter)`            |

### File

| Java                            | Rust                               |
|---------------------------------|--------------------------------------|
| `new File(path)`               | `JFile::new(path)`                   |
| `file.exists()`                | `file.exists()`                      |
| `file.isFile()`                | `file.isFile()`                      |
| `file.isDirectory()`           | `file.isDirectory()`                 |
| `file.length()`                | `file.length()`                      |
| `file.delete()`                | `file.delete()`                      |
| `file.toString()`             | `file.toString()`                    |
| `file.toPath()`               | `file.toPath()`                      |

### BufferedReader / BufferedWriter

| Java                                              | Rust                                                 |
|---------------------------------------------------|------------------------------------------------------|
| `new BufferedReader(new FileReader(path))`         | `JBufferedReader::from_reader(JFileReader::new(path))` |
| `new BufferedReader(new InputStreamReader(System.in))` | `JBufferedReader::new_stdin()`                  |
| `br.readLine()`                                   | `br.readLine()`                                      |
| `br.close()`                                      | `br.close()`                                         |
| `new BufferedWriter(new FileWriter(path))`         | `JBufferedWriter::from_writer(JFileWriter::new(path))` |
| `bw.write(s)`                                     | `bw.write(s)`                                        |
| `bw.newLine()`                                    | `bw.newLine()`                                       |
| `bw.flush()`                                      | `bw.flush()`                                         |
| `bw.close()`                                      | `bw.close()`                                         |

### PrintWriter

| Java                                   | Rust                                         |
|----------------------------------------|----------------------------------------------|
| `new PrintWriter(path)`               | `JPrintWriter::new_from_path(path)`          |
| `new PrintWriter(new FileWriter(f))`  | `JPrintWriter::from_writer(JFileWriter::new(f))` |
| `pw.println(s)`                        | `pw.println(s)`                              |
| `pw.print(s)`                          | `pw.print(s)`                                |
| `pw.flush()`                           | `pw.flush()`                                 |
| `pw.close()`                           | `pw.close()`                                 |

### FileReader / FileWriter

| Java                          | Rust                                 |
|-------------------------------|--------------------------------------|
| `new FileReader(path)`       | `JFileReader::new(path)`             |
| `new FileWriter(path)`       | `JFileWriter::new(path)`             |
| `new FileWriter(path, true)` | `JFileWriter::new_append(path, true)` |

### FileInputStream / FileOutputStream

| Java                                  | Rust                                         |
|---------------------------------------|----------------------------------------------|
| `new FileInputStream(path)`          | `JFileInputStream::new(path)`                |
| `new FileOutputStream(path)`         | `JFileOutputStream::new(path)`               |
| `new FileOutputStream(path, true)`   | `JFileOutputStream::new_append(path, true)`  |
| `fis.read()`                          | `fis.read()`                                 |
| `fos.write(b)`                        | `fos.write_byte(b)`                          |
| `fos.flush()`                         | `fos.flush()`                                |

### Scanner

| Java                              | Rust                                     |
|-----------------------------------|------------------------------------------|
| `new Scanner(new File(path))`    | `JScanner::from_file(&JFile::new(path))`         |
| `new Scanner(string)`            | `JScanner::from_string(string)`          |
| `sc.hasNextLine()`               | `sc.hasNextLine()`                       |
| `sc.nextLine()`                  | `sc.nextLine()`                          |
| `sc.hasNext()`                   | `sc.hasNext()`                           |
| `sc.next()`                      | `sc.next()`                              |
| `sc.hasNextInt()`                | `sc.hasNextInt()`                        |
| `sc.nextInt()`                   | `sc.nextInt()`                           |
| `sc.nextDouble()`               | `sc.nextDouble()`                        |
| `sc.nextLong()`                  | `sc.nextLong()`                          |
| `sc.close()`                     | `sc.close()`                             |

### Path / Paths / Files (java.nio.file)

| Java                                  | Rust                                         |
|---------------------------------------|----------------------------------------------|
| `Paths.get(s)`                       | `JPath::get(s)`                              |
| `path.toString()`                    | `path.toString()`                            |
| `path.getFileName()`                 | `path.getFileName()`                         |
| `path.getParent()`                   | `path.getParent()`                           |
| `path.resolve(other)`               | `path.resolve(other)`                        |
| `path.toAbsolutePath()`             | `path.toAbsolutePath()`                      |
| `path.toFile()`                      | `path.toFile()`                              |
| `Files.readString(path)`            | `JFiles::readString(&path)`                  |
| `Files.writeString(path, content)`  | `JFiles::writeString(&path, content)`        |
| `Files.readAllLines(path)`          | `JFiles::readAllLines(&path)`                |
| `Files.write(path, lines)`          | `JFiles::write_lines(&path, &lines)`         |
| `Files.exists(path)`                | `JFiles::exists(&path)`                      |
| `Files.isDirectory(path)`           | `JFiles::isDirectory(&path)`                 |
| `Files.isRegularFile(path)`         | `JFiles::isRegularFile(&path)`               |
| `Files.size(path)`                  | `JFiles::size(&path)`                        |
| `Files.delete(path)`                | `JFiles::delete(&path)`                      |
| `Files.deleteIfExists(path)`        | `JFiles::deleteIfExists(&path)`              |
| `Files.createDirectory(path)`       | `JFiles::createDirectory(&path)`             |
| `Files.createDirectories(path)`     | `JFiles::createDirectories(&path)`           |
| `Files.copy(src, dst)`              | `JFiles::copy(&src, &dst)`                   |
| `Files.move(src, dst)`              | `JFiles::move_path(&src, &dst)`              |

## Lambda Expressions

```java
list.stream().filter(x -> x > 0).collect(Collectors.toList());
```
```rust
JStream::of_list(&list).filter(|x: &i32| *x > 0).collect_to_list();
```

Lambda expressions are translated to Rust closures (`|params| { body }`).
Multi-statement block lambdas are fully supported:

```java
list.stream().map(x -> {
    int y = x * 2;
    return y + 1;
})
```
```rust
list.stream().map(|x: i32| {
    let mut y: i32 = x * 2;
    y + 1
})
```

## Text Blocks (Java 13+)

```java
String sql = """
        SELECT *
        FROM users
        WHERE id = ?
        """;
```
```rust
let mut sql: JString = JString::from("SELECT *\nFROM users\nWHERE id = ?\n");
```

Common leading indentation is stripped per JEP 378. The level is determined
by the least-indented content line (or the closing `"""` line if it is placed
on its own line with less indentation).

## Map Iteration

```java
for (Map.Entry<String, Integer> entry : map.entrySet()) {
    System.out.println(entry.getKey() + "=" + entry.getValue());
}
```
```rust
for entry in map.entrySet().iter() {
    let entry: JMapEntry<JString, i32> = entry.clone();
    println!("{}", entry);
}
```

| Java method | Rust runtime method |
|-------------|---------------------|
| `map.keySet()` | `map.keySet() → JList<K>` (sorted for `JTreeMap`) |
| `map.values()` | `map.values() → JList<V>` |
| `map.entrySet()` | `map.entrySet() → JList<JMapEntry<K,V>>` |
| `entry.getKey()` | `entry.getKey() → K` |
| `entry.getValue()` | `entry.getValue() → V` |

## Annotations

| Java            | Rust                              |
|-----------------|-----------------------------------|
| `@Override`     | Silently ignored (no Rust equivalent needed) |
| `@Deprecated`   | Silently ignored                  |

Other annotations are parsed and silently tolerated. They do not affect code
generation.

## System.out / System.err

| Java                            | Rust                        |
|---------------------------------|-----------------------------|
| `System.out.println(x)`        | `println!("{}", x)`         |
| `System.out.print(x)`          | `print!("{}", x)`           |
| `System.err.println(x)`        | `eprintln!("{}", x)`        |
| `System.out.printf(fmt, args)` | `print!("{}", jformat(fmt, &[...]))` |

## System Utilities

| Java                            | Rust                                       |
|---------------------------------|--------------------------------------------|
| `System.exit(code)`            | `std::process::exit(code)`                  |
| `System.currentTimeMillis()`   | `SystemTime` millis since epoch             |
| `System.nanoTime()`            | `SystemTime` nanos since epoch              |
| `System.getenv(key)`           | `std::env::var(key).unwrap_or_default()`    |
| `System.getProperty(key)`      | Inline match on known properties            |
| `System.getProperty(key, def)` | Inline match with default fallback          |
| `System.lineSeparator()`       | `JString::from("\n")`                      |

## String.format / String.join

| Java                            | Rust                                       |
|---------------------------------|--------------------------------------------|
| `String.format(fmt, args)`     | `jformat(fmt, &[args...])`                  |
| `String.join(delim, strs)`     | `[strs].join(delim.as_str())`               |

## Main Method

```java
public class HelloWorld {
    public static void main(String[] args) {
        System.out.println("Hello, World!");
    }
}
```
```rust
fn main() {
    let args: JArray<JString> = JArray::from_vec(
        std::env::args().skip(1).map(|s| JString::from(s.as_str())).collect(),
    );
    HelloWorld::main(args);
}
```

## Casting

| Java                    | Rust                              |
|-------------------------|-----------------------------------|
| `(int) doubleVal`       | `double_val as i32`               |
| `(double) intVal`       | `int_val as f64`                  |
| `(long) intVal`         | `int_val as i64`                  |

Primitive casts use Rust's `as` keyword. Reference casts are not currently
supported (see [LIMITATIONS.md](LIMITATIONS.md)).

## Records (Java 16+)

### Basic Record

```java
record Point(int x, int y) {
    String describe() { return "(" + x + "," + y + ")"; }
}
```
```rust
#[derive(Debug, Clone, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}
impl Point {
    pub fn new(mut x: i32, mut y: i32) -> Self { /* assign fields */ }
    pub fn x(&self) -> i32 { self.x }
    pub fn y(&self) -> i32 { self.y }
    pub fn describe(&mut self) -> JString { /* body */ }
}
impl ::std::fmt::Display for Point {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        write!(f, "Point[x={}, y={}]", &self.x, &self.y)
    }
}
```

### Record with Compact Constructor

```java
record Range(int lo, int hi) {
    Range {
        if (lo > hi) throw new IllegalArgumentException("lo > hi");
    }
}
```
```rust
impl Range {
    pub fn new(mut lo: i32, mut hi: i32) -> Self {
        let mut __self__: Self = Self { lo: 0, hi: 0, ..Default::default() };
        // compact body runs first (validation)
        if lo > hi { panic!("JException:IllegalArgumentException:lo > hi") }
        // then implicit field assignments
        (__self__).lo = lo;
        (__self__).hi = hi;
        __self__
    }
}
```
