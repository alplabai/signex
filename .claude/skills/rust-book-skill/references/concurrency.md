# Fearless Concurrency

> Source: Rust Book chapter 16

---

## Threads

```rust
use std::thread;
use std::time::Duration;

// Spawn a thread:
let handle = thread::spawn(|| {
    for i in 1..=10 {
        println!("hi {i} from spawned thread");
        thread::sleep(Duration::from_millis(1));
    }
});

// Main thread work...
for i in 1..=5 {
    println!("hi {i} from main thread");
    thread::sleep(Duration::from_millis(1));
}

// Wait for spawned thread to finish:
handle.join().unwrap();
```

### Moving ownership into threads

```rust
let v = vec![1, 2, 3];

// `move` transfers ownership of v into the closure:
let handle = thread::spawn(move || {
    println!("{v:?}");
});

// v is no longer accessible here
handle.join().unwrap();
```

### Thread return values

```rust
let handle = thread::spawn(|| {
    42   // thread returns this value
});

let result: i32 = handle.join().unwrap();
println!("Thread returned: {result}");
```

---

## Message Passing — Channels

Rust channels are **multi-producer, single-consumer (mpsc)**.

```rust
use std::sync::mpsc;

let (tx, rx) = mpsc::channel();

// Sender:
thread::spawn(move || {
    tx.send(String::from("hello")).unwrap();
    tx.send(String::from("world")).unwrap();
    // tx is dropped here, closing the channel
});

// Receiver (blocking):
let msg = rx.recv().unwrap();    // blocks until a message arrives
println!("{msg}");

// Receive in a loop until closed:
for received in rx {
    println!("{received}");
}
```

### Multiple producers

```rust
let (tx, rx) = mpsc::channel::<String>();

let tx2 = tx.clone();   // clone the sender

thread::spawn(move || tx.send("from thread 1".to_string()).unwrap());
thread::spawn(move || tx2.send("from thread 2".to_string()).unwrap());

for msg in rx { println!("{msg}"); }   // receive from both
```

### Non-blocking `try_recv`

```rust
match rx.try_recv() {
    Ok(msg)                        => println!("{msg}"),
    Err(mpsc::TryRecvError::Empty) => println!("no message yet"),
    Err(mpsc::TryRecvError::Disconnected) => break,
}
```

---

## Shared State — `Mutex<T>`

```rust
use std::sync::Mutex;

let m = Mutex::new(5);

{
    let mut num = m.lock().unwrap();   // blocks until lock acquired
    *num = 6;
}   // MutexGuard drops here, lock is released automatically

println!("{:?}", m);  // Mutex { data: 6 }
```

**Mutex provides interior mutability**: even an immutable `Mutex` reference can lock and mutate.

### `Arc<Mutex<T>>` for shared mutable state across threads

```rust
use std::sync::{Arc, Mutex};
use std::thread;

let counter = Arc::new(Mutex::new(0_i32));
let mut handles = vec![];

for _ in 0..10 {
    let c = Arc::clone(&counter);
    handles.push(thread::spawn(move || {
        *c.lock().unwrap() += 1;
    }));
}

for h in handles { h.join().unwrap(); }
println!("Result: {}", *counter.lock().unwrap());  // 10
```

### `RwLock<T>` — many readers or one writer

```rust
use std::sync::RwLock;

let lock = RwLock::new(vec![1, 2, 3]);

// Multiple concurrent readers:
{
    let r1 = lock.read().unwrap();
    let r2 = lock.read().unwrap();
    println!("{r1:?} {r2:?}");
}

// Single writer:
{
    let mut w = lock.write().unwrap();
    w.push(4);
}
```

---

## `Send` and `Sync` traits

```rust
// Send: type can be MOVED to another thread
// Sync: &T can be SHARED across threads (T is Sync if &T is Send)

// Automatically implemented when all fields are Send/Sync.

// NOT Send: Rc<T> (non-atomic ref count)
// NOT Sync: RefCell<T> (non-thread-safe borrow checking)
// IS Send+Sync: Arc<T>, Mutex<T>, atomic types, most primitives

// Manual implementation (unsafe — only for custom primitives):
unsafe impl Send for MyType {}
unsafe impl Sync for MyType {}
```

| Type | `Send` | `Sync` |
|------|--------|--------|
| `i32`, `f64`, etc. | Yes | Yes |
| `String`, `Vec<T>` | Yes | Yes |
| `Rc<T>` | **No** | **No** |
| `Arc<T>` | Yes | Yes |
| `RefCell<T>` | Yes | **No** |
| `Mutex<T>` | Yes | Yes |
| `*mut T` (raw ptr) | **No** | **No** |

---

## Atomics — lock-free counters

```rust
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

let count = Arc::new(AtomicUsize::new(0));
let c = Arc::clone(&count);

thread::spawn(move || {
    c.fetch_add(1, Ordering::SeqCst);
});

thread::sleep(Duration::from_millis(50));
println!("{}", count.load(Ordering::SeqCst));
```

For simple counters, prefer `AtomicUsize` / `AtomicBool` over `Mutex<i32>` — cheaper.

---

## Thread pools

The standard library has no thread pool. Use the `rayon` crate for data parallelism:

```rust
// Cargo.toml: rayon = "1"
use rayon::prelude::*;

let data = vec![1_u64, 2, 3, 4, 5, 6, 7, 8];
let sum: u64 = data.par_iter().sum();   // parallel iterator
```

---

## Common pitfalls

**Deadlock**: `Mutex` is not reentrant — holding a lock and trying to acquire it again deadlocks.

```rust
// DEADLOCK:
let m = Mutex::new(0);
let g1 = m.lock().unwrap();
let g2 = m.lock().unwrap();   // deadlock: g1 is still held
```

**Poisoned mutex**: if a thread panics while holding a lock, the mutex is "poisoned".
```rust
let result = m.lock();  // Result::Err if poisoned
// Use .unwrap() only if you're OK with panicking on poisoned mutex.
// Or recover: let guard = m.lock().unwrap_or_else(|e| e.into_inner());
```
