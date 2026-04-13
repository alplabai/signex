# Task, Subscription, and Async — iced 0.14

---

## Task — one-shot async operations

`Task<Message>` is returned from `update` to tell the runtime to execute an
async operation. Replaces the old `Command<Message>` (renamed in 0.13+).

### Returning Task from update

```rust
// update can optionally return Task<Message>:
fn update(&mut self, message: Message) -> Task<Message> {
    match message {
        Message::LoadFile(path) => Task::perform(
            read_kicad_file(path),   // impl Future<Output = Result<Schematic, Error>>
            Message::FileLoaded,     // fn(Result<Schematic, Error>) -> Message
        ),
        Message::FileLoaded(Ok(sch)) => {
            self.schematic = Some(sch);
            self.canvas_cache.clear();
            Task::none()
        }
        Message::FileLoaded(Err(e)) => {
            self.error = Some(e.to_string());
            Task::none()
        }
    }
}
```

### Task API

```rust
use iced::Task;

Task::none()                            // do nothing
Task::done(Message::Ready)              // immediately produce one message
Task::perform(future, fn)               // Future → Message
Task::run(stream, fn)                   // Stream → multiple Messages
Task::batch(vec![task_a, task_b])       // run several tasks concurrently
task_a.chain(task_b)                    // run in sequence
task_a.map(|msg| other_msg)             // transform produced message
task_a.discard()                        // run for side-effects only
```

### Widget interaction tasks

```rust
use iced::widget::{text_input, scrollable};

// Focus a text input:
text_input::focus(text_input::Id::new("search"))

// Scroll to bottom:
scrollable::scroll_to(
    scrollable::Id::new("output-log"),
    scrollable::AbsoluteOffset { x: 0.0, y: f32::MAX },
)

// Exit the application:
iced::exit()
```

---

## Subscription — continuous event streams

`Subscription<Message>` declaratively describes an ongoing data source.
The runtime starts it when first returned from `subscription`, and stops it
when no longer returned. Identity determines whether the same stream is reused.

### subscription function

```rust
// Register via iced::application builder:
iced::application(...).subscription(MyApp::subscription)

fn subscription(state: &AppState) -> Subscription<Message> {
    if state.auto_reload {
        iced::time::every(Duration::from_secs(2))
            .map(|_| Message::Reload)
    } else {
        Subscription::none()
    }
}
```

### Timer

```rust
use iced::time::{self, Duration, Instant};

time::every(Duration::from_millis(16))
    .map(|_instant| Message::Tick)
```

### Keyboard events

```rust
use iced::keyboard;

keyboard::on_key_press(|key, modifiers| {
    use keyboard::{Key, Named};
    match (key, modifiers) {
        (Key::Named(Named::Delete), _)      => Some(Message::DeleteSelected),
        (Key::Named(Named::Escape), _)      => Some(Message::CancelTool),
        (Key::Character(c), m) if c == "s" && m.control() => Some(Message::Save),
        (Key::Character(c), m) if c == "z" && m.control() => Some(Message::Undo),
        _ => None,
    }
})
```

### Combining subscriptions

```rust
Subscription::batch([
    time::every(Duration::from_secs(1)).map(Message::Tick),
    keyboard::on_key_press(|k, m| handle_key(k, m)),
    file_watcher(state.open_path.clone()).map(Message::FileChanged),
])
```

---

## Channels — worker thread → app

### Task::run with Stream

```rust
use iced::futures::{SinkExt, StreamExt};
use iced::stream;

fn drc_stream(board: Board) -> impl Stream<Item = DrcResult> {
    stream::channel(32, async move |mut output| {
        for result in run_drc(&board) {
            output.send(result).await.ok();
        }
    })
}

// In update:
Message::RunDrc => Task::run(drc_stream(self.board.clone()), Message::DrcResult),
```

### Subscription::run with bidirectional channel

```rust
use iced::futures::{channel::mpsc, SinkExt, StreamExt};
use iced::stream;

#[derive(Debug, Clone)]
enum WorkerEvent {
    Ready(mpsc::Sender<WorkerInput>),
    Progress(f32),
    Done(Result<Output, String>),
}

fn worker_subscription() -> Subscription<WorkerEvent> {
    Subscription::run(|| stream::channel(32, async |mut output| {
        let (tx, mut rx) = mpsc::channel(32);
        output.send(WorkerEvent::Ready(tx)).await.ok();
        while let Some(input) = rx.next().await {
            let result = process(input).await;
            output.send(WorkerEvent::Done(result)).await.ok();
        }
    }))
}

// In update:
Message::WorkerReady(tx) => { self.worker_tx = Some(tx); Task::none() }
Message::StartWork(input) => {
    if let Some(tx) = &mut self.worker_tx {
        tx.try_send(input).ok();
    }
    Task::none()
}
```

---

## tokio feature flag

```toml
iced = { version = "0.14", features = ["tokio"] }
```

Required when Task wraps futures that use `tokio::fs`, `reqwest`, or any other
tokio-dependent crate. Without it you get:

```
there is no reactor running, must be called from the context of a Tokio 1.x runtime
```

---

## Subscription identity

The runtime tracks subscriptions by identity. When a `Subscription::run` closure
is new, a fresh stream is started; when the same closure is returned again the
existing stream continues. When a subscription stops being returned, its stream
is dropped.

```rust
// This subscription is uniquely identified by `path`:
Subscription::run_with_id(
    path.clone(),   // identity key — change it to restart the stream
    stream::channel(10, async move |mut output| {
        // … file watcher logic …
    }),
)
```

---

## HTTP request pattern

```rust
#[derive(Debug, Clone)]
enum Message {
    FetchIssues,
    IssuesFetched(Result<Vec<Issue>, String>),
}

fn update(&mut self, msg: Message) -> Task<Message> {
    match msg {
        Message::FetchIssues => Task::perform(
            fetch_github_issues(self.token.clone()),
            Message::IssuesFetched,
        ),
        Message::IssuesFetched(Ok(issues)) => {
            self.issues = issues;
            Task::none()
        }
        Message::IssuesFetched(Err(e)) => {
            self.error = Some(e);
            Task::none()
        }
    }
}

async fn fetch_github_issues(token: String) -> Result<Vec<Issue>, String> {
    reqwest::Client::new()
        .get("https://api.github.com/repos/owner/repo/issues")
        .bearer_auth(&token)
        .header("User-Agent", "my-eda-tool")
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())
}
```
