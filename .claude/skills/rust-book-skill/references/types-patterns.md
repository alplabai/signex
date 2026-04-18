# Structs, Enums, and Pattern Matching

> Source: Rust Book chapters 5, 6, and 19

---

## Structs

### Definition and instantiation

```rust
struct User {
    username:  String,
    email:     String,
    sign_in_count: u64,
    active:    bool,
}

let user1 = User {
    email:         String::from("alice@example.com"),
    username:      String::from("alice"),
    active:        true,
    sign_in_count: 1,
};

// Field init shorthand (when variable name == field name):
fn build_user(email: String, username: String) -> User {
    User { email, username, active: true, sign_in_count: 1 }
}

// Struct update syntax:
let user2 = User {
    email: String::from("bob@example.com"),
    ..user1   // remaining fields from user1
    // NOTE: user1.username is MOVED into user2; user1.username is no longer valid
};
```

### Methods

```rust
struct Rectangle {
    width:  f64,
    height: f64,
}

impl Rectangle {
    // Associated function (no self) — typically used as constructor
    fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }

    // Method with immutable self
    fn area(&self) -> f64 {
        self.width * self.height
    }

    // Method with mutable self
    fn scale(&mut self, factor: f64) {
        self.width  *= factor;
        self.height *= factor;
    }

    // Method consuming self (rare)
    fn into_square(self) -> Self {
        let side = self.width.min(self.height);
        Self::new(side, side)
    }

    fn can_hold(&self, other: &Rectangle) -> bool {
        self.width > other.width && self.height > other.height
    }
}

// Call: Rectangle::new(3.0, 4.0)
// Method: rect.area()
```

### Tuple structs and unit structs

```rust
struct Color(i32, i32, i32);  // tuple struct — access with .0, .1, .2
struct Point(f64, f64, f64);

let c = Color(255, 0, 128);
let r = c.0;

struct AlwaysEqual;   // unit struct — useful for implementing traits
```

### Derived traits

```rust
#[derive(Debug, Clone, PartialEq)]
struct Point {
    x: f64,
    y: f64,
}

let p = Point { x: 1.0, y: 2.0 };
println!("{p:?}");       // Debug output
println!("{p:#?}");      // Pretty-print
let p2 = p.clone();
assert_eq!(p, p2);
```

---

## Enums

```rust
enum Direction { North, South, East, West }

// Enums can hold data:
enum Shape {
    Circle(f64),              // radius
    Rectangle(f64, f64),      // width, height
    Triangle { base: f64, height: f64 },  // named fields
}

let s = Shape::Circle(5.0);
```

### `Option<T>` — the null replacement

```rust
// Standard library definition:
// enum Option<T> { Some(T), None }

let some_number: Option<i32> = Some(42);
let no_number:   Option<i32> = None;

// Must unwrap before use:
let n = some_number.unwrap();            // panics on None
let n = some_number.unwrap_or(0);        // default on None
let n = some_number.unwrap_or_else(|| compute_default());
let n = some_number.expect("should have a number");  // panic msg

// Safe access:
if let Some(v) = some_number { println!("{v}"); }
let doubled = some_number.map(|v| v * 2);           // Option<i32>
let filtered = some_number.filter(|&v| v > 10);     // Option<i32>
let chained = some_number.and_then(|v| Some(v + 1)); // flat map
```

---

## Pattern Matching

### `match`

```rust
match shape {
    Shape::Circle(r)         => println!("Circle r={r}"),
    Shape::Rectangle(w, h)  => println!("Rect {w}x{h}"),
    Shape::Triangle { base, height } => {
        let area = 0.5 * base * height;
        println!("Triangle area={area}");
    }
}
```

**`match` is exhaustive**: all cases must be covered.

### Guards and bindings

```rust
let n = 7;

let msg = match n {
    0       => "zero",
    1..=5   => "one to five",
    n if n % 2 == 0 => "even and > 5",  // guard
    _       => "odd and > 5",            // catch-all
};

// Binding with @:
match n {
    x @ 1..=10 => println!("1-10, got {x}"),
    _           => (),
}
```

### `if let` — single-case match

```rust
if let Some(v) = some_option {
    println!("Got {v}");
} else {
    println!("Nothing");
}
```

### `while let`

```rust
let mut stack = vec![1, 2, 3];
while let Some(top) = stack.pop() {
    println!("{top}");
}
```

### Tuple and struct destructuring

```rust
let (a, b, c) = (1, 2, 3);

let Point { x, y } = Point { x: 1.0, y: 2.0 };
// or with renaming:
let Point { x: px, y: py } = point;

// Ignoring fields:
let Point { x, .. } = point;

// Nested:
let ((a, b), c) = ((1, 2), 3);
```

### Destructuring in function parameters

```rust
fn print_point(&(x, y): &(i32, i32)) {
    println!("({x}, {y})");
}
```

### `matches!` macro

```rust
let x = 'c';
assert!(matches!(x, 'a'..='z'));
assert!(matches!(some_option, Some(x) if x > 0));
```

### Multiple patterns and ranges

```rust
match x {
    1 | 2 | 3   => "one two or three",
    4..=9       => "four through nine",
    _           => "other",
}
```
