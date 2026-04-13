# Collections, Strings, and HashMap

> Source: Rust Book chapter 8

---

## Vec<T>

```rust
// Create
let v: Vec<i32> = Vec::new();
let v = vec![1, 2, 3];

// Push and pop
let mut v = Vec::new();
v.push(5);
v.push(6);
let last = v.pop();   // Option<i32>

// Access — two options:
let third = &v[2];       // panics if out of bounds
let third = v.get(2);    // returns Option<&i32>

// Iteration (immutable):
for i in &v { println!("{i}"); }

// Iteration (mutable):
for i in &mut v { *i += 50; }

// Common operations:
v.len()
v.is_empty()
v.contains(&5)
v.sort()
v.dedup()                // remove consecutive duplicates (sort first)
v.retain(|&x| x > 2)    // keep only elements matching predicate
v.extend([7, 8, 9])
v.truncate(3)            // keep first 3 elements
v.clear()

// Slicing:
let slice: &[i32] = &v[1..3];

// Collect from iterator:
let doubled: Vec<i32> = v.iter().map(|&x| x * 2).collect();
```

### Vec of enums for mixed types

```rust
#[derive(Debug)]
enum Cell {
    Int(i32),
    Float(f64),
    Text(String),
}

let row = vec![
    Cell::Int(3),
    Cell::Float(10.12),
    Cell::Text(String::from("hello")),
];
```

---

## String

Rust has two string types:
- `str` / `&str` — string slice, UTF-8, usually borrowed, fixed size
- `String` — heap-allocated, owned, growable, UTF-8

```rust
// Creation
let s = String::new();
let s = String::from("hello");
let s = "initial".to_string();

// Append
let mut s = String::from("hello");
s.push_str(" world");  // append &str
s.push('!');           // append char

// Concatenation
let s1 = String::from("Hello, ");
let s2 = String::from("world!");
let s3 = s1 + &s2;    // s1 is MOVED, s2 is borrowed
// use format! to avoid ownership issues:
let s3 = format!("{s1}{s2}");  // neither moved

// Length (bytes, NOT chars — UTF-8!)
s.len()            // byte count
s.chars().count()  // character count

// Slicing — MUST slice at char boundaries:
let s = String::from("hello");
let slice = &s[0..3];  // "hel" — OK, all ASCII
// let bad = &s[0..1]; // panics if multi-byte char

// Iteration:
for c in s.chars() { /* char */ }
for b in s.bytes() { /* u8 */  }

// Contains, replace, trim:
s.contains("hello")
s.replace("hello", "hi")
s.trim()
s.to_uppercase()
s.to_lowercase()
s.starts_with("he")
s.ends_with("lo")

// Splitting:
let parts: Vec<&str> = "a,b,c".split(',').collect();
```

### `&str` vs `String`

| | `&str` | `String` |
|--|--------|---------|
| Ownership | Borrowed | Owned |
| Allocation | Stack / static | Heap |
| Mutable | No | Yes |
| Use for | Function parameters | Storing/building text |

**Best practice**: accept `&str` in function parameters — both `String` and `&str` can be passed:
```rust
fn greet(name: &str) { println!("Hello, {name}!"); }

greet("Alice");                              // &str literal
greet(&String::from("Bob"));                // &String coerces to &str
```

---

## HashMap<K, V>

```rust
use std::collections::HashMap;

// Create
let mut scores: HashMap<String, i32> = HashMap::new();

// Insert
scores.insert(String::from("Blue"),  10);
scores.insert(String::from("Red"),   50);

// Build from iterators:
let teams  = vec!["Blue", "Red"];
let scores = vec![10, 50];
let map: HashMap<_, _> = teams.iter().zip(scores.iter()).collect();

// Access:
let score = scores.get("Blue");           // Option<&i32>
let score = scores["Blue"];               // panics if missing
let score = scores.get("Blue").copied();  // Option<i32>

// Entry API — insert or update:
scores.entry(String::from("Yellow")).or_insert(50);  // only insert if missing
let count = word_count.entry(word).or_insert(0);
*count += 1;  // increment existing or initialised-to-0

// Iteration (order is non-deterministic):
for (key, value) in &scores {
    println!("{key}: {value}");
}

// Common operations:
scores.contains_key("Blue")
scores.remove("Blue")         // Option<i32>
scores.len()
scores.is_empty()
scores.keys()                 // iterator
scores.values()               // iterator
scores.values_mut()           // mutable iterator

// Overwrite:
scores.insert(String::from("Blue"), 25);  // replaces existing
```

### Ownership and HashMap

When values are inserted, they are moved (for owned types) or copied (for `Copy` types):

```rust
let key   = String::from("field");
let value = String::from("blue");

map.insert(key, value);
// key and value are MOVED; use &key if you need them after
```

### Using `BTreeMap` for sorted keys

```rust
use std::collections::BTreeMap;
let mut map = BTreeMap::new();
// Keys are always iterated in sorted order.
```

---

## Other standard collections

| Collection | Description |
|-----------|-------------|
| `HashSet<T>` | Unique values, O(1) lookup |
| `BTreeSet<T>` | Unique values, sorted |
| `BTreeMap<K,V>` | Sorted key-value pairs |
| `VecDeque<T>` | Double-ended queue (push/pop front and back) |
| `LinkedList<T>` | Doubly-linked list (rarely needed) |
| `BinaryHeap<T>` | Priority queue (max-heap by default) |
