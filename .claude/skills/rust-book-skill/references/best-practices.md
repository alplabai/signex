# Best Practices and Idioms

> Derived from: Rust Book chapters 1–20, Rust API Guidelines,
> Rust Performance Book, and community idioms.

---

## API Design

### Accept `&str`, return `String`

```rust
// GOOD: accept both &str and String via coercion
fn greet(name: &str) { println!("Hello, {name}!"); }

greet("Alice");                      // &str literal
greet(&String::from("Bob"));         // &String coerces

// BAD: forces callers to allocate
fn greet(name: String) { /* ... */ }
```

### Accept `impl Trait` instead of concrete types

```rust
// GOOD: accepts Vec, slice, any iterable
fn sum_items(items: impl Iterator<Item = i32>) -> i32 {
    items.sum()
}

// Also good for Display:
fn log(msg: impl std::fmt::Display) { println!("{msg}"); }
```

### Use `Into<T>` for ergonomic constructors

```rust
struct Config {
    name: String,
    value: i32,
}

impl Config {
    fn new(name: impl Into<String>, value: i32) -> Self {
        Config { name: name.into(), value }
    }
}

Config::new("timeout", 30);           // &str works
Config::new(String::from("retry"), 3); // String works too
```

### Return `impl Trait` to hide implementation details

```rust
fn make_iterator() -> impl Iterator<Item = u32> {
    (1..=10).filter(|n| n % 2 == 0)
}
// Caller doesn't need to know the concrete type
```

### Builder pattern for complex constructors

```rust
#[derive(Default)]
struct RequestBuilder {
    url:     String,
    timeout: u64,
    headers: Vec<(String, String)>,
}

impl RequestBuilder {
    fn url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }
    fn timeout(mut self, secs: u64) -> Self {
        self.timeout = secs;
        self
    }
    fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((key.into(), value.into()));
        self
    }
    fn build(self) -> Request { Request { /* ... */ } }
}

let request = RequestBuilder::default()
    .url("https://example.com")
    .timeout(30)
    .header("Content-Type", "application/json")
    .build();
```

---

## Ownership idioms

### Prefer borrowing over cloning

```rust
// EXPENSIVE: clones the whole string
fn process(s: String) { println!("{s}"); }
process(my_string.clone());

// FREE: borrows
fn process(s: &str) { println!("{s}"); }
process(&my_string);
```

### Clone only when you need independent ownership

```rust
// Storing in a struct that needs to own its data:
struct Cache { data: String }
let cache = Cache { data: input.to_string() };  // .to_string() clones

// Multiple threads need their own copy:
let s = String::from("hello");
let h = std::thread::spawn(move || s);   // move, not clone
```

### Use `Cow<str>` for conditionally-owned strings

```rust
use std::borrow::Cow;

fn maybe_modify(input: &str, should_modify: bool) -> Cow<str> {
    if should_modify {
        Cow::Owned(input.to_uppercase())   // allocates only when needed
    } else {
        Cow::Borrowed(input)               // zero-cost borrow
    }
}
```

---

## Error handling

### Use `?` everywhere in production code

```rust
// BAD: swallows errors, panics unexpectedly
fn load_config() -> Config {
    let content = std::fs::read_to_string("config.toml").unwrap();
    toml::from_str(&content).unwrap()
}

// GOOD: propagates errors to caller
fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string("config.toml")?;
    let config  = toml::from_str(&content)?;
    Ok(config)
}
```

### Use `thiserror` in libraries, `anyhow` in applications

```toml
# Library crate:
[dependencies]
thiserror = "1"

# Application / binary:
[dependencies]
anyhow = "1"
```

### Add context to errors

```rust
use anyhow::Context;

let config = load_config()
    .context("failed to load application config")?;

let user = db.get_user(id)
    .with_context(|| format!("user {id} not found"))?;
```

### Don't use `.unwrap()` in library code

```rust
// BAD: library panics, crashing caller's program
pub fn parse_port(s: &str) -> u16 {
    s.parse().unwrap()   // panics on bad input
}

// GOOD: let caller handle the error
pub fn parse_port(s: &str) -> Result<u16, std::num::ParseIntError> {
    s.parse()
}
```

---

## Performance idioms

### Avoid unnecessary allocations

```rust
// ALLOCATION: format! always allocates
let s = format!("Hello, {}!", name);

// ZERO ALLOC: write! to a pre-allocated buffer
use std::fmt::Write;
let mut s = String::with_capacity(64);
write!(s, "Hello, {}!", name).unwrap();
```

### Pre-allocate collections

```rust
// BAD: many reallocations as vector grows
let mut v: Vec<i32> = Vec::new();
for i in 0..1000 { v.push(i); }

// GOOD: one allocation
let mut v = Vec::with_capacity(1000);
for i in 0..1000 { v.push(i); }

// BEST: collect from iterator (compiler may optimise)
let v: Vec<i32> = (0..1000).collect();
```

### Use iterators over index loops

```rust
// SLOWER + error-prone:
for i in 0..v.len() {
    println!("{}", v[i]);  // bounds check every iteration
}

// FASTER + idiomatic:
for item in &v { println!("{item}"); }
for item in v.iter() { println!("{item}"); }
```

### Avoid cloning in hot paths

```rust
// Every call clones the entire string:
fn get_name(&self) -> String { self.name.clone() }

// Returns a reference — zero cost:
fn get_name(&self) -> &str { &self.name }
fn get_name(&self) -> &String { &self.name }
```

### Use `Box<[T]>` instead of `Vec<T>` for fixed-size data

```rust
// Vec has 3 words (ptr, len, capacity)
let v: Vec<i32> = vec![1, 2, 3];

// Box<[T]> has 2 words (ptr, len) and signals "this won't grow"
let b: Box<[i32]> = vec![1, 2, 3].into_boxed_slice();
```

---

## Naming conventions

| Item | Convention | Example |
|------|-----------|---------|
| Types (struct, enum, trait) | `UpperCamelCase` | `HttpClient` |
| Functions, methods | `snake_case` | `parse_input` |
| Variables, fields | `snake_case` | `user_name` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_SIZE` |
| Static | `SCREAMING_SNAKE_CASE` | `GLOBAL_CONFIG` |
| Modules, crates | `snake_case` | `my_crate` |
| Lifetimes | Short lowercase | `'a`, `'conn` |
| Type params | Short `UpperCamelCase` | `T`, `K`, `V`, `Item` |
| Trait methods | `snake_case` | `to_string()`, `is_empty()` |
| Conversion methods | `as_`, `to_`, `into_` | `as_str()`, `to_string()`, `into_vec()` |

---

## Common patterns

### Newtype pattern

```rust
struct Kilometres(f64);
struct Miles(f64);

// You can't accidentally pass Miles where Kilometres is expected.
fn drive(distance: Kilometres) { /* ... */ }
```

### Type-state pattern

```rust
struct Connection<State> {
    socket: TcpStream,
    _state: std::marker::PhantomData<State>,
}

struct Disconnected;
struct Connected;

impl Connection<Disconnected> {
    fn connect(self) -> Result<Connection<Connected>, io::Error> { /* ... */ }
}

impl Connection<Connected> {
    fn send(&mut self, data: &[u8]) { /* ... */ }
    fn disconnect(self) -> Connection<Disconnected> { /* ... */ }
}
// compile-time guarantee: can't call send() on a disconnected connection
```

### Extension traits

```rust
trait StrExt {
    fn word_count(&self) -> usize;
}

impl StrExt for str {
    fn word_count(&self) -> usize {
        self.split_whitespace().count()
    }
}

"hello world rust".word_count()   // 3
```

---

## Common mistakes and how to fix them

### Mistake 1: Fighting the borrow checker with clones

```rust
// FIGHT: cloning to avoid compiler errors
let s = string.clone();
let t = string.clone();
process(s, t);

// SOLUTION: restructure to avoid overlapping borrows
process(&string[..5], &string[5..]);
```

### Mistake 2: Using `unwrap()` / `expect()` in production

```rust
// BAD:
let n: i32 = user_input.parse().unwrap();

// GOOD:
let n: i32 = user_input.parse().map_err(|e| AppError::Parse(e))?;
```

### Mistake 3: `Vec<String>` when `Vec<&str>` works

```rust
// EXPENSIVE: each word is a heap allocation
let words: Vec<String> = text.split_whitespace()
    .map(|w| w.to_string())
    .collect();

// FREE: references into `text`
let words: Vec<&str> = text.split_whitespace().collect();
```

### Mistake 4: Not using `if let` / `while let`

```rust
// VERBOSE:
match value {
    Some(v) => use_value(v),
    None    => {},
}

// CONCISE:
if let Some(v) = value { use_value(v); }
```

### Mistake 5: Incorrect float comparison

```rust
// WRONG:
assert_eq!(0.1 + 0.2, 0.3);   // fails!

// CORRECT:
let epsilon = 1e-10;
assert!((0.1 + 0.2 - 0.3).abs() < epsilon);
```

### Mistake 6: Holding a MutexGuard across an await point

```rust
// DEADLOCK / WON'T COMPILE:
async fn bad() {
    let guard = mutex.lock().await;
    some_async_fn().await;  // guard is held across await — blocks other threads
    drop(guard);
}

// CORRECT: drop the guard before awaiting
async fn good() {
    {
        let mut guard = mutex.lock().await;
        *guard += 1;
    }  // guard dropped here
    some_async_fn().await;
}
```

### Mistake 7: Reference cycles with `Rc`

```rust
// MEMORY LEAK: a → b → a (neither will ever be freed)
let a = Rc::new(RefCell::new(Node { next: None }));
let b = Rc::new(RefCell::new(Node { next: Some(Rc::clone(&a)) }));
a.borrow_mut().next = Some(Rc::clone(&b));  // cycle!

// FIX: use Weak for back-references
struct Node { next: Option<Rc<Node>>, parent: Option<Weak<Node>> }
```

---

## Clippy lints to know

```bash
cargo clippy -- -D warnings   # treat clippy warnings as errors
```

Common lints:
- `clippy::needless_return` — remove explicit `return` at end of function
- `clippy::clone_on_ref_ptr` — unnecessary `.clone()` on reference
- `clippy::redundant_clone` — `.clone()` that could be avoided
- `clippy::map_unwrap_or` — use `map_or` instead of `.map().unwrap_or()`
- `clippy::option_if_let_else` — use `map_or_else` instead of `if let`
- `clippy::large_enum_variant` — put large variants behind `Box`
- `clippy::single_match` — use `if let` instead of single-arm `match`

---

## Performance profiling checklist

1. Always profile with `--release` builds.
2. Use `criterion` for microbenchmarks.
3. Use `perf`, `flamegraph`, or `cargo-flamegraph` for profiling.
4. Check for unnecessary clones with `cargo clippy`.
5. Check allocator pressure with `heaptrack` or `DHAT`.
6. Use `#[inline]` or `#[inline(always)]` for hot small functions (rarely needed).
7. Avoid `Box<dyn Trait>` in hot paths — use generics (`impl Trait`) for zero-cost dispatch.
8. Prefer `Vec` over `LinkedList` — cache-friendly sequential access is almost always faster.
9. Use `HashMap::with_capacity(n)` when the size is known ahead of time.
10. Consider `FxHashMap` (from `rustc-hash`) for faster hashing when security is not needed.
