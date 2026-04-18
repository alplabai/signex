# Closures and Iterators

> Source: Rust Book chapter 13

---

## Closures

Closures are anonymous functions that can capture their environment.

### Syntax

```rust
// Full annotation (optional):
let add = |x: i32, y: i32| -> i32 { x + y };

// Type-inferred (types inferred from first use):
let add = |x, y| x + y;

// Multi-line:
let process = |item: &str| {
    let trimmed = item.trim();
    trimmed.to_uppercase()
};

// No parameters:
let greet = || println!("Hello!");

// Comparison with functions:
fn  add_one_fn(x: u32) -> u32 { x + 1 }
let add_one_cl = |x: u32| -> u32 { x + 1 };
let add_one_cl = |x|              { x + 1 };
let add_one_cl = |x|                x + 1  ;
```

### Capturing environment

Closures capture variables from the enclosing scope in three ways:

```rust
let x = 5;

// Immutable borrow (FnOnce, FnMut, Fn):
let print_x = || println!("{x}");
print_x();   // x is borrowed immutably

// Mutable borrow (FnMut):
let mut count = 0;
let mut increment = || { count += 1; count };
increment();
increment();

// Move (take ownership) — use `move` keyword:
let s = String::from("hello");
let contains_hello = move || s.contains("hello");
// s is MOVED into the closure; can now be sent to a thread
```

### The `Fn` traits

| Trait | Description | Call count |
|-------|-------------|-----------|
| `FnOnce` | Can only call once; consumes captured values | Once |
| `FnMut` | Can call multiple times; mutates captured values | Many |
| `Fn` | Can call multiple times; only borrows immutably | Many |

Every closure implements at least `FnOnce`. Closures that don't move out of captured values implement `FnMut`. Closures that don't mutate captured values also implement `Fn`.

```rust
// Accepting any callable:
fn apply<F: Fn(i32) -> i32>(f: F, x: i32) -> i32 { f(x) }
fn apply_once<F: FnOnce() -> String>(f: F) -> String { f() }
fn apply_mut<F: FnMut()>(mut f: F) { f(); f(); }
```

### `move` closures for threads

```rust
use std::thread;

let v = vec![1, 2, 3];
let handle = thread::spawn(move || {  // v is moved into the closure
    println!("{v:?}");
});
handle.join().unwrap();
// v is no longer accessible here
```

---

## Iterators

All iterators implement the `Iterator` trait:

```rust
pub trait Iterator {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
    // + many default-implemented adapter methods
}
```

### Creating iterators

```rust
let v = vec![1, 2, 3];

v.iter()       // &T  — immutable references
v.iter_mut()   // &mut T — mutable references
v.into_iter()  // T   — consumes the collection, returns owned values

// Ranges:
(0..5)         // 0, 1, 2, 3, 4
(0..=5)        // 0, 1, 2, 3, 4, 5
```

Iterators are **lazy** — no work is done until consumed.

### Consuming adapters (terminal operations)

```rust
let v = vec![1, 2, 3, 4, 5];

// sum and product:
let total: i32 = v.iter().sum();           // 15
let product: i32 = v.iter().product();    // 120

// collect into a collection:
let doubled: Vec<i32> = v.iter().map(|&x| x * 2).collect();

// find and position:
let first_even = v.iter().find(|&&x| x % 2 == 0); // Option<&i32>
let pos        = v.iter().position(|&x| x == 3);   // Option<usize>

// any and all (short-circuit):
let has_five  = v.iter().any(|&x| x == 5);   // true
let all_pos   = v.iter().all(|&x| x > 0);    // true

// count:
let n = v.iter().count();   // 5

// min, max:
let min = v.iter().min();    // Option<&i32>
let max = v.iter().max();    // Option<&i32>

// fold (reduce):
let sum = v.iter().fold(0, |acc, &x| acc + x);

// for_each (consume without returning):
v.iter().for_each(|x| println!("{x}"));

// last:
let last = v.iter().last();   // Some(&5)

// nth (0-indexed):
let third = v.iter().nth(2);  // Some(&3)
```

### Iterator adapters (lazy transformers)

```rust
let v = vec![1, 2, 3, 4, 5];

// map — transform each element:
v.iter().map(|&x| x * 2)

// filter — keep matching elements:
v.iter().filter(|&&x| x % 2 == 0)

// filter_map — combine filter + map (remove None):
v.iter().filter_map(|&x| if x > 2 { Some(x * 10) } else { None })

// flat_map — map then flatten:
vec!["hello world", "foo bar"]
    .iter()
    .flat_map(|s| s.split_whitespace())

// flatten — one level of nesting:
vec![vec![1,2], vec![3,4]].into_iter().flatten()

// chain — concatenate iterators:
v.iter().chain([6, 7, 8].iter())

// zip — pair elements:
let names = vec!["Alice", "Bob"];
let ages  = vec![30, 25];
names.iter().zip(ages.iter())   // [("Alice",&30), ("Bob",&25)]

// enumerate — add index:
v.iter().enumerate()   // [(0,&1), (1,&2), ...]

// take / skip:
v.iter().take(3)       // first 3 elements
v.iter().skip(2)       // all but first 2

// take_while / skip_while:
v.iter().take_while(|&&x| x < 4)
v.iter().skip_while(|&&x| x < 3)

// peekable — look ahead without consuming:
let mut p = v.iter().peekable();
if p.peek() == Some(&&1) { println!("starts with 1"); }

// step_by:
(0..10).step_by(2)     // 0, 2, 4, 6, 8

// rev — reverse (only for DoubleEndedIterator):
v.iter().rev()

// cloned — &T → T (for Copy types):
v.iter().cloned()

// copied — same as cloned but for Copy:
v.iter().copied()

// inspect — debug without consuming:
v.iter()
 .filter(|&&x| x > 2)
 .inspect(|x| println!("after filter: {x}"))
 .collect::<Vec<_>>()
```

### Chaining complex pipelines

```rust
let result: Vec<String> = (1..=10)
    .filter(|n| n % 2 == 0)
    .map(|n| n * n)
    .take(3)
    .map(|n| format!("#{n}"))
    .collect();
// ["#4", "#16", "#36"]
```

### Implementing Iterator for custom types

```rust
struct Counter { count: u32, max: u32 }

impl Counter {
    fn new(max: u32) -> Self { Counter { count: 0, max } }
}

impl Iterator for Counter {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        if self.count < self.max {
            self.count += 1;
            Some(self.count)
        } else {
            None
        }
    }
}

// All adapter methods now work automatically:
let sum: u32 = Counter::new(5).sum();    // 15
let pairs: Vec<_> = Counter::new(3)
    .zip(Counter::new(3).skip(1))
    .collect();
```

### `collect()` type hints

```rust
// Type annotation on variable:
let v: Vec<i32> = iter.collect();

// Turbofish:
let v = iter.collect::<Vec<i32>>();
let v = iter.collect::<Vec<_>>();   // infer element type

// Collect into String:
let s: String = chars.collect();

// Collect into HashMap:
let map: HashMap<_, _> = keys.zip(values).collect();

// Collect into Result (all-or-nothing):
let results: Result<Vec<i32>, _> = strings.iter().map(|s| s.parse()).collect();
```

### Performance: iterators vs loops

Iterator chains compile to the same or better machine code as hand-written loops.
The Rust compiler unrolls and optimises iterator pipelines — they are **zero-cost abstractions**.
