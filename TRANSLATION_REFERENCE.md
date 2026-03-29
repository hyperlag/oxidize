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
| `arr[i]`                    | `arr.get(i)`                         |
| `arr[i] = x`                | `arr.set(i, x)`                     |
| `arr.length`                 | `arr.length()`                       |

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

```java
synchronized (this) { ... }
```
```rust
{
    let (__lock, __cvar) = &*java_compat::__sync_block_monitor();
    let mut __guard = __lock.lock().unwrap();
    ...
}
```

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
