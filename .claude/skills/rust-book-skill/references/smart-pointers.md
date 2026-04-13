# Smart Pointers

> Source: Rust Book chapter 15

---

## Overview

Smart pointers implement `Deref` (for `*` dereferencing) and/or `Drop` (for cleanup).

| Type | Ownership | Thread-safe | Mutability | Use when |
|------|-----------|-------------|-----------|----------|
| `Box<T>` | Single | Yes | Follows T | Heap allocation, recursive types, large values |
| `Rc<T>` | Shared | **No** | Immutable | Multiple owners, single thread |
| `Arc<T>` | Shared | Yes | Immutable | Multiple owners, multiple threads |
| `RefCell<T>` | Single | **No** | Interior mutability | Runtime borrow checking |
| `Cell<T>` | Single | **No** | Interior mutability | `Copy` types only, no refs |
| `Mutex<T>` | Shared (with `Arc`) | Yes | Interior mutability | Thread-safe mutation |
| `RwLock<T>` | Shared (with `Arc`) | Yes | Interior mutability | Many readers or one writer |

---

## `Box<T>` — heap allocation

```rust
let b = Box::new(5);   // integer on the heap
println!("{}", *b);    // auto-deref also works: println!("{b}")

// Use cases:

// 1. Recursive types (otherwise compiler can't know the size):
#[derive(Debug)]
enum List {
    Cons(i32, Box<List>),
    Nil,
}
let list = List::Cons(1, Box::new(List::Cons(2, Box::new(List::Nil))));

// 2. Trait objects:
let shape: Box<dyn Draw> = Box::new(Circle { radius: 5.0 });

// 3. Large data moved to heap without copying:
let large = Box::new([0u8; 1_000_000]);
```

---

## `Rc<T>` — single-threaded reference counting

```rust
use std::rc::Rc;

let a = Rc::new(5);
let b = Rc::clone(&a);   // increment ref count; NOT a deep copy
let c = Rc::clone(&a);

println!("count = {}", Rc::strong_count(&a));  // 3

// Rc only allows immutable access:
println!("{}", *a);   // OK
// *a = 10;           // ERROR: cannot mutate through Rc

// When the last Rc drops, the value is freed.
```

### `Rc<RefCell<T>>` — shared mutable data (single thread)

```rust
use std::rc::Rc;
use std::cell::RefCell;

let shared = Rc::new(RefCell::new(vec![1, 2, 3]));
let clone1 = Rc::clone(&shared);
let clone2 = Rc::clone(&shared);

clone1.borrow_mut().push(4);   // OK: runtime borrow check
clone2.borrow_mut().push(5);   // OK: first borrow dropped

println!("{:?}", shared.borrow());  // [1, 2, 3, 4, 5]
```

---

## `Arc<T>` — thread-safe reference counting

```rust
use std::sync::Arc;
use std::thread;

let a = Arc::new(vec![1, 2, 3]);
let a_clone = Arc::clone(&a);

let handle = thread::spawn(move || {
    println!("{:?}", a_clone);
});
handle.join().unwrap();
println!("{:?}", a);
```

`Arc<T>` is identical to `Rc<T>` but uses atomic operations — slightly more expensive.
Use `Arc` only when you need cross-thread sharing.

### `Arc<Mutex<T>>` — shared mutable data across threads

```rust
use std::sync::{Arc, Mutex};
use std::thread;

let counter = Arc::new(Mutex::new(0));
let mut handles = vec![];

for _ in 0..10 {
    let c = Arc::clone(&counter);
    let h = thread::spawn(move || {
        let mut num = c.lock().unwrap();
        *num += 1;
    });   // lock released when `num` goes out of scope (MutexGuard: Drop)
    handles.push(h);
}

for h in handles { h.join().unwrap(); }
println!("Result: {}", *counter.lock().unwrap());  // 10
```

---

## `RefCell<T>` — interior mutability

```rust
use std::cell::RefCell;

let data = RefCell::new(vec![1, 2, 3]);

// Immutable borrow:
let r1 = data.borrow();         // Ref<Vec<i32>>
let r2 = data.borrow();         // OK: multiple immutable borrows
println!("{:?}", *r1);
drop(r1); drop(r2);             // borrows released

// Mutable borrow:
let mut m = data.borrow_mut();  // RefMut<Vec<i32>>
m.push(4);
drop(m);                        // MUST drop before another borrow

// PANIC at runtime if rules violated:
// let m1 = data.borrow_mut();
// let m2 = data.borrow_mut(); // panic: already mutably borrowed
```

**When to use RefCell**: when you know your logic is correct but the compiler can't prove it.
Classic example: mock objects in tests that need to record calls through an `&self` method.

### `Cell<T>` — for `Copy` types

```rust
use std::cell::Cell;

let c = Cell::new(5);
c.set(10);         // no borrow — copies in and out
let v = c.get();   // 10 — returns a copy, not a reference
```

---

## `Deref` trait

```rust
let b = Box::new(5);
let r = &b;           // &Box<i32>
let v = **r;          // deref Box, then deref &
// Rust auto-derefs in most contexts: b + 1 works directly
```

### Deref coercion

Rust automatically inserts `*` as needed:
- `&Box<String>` → `&String` → `&str` (via `Deref` chain)
- `&mut T` → `&T` is allowed; `&T` → `&mut T` is NOT

```rust
fn hello(name: &str) { println!("Hello, {name}!"); }

let s = Box::new(String::from("Alice"));
hello(&s);    // &Box<String> coerces to &String coerces to &str automatically
```

---

## `Drop` trait

```rust
struct CustomDrop { data: String }

impl Drop for CustomDrop {
    fn drop(&mut self) {
        println!("Dropping: {}", self.data);
    }
}

let x = CustomDrop { data: String::from("first") };
let y = CustomDrop { data: String::from("second") };
// y is dropped first (LIFO), then x

// Force early drop:
drop(x);   // calls Drop::drop and frees memory
```

---

## `Weak<T>` — break reference cycles

```rust
use std::rc::{Rc, Weak};
use std::cell::RefCell;

#[derive(Debug)]
struct Node {
    value:    i32,
    children: RefCell<Vec<Rc<Node>>>,
    parent:   RefCell<Weak<Node>>,    // Weak: doesn't keep parent alive
}

let leaf = Rc::new(Node {
    value:    3,
    children: RefCell::new(vec![]),
    parent:   RefCell::new(Weak::new()),
});

let branch = Rc::new(Node {
    value:    5,
    children: RefCell::new(vec![Rc::clone(&leaf)]),
    parent:   RefCell::new(Weak::new()),
});

*leaf.parent.borrow_mut() = Rc::downgrade(&branch);  // Rc → Weak

// To use a Weak reference, upgrade it:
let parent = leaf.parent.borrow().upgrade();   // Option<Rc<Node>>
match parent {
    Some(p) => println!("parent = {}", p.value),
    None    => println!("no parent"),
}
```

**Rule**: `Rc` for parent→children (ownership direction). `Weak` for children→parent (no ownership).
