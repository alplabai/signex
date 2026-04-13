---
name: rust-lang-book
description: >
  The Rust Programming Language kitabının tamamını kapsayan kapsamlı Rust
  referansı. Ownership, borrowing, lifetimes, structs, enums, pattern matching,
  generics, traits, trait objects, closures, iterators, error handling
  (Result/Option/?), smart pointers (Box/Rc/Arc/RefCell/Weak), fearless
  concurrency (threads/channels/Mutex), async/await, futures, streams,
  modules/crates/Cargo, testing, best practices ve common idioms içerir.
  "rust ownership", "rust borrowing", "rust lifetime", "rust trait",
  "rust generics", "rust error handling", "rust closures", "rust iterators",
  "rust smart pointers", "rust async", "rust concurrency", "rust struct enum"
  gibi ifadelerde tetiklenmeli. Kaynak: doc.rust-lang.org/book (Rust 1.90+,
  edition 2024).
---

# The Rust Programming Language — Complete Reference

> Source: doc.rust-lang.org/book — Rust 1.90+, edition 2024

---

## Reference map

| Topic | File |
|-------|------|
| Ownership, Borrowing, Lifetimes, Slices | `references/ownership-borrowing.md` |
| Structs, Enums, Pattern Matching | `references/types-patterns.md` |
| Generics, Traits, Trait Objects | `references/generics-traits.md` |
| Collections, Strings, HashMap | `references/collections.md` |
| Error Handling — Result, Option, `?` | `references/error-handling.md` |
| Closures, Iterators, Functional Style | `references/closures-iterators.md` |
| Smart Pointers — Box, Rc, Arc, RefCell | `references/smart-pointers.md` |
| Concurrency — threads, Mutex, channels | `references/concurrency.md` |
| Async / Await, Futures, Streams | `references/async-await.md` |
| Modules, Crates, Cargo | `references/modules-cargo.md` |
| Testing | `references/testing.md` |
| Best Practices and Idioms | `references/best-practices.md` |

---

## Core language at a glance

```rust
// Variables: immutable by default
let x = 5;
let mut y = 10;   // mutable

// Types inferred or explicit
let z: f64 = 3.14;

// Functions
fn add(a: i32, b: i32) -> i32 {
    a + b   // no semicolon = expression, returns value
}

// Control flow
if x > 0 { "positive" } else { "non-positive" };

// Loops
for i in 0..5 { println!("{i}"); }
while y > 0 { y -= 1; }
loop { break; }                    // infinite loop with break

// Ownership: each value has one owner
let s1 = String::from("hello");
let s2 = s1;                       // s1 MOVED to s2; s1 invalid
let s3 = s2.clone();               // explicit deep copy

// References (borrowing)
let len = calculate_length(&s2);   // lend, don't move
fn calculate_length(s: &String) -> usize { s.len() }
```

---

## Type system overview

| Category | Types |
|----------|-------|
| **Scalars** | `i8..i128`, `u8..u128`, `isize`, `usize`, `f32`, `f64`, `bool`, `char` |
| **Compound** | tuples `(i32, f64)`, arrays `[i32; 5]` |
| **Heap** | `String`, `Vec<T>`, `HashMap<K,V>`, `Box<T>` |
| **Ref types** | `&T` (immutable ref), `&mut T` (mutable ref) |
| **Smart ptr** | `Box<T>`, `Rc<T>`, `Arc<T>`, `RefCell<T>` |
| **Enums** | `Option<T>`, `Result<T,E>`, user-defined |

---

## The three rules of ownership

1. Each value in Rust has an **owner**.
2. There can only be **one owner at a time**.
3. When the owner goes **out of scope**, the value is dropped.

---

## Borrowing rules (compile-time enforced)

At any given time, you can have **either**:
- **Any number of immutable references** (`&T`) — no mutations allowed, but many readers OK
- **Exactly one mutable reference** (`&mut T`) — exclusive write access

References must always be valid (no dangling pointers).

---

## The `?` operator (error propagation)

```rust
fn read_file() -> Result<String, io::Error> {
    let content = fs::read_to_string("file.txt")?;  // ? = return Err if Err
    Ok(content)
}
// Equivalent to:
// match fs::read_to_string("file.txt") {
//     Ok(v)  => v,
//     Err(e) => return Err(e.into()),
// }
```

---

## Critical rules summary

1. **Immutable by default** — `let mut` required for mutation.
2. **Move semantics** — assignment moves non-`Copy` types; use `.clone()` for explicit copy.
3. **Copy types** — scalar types and tuples of scalars are `Copy`; no move occurs.
4. **No null** — use `Option<T>` instead; `None` not `null`.
5. **No exceptions** — use `Result<T,E>`; propagate with `?`.
6. **Pattern matching is exhaustive** — `match` must cover all cases.
7. **Trait bounds** instead of interfaces — `impl Trait` and `where T: Trait`.
8. **`dyn Trait`** for runtime polymorphism (trait objects); `impl Trait` for compile-time (monomorphisation).
9. **`Rc<T>`** for single-thread shared ownership; **`Arc<T>`** for multi-thread.
10. **`RefCell<T>`** moves borrow-checking to runtime — panics instead of compiler errors.
