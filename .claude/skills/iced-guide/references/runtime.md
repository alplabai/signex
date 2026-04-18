# Iced Runtime — Tasks, Subscriptions, Blocking Code

## Tasks

> NOTE: `Task` was previously called `Command`.

A Task is "a set of asynchronous actions to be performed by some runtime."
Internally, a Task is a `Stream` that returns messages.

### Returning Tasks from update

```rust
fn update(&mut self, message: Message) -> iced::Task<Message> {
    match message {
        Message::DoSomething => {
            // return a task
            Task::perform(async_operation(), Message::Result)
        }
        Message::Result(value) => {
            self.value = value;
            Task::none()  // no async follow-up
        }
    }
}
```

### Batch multiple tasks

```rust
return Task::batch(vec![task1, task2, task3]);
```

---

## Custom Task with `Task::perform`

Takes a future and a closure that converts the result into a Message.

```rust
#[derive(Debug, Clone)]
enum Message {
    Refetch,
    CurrentIp(String),
}

impl App {
    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::Refetch => Task::perform(fetch_ip(), Message::CurrentIp),
            Message::CurrentIp(text) => {
                self.ip = text;
                Task::none()
            }
        }
    }
}

async fn fetch_ip() -> String {
    reqwest::get("https://api.seeip.org")
        .await.unwrap()
        .text()
        .await.unwrap()
}
```

**Requires:** `iced = { features = ["tokio"] }` and `reqwest` dependency.

---

## Task from Future (single message)

```rust
iced::Task::future(async {
    // do async work
    let result = some_async_fn().await;
    Message::Done(result)
})
```

---

## Task from Stream (multiple messages)

Use `Task::run` with a Stream for tasks that produce multiple messages.

```rust
use iced::stream;

fn download_task() -> Task<Message> {
    Task::run(
        stream::channel(100, |mut sender| async move {
            sender.send(Message::Progress(0.0)).await.ok();
            // ... do work ...
            sender.send(Message::Progress(0.5)).await.ok();
            // ... more work ...
            sender.send(Message::Progress(1.0)).await.ok();
        })
    )
}
```

`iced::stream::channel(buffer_size, |sender| future)` converts a Future to a Stream.

---

## Subscriptions

A Subscription runs in the background as long as you return it.
Often used for external event listeners.

### Key differences from Task
- **Task** runs until complete
- **Subscription** runs as long as returned from `subscription()` closure
- Subscription can never end by itself — runtime controls lifecycle
- Each Subscription has an **ID** (hash of data + TypeId)

### Subscription lifecycle
1. Runtime calls `subscription()` after each `update()`
2. If new subscription returned → runtime starts it
3. If already-running subscription returned → nothing happens (continues)
4. If running subscription NOT returned → runtime terminates it

### Setting up subscriptions

```rust
fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .subscription(App::subscription)
        .run()
}

impl App {
    fn subscription(&self) -> iced::Subscription<Message> {
        // return subscription based on state
        if self.listening {
            iced::event::listen().map(Message::Event)
        } else {
            iced::Subscription::none()
        }
    }
}
```

### Listen to all events

```rust
fn subscription(&self) -> Subscription<Message> {
    iced::event::listen().map(Message::Event)
}

fn update(&mut self, message: Message) -> Task<Message> {
    match message {
        Message::Event(event) => {
            if let iced::event::Event::Keyboard(
                iced::keyboard::Event::KeyReleased { key, .. }
            ) = event {
                println!("Key {key:?} was pressed");
            }
            Task::none()
        }
    }
}
```

**Note:** There are specific subscriptions for keyboard, window, etc. events.
`iced::event::listen()` reacts to ALL event kinds.

### Custom subscription with Stream

```rust
fn my_subscription() -> Subscription<Message> {
    Subscription::run(|| {
        stream::channel(100, |mut sender| async move {
            loop {
                // listen for external events
                let event = wait_for_event().await;
                sender.send(Message::ExternalEvent(event)).await.ok();
            }
        })
    })
}
```

---

## Blocking Code

> NOTE: Cannot run blocking code on WASM.

### With Tokio (recommended)

```toml
[dependencies]
iced = { version = "0.14", features = ["tokio"] }
```

```rust
fn update(&mut self, message: Message) -> iced::Task<Message> {
    match message {
        Message::StartCalculation => {
            self.calculating = true;
            iced::Task::future(async {
                let result = tokio::task::spawn_blocking(|| {
                    // heavy computation here
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    42
                })
                .await
                .unwrap();

                Message::CalculationDone(result)
            })
        }
        Message::CalculationDone(result) => {
            self.result = Some(result);
            self.calculating = false;
            Task::none()
        }
    }
}
```

### With Smol

```toml
smol = "2"
```

```rust
async fn heavy_work() -> i32 {
    smol::unblock(|| {
        // blocking computation
        42
    }).await
}
```

### With Oneshot Channel (no extra deps)

```rust
async fn meaning_of_life() -> Option<MeaningOfLife> {
    use iced::futures::channel;
    let (tx, rx) = channel::oneshot::channel();
    std::thread::spawn(|| {
        let result = calculate_meaning();
        tx.send(result).expect("send result");
    });
    rx.await.ok()
}
```

Uses `iced::futures` re-export — no extra dependencies needed.

---

## Task Chaining

```rust
fn open_image() -> Task<Message> {
    Task::future(
        rfd::AsyncFileDialog::new()
            .add_filter("Images", &["png", "jpg"])
            .pick_file(),
    )
    .then(|handle| match handle {
        Some(file) => Task::perform(load_image(file), Message::ImageLoaded),
        None => Task::done(Message::FileCancelled),
    })
}
```

### Task methods
- `Task::none()` — no-op
- `Task::done(msg)` — immediately produce a message
- `Task::perform(future, map)` — run future, map result to message
- `Task::future(future)` — run future that directly returns a message
- `Task::run(stream)` — run stream that produces multiple messages
- `Task::batch(vec)` — run multiple tasks concurrently
- `task.map(fn)` — transform messages
- `task.then(fn)` — chain tasks sequentially

---

## Mapping Messages

Convert child messages to parent messages:

```rust
// Element mapping (in view)
child.view().map(Message::Child)

// Task mapping (in update)
child_task.map(Message::Child)

// Subscription mapping
child_subscription.map(Message::Child)
```
