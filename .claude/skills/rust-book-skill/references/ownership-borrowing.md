# Ownership, Borrowing, Lifetimes, and Slices

> Source: Rust Book chapters 4 and 10

---

## Ownership

Every Rust program manages memory through the ownership system — no GC, no manual malloc/free.

### Three rules
1. Each value has exactly one owner.
2. There can only be one owner at a time.
3. When the owner goes out of scope, the value is dropped (`Drop` is called).

### Move semantics

```rust
let s1 = String::from("hello");
let s2 = s1;          // s1 is MOVED into s2; s1 is no longer valid
println!("{s1}");     // COMPILE ERROR: borrow of moved value

let s3 = s2.clone();  // deep copy — both s2 and s3 are valid
println!("{s2} {s3}");
```

### Copy types (stack-only, cheap to copy)

Types that implement `Copy` are copied, not moved:
- All integer types (`i32`, `u64`, etc.)
- `f32`, `f64`
- `bool`
- `char`
- Tuples of `Copy` types: `(i32, f64)` is `Copy`
- Arrays of `Copy` types: `[i32; 5]` is `Copy`

`String`, `Vec`, `Box`, etc. do **not** implement `Copy` — they move.

### Ownership and functions

```rust
fn takes_ownership(s: String) { /* s is moved in */ }
fn makes_copy(x: i32) { /* x is copied in */ }

let s = String::from("hello");
takes_ownership(s);  // s moved; no longer valid here

let x = 5;
makes_copy(x);       // x copied; still valid
println!("{x}");     // OK

// Return values also transfer ownership:
fn gives_ownership() -> String {
    String::from("yours")   // moved to caller
}
```

---

## References and Borrowing

### Immutable references

```rust
fn calculate_length(s: &String) -> usize {
    s.len()   // we borrow s, do not own it
}

let s1 = String::from("hello");
let len = calculate_length(&s1);  // pass reference
println!("{s1} has length {len}");  // s1 still valid
```

### Mutable references

```rust
fn change(s: &mut String) {
    s.push_str(", world");
}

let mut s = String::from("hello");
change(&mut s);
```

### Borrowing rules

```rust
let mut s = String::from("hello");

let r1 = &s;    // OK: immutable borrow
let r2 = &s;    // OK: multiple immutable borrows allowed
// let r3 = &mut s;  // ERROR: can't have mut borrow while immutable ones live

println!("{r1} {r2}");   // r1 and r2 last used here — they end

let r3 = &mut s;   // OK: no active borrows above this point
r3.push_str(" world");
```

**Non-Lexical Lifetimes (NLL)**: borrows end when they are last used, not when the scope closes.

### No dangling references

```rust
// This does NOT compile:
fn dangle() -> &String {
    let s = String::from("hello");
    &s   // ERROR: s is dropped, reference would dangle
}

// Correct: return the String (transfer ownership)
fn no_dangle() -> String {
    String::from("hello")
}
```

---

## Slices

Slices are references to a contiguous sequence of elements — they borrow part of a collection.

### String slices

```rust
let s = String::from("hello world");

let hello = &s[0..5];   // or &s[..5]
let world = &s[6..11];  // or &s[6..]
let all   = &s[..];     // whole string

fn first_word(s: &str) -> &str {
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b' ' { return &s[..i]; }
    }
    &s[..]
}

// &str is the type for string literals and string slices
let literal: &str = "hello";   // 'static lifetime, stored in binary
let slice:   &str = &s[..5];   // borrows from s
```

### Array slices

```rust
let a = [1, 2, 3, 4, 5];
let slice: &[i32] = &a[1..3];  // [2, 3]
```

---

## Lifetimes

Lifetimes prevent dangling references. Most lifetimes are inferred (elision rules), but
complex cases require explicit annotation.

### Lifetime annotations

```rust
// 'a is a generic lifetime parameter
// Return value lives at least as long as both inputs:
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}

let s1 = String::from("long string");
let result;
{
    let s2 = String::from("xyz");
    result = longest(s1.as_str(), s2.as_str());
    println!("{result}");  // OK: result used while s2 is alive
}
// println!("{result}");  // ERROR: s2 dropped, result may dangle
```

### Lifetime in structs

```rust
// This struct cannot outlive the reference it holds:
struct Important<'a> {
    part: &'a str,
}

let novel = String::from("Call me Ishmael. Some years ago...");
let first_sentence = novel.split('.').next().unwrap();
let i = Important { part: first_sentence };  // i cannot outlive novel
```

### Lifetime elision rules

The compiler automatically fills in lifetimes when:
1. Each reference parameter gets its own lifetime.
2. If there is exactly one input lifetime, it is assigned to all output lifetimes.
3. If there is a `&self` or `&mut self`, its lifetime is assigned to all outputs.

```rust
// These are equivalent:
fn first_word(s: &str) -> &str { /* ... */ }
fn first_word<'a>(s: &'a str) -> &'a str { /* ... */ }
```

### `'static` lifetime

```rust
let s: &'static str = "I live for the entire program";
// String literals are 'static — stored directly in the binary.
// Avoid using 'static as a "fix" for lifetime errors; usually indicates a design issue.
```

### Lifetime + Trait bounds + Generics

```rust
fn longest_with_announcement<'a, T>(
    x:           &'a str,
    y:           &'a str,
    ann:         T,
) -> &'a str
where
    T: std::fmt::Display,
{
    println!("Announcement: {ann}");
    if x.len() > y.len() { x } else { y }
}
```
