# Error Handling

> Source: Rust Book chapter 9

---

## Two kinds of errors

| Kind | Type | Mechanism |
|------|------|-----------|
| Unrecoverable | Programmer bugs, invariant violations | `panic!` |
| Recoverable | Expected failures (file not found, parse error) | `Result<T, E>` |

---

## `panic!` — unrecoverable errors

```rust
panic!("crash and burn");          // explicit panic
let v = vec![1, 2, 3];
v[99];                             // index out of bounds → panic

// Backtrace:
// RUST_BACKTRACE=1 cargo run
```

Use `panic!` for:
- Invalid state that should never happen (programmer error, broken invariant).
- Tests: `panic!`, `assert!`, `assert_eq!`, `assert_ne!`, `unreachable!`, `unimplemented!`.
- Prototyping: `.unwrap()` / `.expect()` until proper error handling is added.

---

## `Result<T, E>` — recoverable errors

```rust
enum Result<T, E> {
    Ok(T),
    Err(E),
}
```

### Basic usage

```rust
use std::fs::File;
use std::io;

let f = File::open("hello.txt");
let f = match f {
    Ok(file) => file,
    Err(e)   => panic!("Problem opening file: {e:?}"),
};

// Shorthand: panics with message on Err
let f = File::open("hello.txt").unwrap();
let f = File::open("hello.txt").expect("Failed to open hello.txt");
```

### Matching on error kind

```rust
use std::io::ErrorKind;

let f = File::open("hello.txt").unwrap_or_else(|e| {
    if e.kind() == ErrorKind::NotFound {
        File::create("hello.txt").expect("Failed to create file")
    } else {
        panic!("Problem opening file: {e:?}");
    }
});
```

### Propagating errors with `?`

```rust
fn read_username() -> Result<String, io::Error> {
    let mut s = String::new();
    File::open("hello.txt")?.read_to_string(&mut s)?;
    Ok(s)
}
// ? = if Ok, unwrap; if Err, return Err from current function
// ? also calls .into() to convert error types via From trait
```

`?` can be chained:
```rust
fn read_first_line(path: &str) -> Result<String, io::Error> {
    let content = fs::read_to_string(path)?;
    let first = content.lines().next().unwrap_or("").to_string();
    Ok(first)
}
```

`?` in `main`:
```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let s = fs::read_to_string("hello.txt")?;
    println!("{s}");
    Ok(())
}
```

---

## `Option<T>` — absence of value

```rust
let some = Some(42_i32);
let none: Option<i32> = None;

// Unwrapping:
some.unwrap()                     // panics on None
some.unwrap_or(0)                 // default value
some.unwrap_or_else(|| compute()) // lazy default
some.expect("should have value")  // panic with message

// Mapping:
let doubled = some.map(|v| v * 2);         // Option<i32>
let parsed  = some.map(|v| v.to_string()); // Option<String>

// Filtering:
let big = some.filter(|&v| v > 10);        // Option<i32>

// Chaining (flat_map):
let s: Option<String> = some.and_then(|v| if v > 0 { Some(v.to_string()) } else { None });

// or / or_else:
let fallback = none.or(Some(99));          // Some(99)
let fallback = none.or_else(|| Some(99));  // lazy

// Converting to Result:
some.ok_or("missing value")               // Result<i32, &str>
some.ok_or_else(|| "missing value")

// is_some / is_none:
if some.is_some() { /* ... */ }
```

### Iterating over `Option`

```rust
for val in some { println!("{val}"); }    // prints once; None = empty iter
let v: Vec<i32> = some.into_iter().collect();
```

---

## Custom error types

### Simple: using `Box<dyn Error>`

```rust
use std::error::Error;

fn run() -> Result<(), Box<dyn Error>> {
    let n: i32 = "42".parse()?;    // ParseIntError auto-converted
    let f = File::open("x")?;      // io::Error auto-converted
    Ok(())
}
```

### Production: `thiserror` crate

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(#[from] std::num::ParseIntError),

    #[error("Not found: {0}")]
    NotFound(String),
}

fn get_count(path: &str) -> Result<i32, AppError> {
    let s = fs::read_to_string(path)?;   // io::Error → AppError::Io
    let n: i32 = s.trim().parse()?;     // ParseIntError → AppError::Parse
    Ok(n)
}
```

### `anyhow` for application code

```rust
// Cargo.toml: anyhow = "1"
use anyhow::{Context, Result};

fn run() -> Result<()> {
    let n = "42".parse::<i32>()
        .context("parsing number from config")?;
    Ok(())
}
```

---

## When to `unwrap` / `expect`

| Situation | Action |
|-----------|--------|
| Prototype / script | `.unwrap()` is fine |
| You know it can't fail | `.expect("reason")` |
| Tests | `unwrap()` or `assert!` |
| Library / production | Proper `Result` propagation |
| Invariant that "should never fail" | `.expect("invariant violation: ...")` |

---

## `?` operator requirements

`?` can only be used in functions that return `Result<_, E>`, `Option<T>`,
or any type that implements `std::ops::FromResidual`.

To use `?` on `Option` inside a `Result`-returning function:
```rust
let val = opt_value.ok_or(MyError::Missing)?;
```
