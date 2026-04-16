# Async / Await, Futures, and Streams

> Source: Rust Book chapter 17

---

## Core concepts

| Concept | Description |
|---------|-------------|
| **Future** | A value representing async computation; lazy — does nothing until polled |
| **async fn** | Returns a `Future` automatically |
| **await** | Yields control until the Future completes; can only be used inside `async` |
| **Runtime** | Executes futures (e.g., tokio, async-std); not in std |
| **Task** | A unit of work scheduled on the async runtime (like a lightweight thread) |

---

## Basic `async` / `await`

```rust
// Cargo.toml: tokio = { version = "1", features = ["full"] }

use tokio;

async fn say_hello() -> String {
    String::from("hello")
}

async fn greet(name: &str) {
    let greeting = say_hello().await;   // await suspends until Future completes
    println!("{greeting}, {name}!");
}

#[tokio::main]
async fn main() {
    greet("Alice").await;
}
```

**Futures are lazy**: creating a future does nothing. You must `.await` it (or spawn it as a task).

---

## Spawning tasks

```rust
use tokio::task;

#[tokio::main]
async fn main() {
    let handle = task::spawn(async {
        // Runs concurrently with main task
        println!("from spawned task");
        42   // task return value
    });

    let result = handle.await.unwrap();
    println!("task returned: {result}");
}
```

---

## Concurrent execution

```rust
use tokio::time::{sleep, Duration};

async fn count_to(n: u32, label: &str) {
    for i in 1..=n {
        sleep(Duration::from_millis(100)).await;
        println!("{label}: {i}");
    }
}

#[tokio::main]
async fn main() {
    // Sequential — second starts only after first finishes:
    count_to(3, "A").await;
    count_to(3, "B").await;

    // Concurrent — both run simultaneously:
    tokio::join!(
        count_to(3, "A"),
        count_to(3, "B"),
    );
    // or with different result types:
    let (r1, r2) = tokio::join!(fetch_data(), compute());
}
```

### `select!` — race futures

```rust
use tokio::select;

// Whichever resolves first wins; the other is dropped:
select! {
    result = fast_future()  => println!("fast: {result}"),
    result = slow_future()  => println!("slow: {result}"),
}
```

---

## Async channels

```rust
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::channel::<String>(32);  // buffer size 32
    let tx2 = tx.clone();

    tokio::spawn(async move { tx.send("from task 1".to_string()).await.unwrap(); });
    tokio::spawn(async move { tx2.send("from task 2".to_string()).await.unwrap(); });

    drop(tx);  // close all senders so receiver loop terminates
    while let Some(msg) = rx.recv().await {
        println!("{msg}");
    }
}
```

---

## Async error handling

```rust
use tokio::fs;

async fn read_file(path: &str) -> Result<String, std::io::Error> {
    let content = fs::read_to_string(path).await?;  // ? works in async functions
    Ok(content)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let content = read_file("hello.txt").await?;
    println!("{content}");
    Ok(())
}
```

---

## Streams — async iterators

```rust
use tokio_stream::StreamExt;  // tokio-stream crate

async fn process_stream() {
    let mut stream = tokio_stream::iter(vec![1, 2, 3, 4, 5]);

    while let Some(value) = stream.next().await {
        println!("{value}");
    }
}
```

---

## Threads vs Tasks vs Futures

| | Threads | Tasks | Futures |
|--|---------|-------|---------|
| Managed by | OS | Runtime | Runtime |
| Memory | ~8 MB stack | Small | Minimal |
| Concurrency | OS scheduling | Cooperative | Cooperative |
| Good for | CPU-bound | I/O-bound | Async primitives |

```
Rule of thumb:
- CPU-bound work (heavy computation): std::thread
- I/O-bound work (network, files): async/await
- Mix: async for coordination + spawn_blocking for CPU parts
```

---

## `spawn_blocking` — run sync code in async context

```rust
#[tokio::main]
async fn main() {
    // Don't block the async executor with CPU-heavy work!
    let result = tokio::task::spawn_blocking(|| {
        // Runs on a dedicated thread pool, not on async executor
        compute_heavy_work()
    }).await.unwrap();
}
```

---

## The `Future` trait

```rust
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

// Every async fn compiles to something like this:
struct MyFuture { state: u32 }

impl Future for MyFuture {
    type Output = u32;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<u32> {
        self.state += 1;
        if self.state >= 3 {
            Poll::Ready(self.state)
        } else {
            cx.waker().wake_by_ref();  // ask runtime to poll again
            Poll::Pending
        }
    }
}
```

`Pin<&mut T>` prevents the future from being moved after it's started (needed because futures may self-reference across yield points).

---

## Common async runtimes

| Crate | Features |
|-------|---------|
| `tokio` | Full-featured, multi-threaded, most popular |
| `async-std` | Mirrors std API, single/multi-threaded |
| `smol` | Lightweight, minimal |

```toml
# Cargo.toml for tokio:
tokio = { version = "1", features = ["full"] }
# or minimal:
tokio = { version = "1", features = ["rt", "rt-multi-thread", "macros"] }
```
