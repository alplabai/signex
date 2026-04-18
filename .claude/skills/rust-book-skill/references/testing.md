# Testing

> Source: Rust Book chapter 11

---

## Unit tests

```rust
// Place unit tests in the same file as the code they test,
// inside a `tests` module with `#[cfg(test)]`:

pub fn add(a: i32, b: i32) -> i32 { a + b }

pub fn divide(a: f64, b: f64) -> Option<f64> {
    if b == 0.0 { None } else { Some(a / b) }
}

#[cfg(test)]
mod tests {
    use super::*;   // import everything from parent module

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    fn test_divide_by_zero() {
        assert_eq!(divide(10.0, 0.0), None);
    }

    #[test]
    fn test_divide() {
        let result = divide(10.0, 2.0).unwrap();
        assert!((result - 5.0).abs() < f64::EPSILON);
    }
}
```

---

## Assertion macros

```rust
assert!(condition)                        // panics if false
assert!(condition, "msg {}", val)         // with message

assert_eq!(left, right)                   // panics if != (shows both values)
assert_eq!(left, right, "msg {}", val)

assert_ne!(left, right)                   // panics if ==

// Floating point — never use assert_eq! with f64:
let x = 0.1 + 0.2;
assert!((x - 0.3).abs() < 1e-10);        // correct
// assert_eq!(x, 0.3);                   // WRONG: floating point imprecision
```

---

## Test attributes

```rust
#[test]
fn normal_test() { /* ... */ }

// Expected panic:
#[test]
#[should_panic]
fn test_panics() {
    panic!("this should panic");
}

#[test]
#[should_panic(expected = "index out of bounds")]
fn test_specific_panic() {
    let v = vec![1, 2, 3];
    v[99];   // panics with "index out of bounds"
}

// Ignored (skipped by default, run with --ignored):
#[test]
#[ignore]
fn expensive_test() { /* slow test */ }

// Return Result from test:
#[test]
fn test_with_result() -> Result<(), String> {
    let n: i32 = "42".parse().map_err(|e| format!("{e}"))?;
    assert_eq!(n, 42);
    Ok(())
}
```

---

## Running tests

```bash
cargo test                      # run all tests
cargo test test_name            # run tests matching "test_name"
cargo test --lib                # only unit tests (not integration)
cargo test --test integration   # only a specific integration test file
cargo test -- --ignored         # run only ignored tests
cargo test -- --include-ignored # run all tests including ignored
cargo test -- --nocapture       # print stdout (suppressed by default)
cargo test -- --test-threads=1  # run single-threaded
cargo test -- --show-output     # show output of passing tests too
```

---

## Integration tests

Integration tests live in `tests/` directory and test the public API:

```
my_project/
  src/
    lib.rs
  tests/
    integration_test.rs
    another_test.rs
```

```rust
// tests/integration_test.rs
use my_project;    // import the crate being tested

#[test]
fn it_adds_two() {
    assert_eq!(my_project::add(2, 2), 4);
}
```

**Note**: Integration tests can only call `pub` items. No `#[cfg(test)]` needed — the file only compiles during testing.

### Shared test helpers

```rust
// tests/common/mod.rs   (or tests/common.rs in Rust 2018+)
pub fn setup() {
    // shared setup code
}

// tests/integration_test.rs
mod common;

#[test]
fn test_something() {
    common::setup();
    // ...
}
```

---

## Doc tests

Code examples in documentation are compiled and run as tests:

```rust
/// Adds two numbers together.
///
/// # Examples
///
/// ```
/// let result = my_project::add(2, 3);
/// assert_eq!(result, 5);
/// ```
///
/// # Panics
///
/// Never panics.
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

```bash
cargo test --doc    # run only doc tests
```

Doc test tips:
```rust
/// ```rust,should_panic
/// panic!("this should panic");
/// ```

/// ```rust,no_run
/// // Compiled but not executed (e.g., needs network):
/// make_network_call();
/// ```

/// ```rust,ignore
/// // Not compiled (e.g., not valid standalone code):
/// let x = ...;
/// ```
```

---

## Test organisation best practices

### Arrange-Act-Assert (AAA) pattern

```rust
#[test]
fn test_user_creation() {
    // Arrange
    let name  = "Alice";
    let email = "alice@example.com";

    // Act
    let user = User::new(name, email);

    // Assert
    assert_eq!(user.name,  name);
    assert_eq!(user.email, email);
    assert!(user.is_active);
}
```

### Testing error paths

```rust
#[test]
fn test_parse_error() {
    let result: Result<i32, _> = "not a number".parse();
    assert!(result.is_err());
}

#[test]
fn test_option_none() {
    let map: HashMap<&str, i32> = HashMap::new();
    assert_eq!(map.get("missing"), None);
}
```

### Using `assert_eq!` with descriptive messages

```rust
assert_eq!(
    actual,
    expected,
    "Expected {expected} but got {actual} after processing '{input}'",
    input = "some input"
);
```

### Testing with `#[derive(PartialEq, Debug)]`

`assert_eq!` requires both `PartialEq` (for comparison) and `Debug` (to print values on failure):

```rust
#[derive(Debug, PartialEq)]
struct Point { x: f64, y: f64 }

#[test]
fn test_point() {
    let p = Point { x: 1.0, y: 2.0 };
    assert_eq!(p, Point { x: 1.0, y: 2.0 });
}
```

---

## Property-based testing (`proptest` / `quickcheck`)

```toml
[dev-dependencies]
proptest = "1"
```

```rust
use proptest::prelude::*;

fn reverse<T: Clone>(v: &[T]) -> Vec<T> {
    let mut r = v.to_vec();
    r.reverse();
    r
}

proptest! {
    #[test]
    fn reverse_twice_is_identity(v: Vec<i32>) {
        let twice = reverse(&reverse(&v));
        prop_assert_eq!(twice, v);
    }
}
```

---

## Benchmarks

Stable Rust: use `criterion` crate.

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "my_benchmark"
harness = false
```

```rust
// benches/my_benchmark.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 0, 1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("fib 20", |b| b.iter(|| fibonacci(black_box(20))));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
```

```bash
cargo bench
```
