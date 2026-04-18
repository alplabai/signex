# Generics, Traits, and Trait Objects

> Source: Rust Book chapters 10, 18, 20

---

## Generics

### Generic functions

```rust
fn largest<T: PartialOrd>(list: &[T]) -> &T {
    let mut l = &list[0];
    for item in list {
        if item > l { l = item; }
    }
    l
}

largest(&[34, 50, 25, 100]);   // i32
largest(&['y', 'm', 'a']);     // char
```

### Generic structs and enums

```rust
struct Pair<T> {
    first:  T,
    second: T,
}

impl<T> Pair<T> {
    fn new(first: T, second: T) -> Self { Self { first, second } }
}

// Conditional method implementation (only when T: Display + PartialOrd):
impl<T: std::fmt::Display + PartialOrd> Pair<T> {
    fn cmp_display(&self) {
        if self.first >= self.second {
            println!("First is larger: {}", self.first);
        } else {
            println!("Second is larger: {}", self.second);
        }
    }
}
```

### Zero-cost generics

Rust uses **monomorphisation**: the compiler generates a separate concrete function for each
type the generic is used with. No runtime overhead — generics are faster than dynamic dispatch.

---

## Traits

### Defining a trait

```rust
pub trait Summary {
    fn summarize_author(&self) -> String;

    // Default implementation (can be overridden):
    fn summarize(&self) -> String {
        format!("(Read more from {}...)", self.summarize_author())
    }
}
```

### Implementing a trait

```rust
pub struct NewsArticle {
    pub headline: String,
    pub content:  String,
    pub author:   String,
}

impl Summary for NewsArticle {
    fn summarize_author(&self) -> String { self.author.clone() }

    fn summarize(&self) -> String {
        format!("{}, by {}", self.headline, self.author)
    }
}
```

**Orphan rule**: you can implement a trait on a type only if either the trait or the type is defined in your crate.

### Trait as parameter (`impl Trait`)

```rust
// Syntactic sugar for a generic bounded by Summary:
pub fn notify(item: &impl Summary) {
    println!("Breaking news! {}", item.summarize());
}
```

### Trait bounds

```rust
// Explicit trait bound (required when multiple params must be the same type):
pub fn notify<T: Summary>(item1: &T, item2: &T) { /* ... */ }

// Multiple bounds:
fn notify<T: Summary + std::fmt::Display>(item: &T) { /* ... */ }

// where clause (cleaner for complex bounds):
fn some_function<T, U>(t: &T, u: &U) -> String
where
    T: std::fmt::Display + Clone,
    U: Clone + std::fmt::Debug,
{
    format!("{t}")
}
```

### Returning `impl Trait`

```rust
fn make_summarizable() -> impl Summary {
    NewsArticle { /* ... */ }
}
// NOTE: can only return ONE concrete type. For multiple types, use Box<dyn Trait>.
```

### Blanket implementations

```rust
// Standard library does this for all T that implement Display:
impl<T: std::fmt::Display> ToString for T {
    fn to_string(&self) -> String { /* ... */ }
}
// This is why any type implementing Display automatically gets .to_string()
```

---

## Important standard traits

| Trait | Purpose |
|-------|---------|
| `Clone` | Explicit deep copy via `.clone()` |
| `Copy` | Implicit bitwise copy (stack only) |
| `Debug` | Formatting with `{:?}` |
| `Display` | Formatting with `{}` (user-facing) |
| `PartialEq` / `Eq` | `==` and `!=` |
| `PartialOrd` / `Ord` | `<`, `>`, `<=`, `>=` |
| `Hash` | Hashable for `HashMap` keys |
| `Default` | `T::default()` zero/empty value |
| `From<T>` / `Into<T>` | Infallible type conversion |
| `TryFrom<T>` / `TryInto<T>` | Fallible conversion → `Result` |
| `Iterator` | `.next()` and the iterator adapter ecosystem |
| `Deref` / `DerefMut` | `*` operator, smart pointer behaviour |
| `Drop` | Custom cleanup when value goes out of scope |
| `Send` | Type can be transferred across threads |
| `Sync` | `&T` can be shared across threads |

Derive common ones automatically:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
struct MyType { /* ... */ }
```

---

## Trait Objects — Runtime Polymorphism

### `dyn Trait`

```rust
pub trait Draw {
    fn draw(&self);
}

// Vec of DIFFERENT types, all implementing Draw:
let components: Vec<Box<dyn Draw>> = vec![
    Box::new(Button { label: "OK".to_string() }),
    Box::new(Checkbox { checked: true }),
];

for c in &components {
    c.draw();   // dynamic dispatch via vtable
}
```

### `Box<dyn Trait>` vs generics

```rust
// Generics — SAME type at runtime (monomorphised, zero-cost):
fn print_all<T: Display>(items: &[T]) { /* all items must be same type T */ }

// Trait objects — DIFFERENT types at runtime (vtable dispatch, heap alloc):
fn draw_all(items: &[Box<dyn Draw>]) { /* items can be Button, Checkbox, etc. */ }
```

Use `dyn Trait` when:
- You need a heterogeneous collection of different types.
- The concrete type is not known at compile time.
- You're building a plugin system.

Use generics when types are homogeneous or known at compile time — they're faster.

### Object safety rules

A trait is **object-safe** (usable as `dyn Trait`) if:
- Return type is not `Self`.
- No generic type parameters.

```rust
// NOT object-safe (returns Self):
trait Clone { fn clone(&self) -> Self; }

// Object-safe:
trait Draw { fn draw(&self); }
```

---

## Associated Types

```rust
pub trait Iterator {
    type Item;   // associated type
    fn next(&mut self) -> Option<Self::Item>;
}

// Implementor chooses the concrete type:
struct Counter { count: u32 }
impl Iterator for Counter {
    type Item = u32;
    fn next(&mut self) -> Option<u32> {
        if self.count < 5 { self.count += 1; Some(self.count) } else { None }
    }
}
```

Associated types vs generics: use associated types when there's only one logical implementation per type. Use generics when you want multiple implementations (e.g., `From<i32>` and `From<String>` on the same type).

---

## Operator Overloading

```rust
use std::ops::Add;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Point { x: f64, y: f64 }

impl Add for Point {
    type Output = Point;
    fn add(self, other: Point) -> Point {
        Point { x: self.x + other.x, y: self.y + other.y }
    }
}

let p3 = Point { x: 1.0, y: 2.0 } + Point { x: 3.0, y: 4.0 };
// p3 = Point { x: 4.0, y: 6.0 }
```

---

## Newtype pattern

Wrapping a type to implement foreign traits or add type safety:

```rust
struct Meters(f64);
struct Seconds(f64);

// Now you can't accidentally mix meters and seconds!
impl std::fmt::Display for Meters {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}m", self.0)
    }
}
```
