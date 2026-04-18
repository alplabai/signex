# Iced Architecture — ELM / MVU Pattern

## The 4 Parts

### 1. State (struct)
All data your program stores throughout its lifespan.

```rust
struct App {
    count: i32,
    theme: Option<Theme>,
    items: Vec<String>,
}
```

### 2. Message (enum)
Events or interactions your program cares about. Must derive `Debug, Clone`.

```rust
#[derive(Debug, Clone)]
enum Message {
    IncrementCount,
    DecrementCount,
    ThemeChanged(Theme),
    ItemAdded(String),
}
```

### 3. Update Logic
Called every time a message is emitted. **Only place that can change state.**
Returns `Task<Message>` for async follow-up work.

```rust
fn update(&mut self, message: Message) -> iced::Task<Message> {
    match message {
        Message::IncrementCount => self.count = self.count.saturating_add(1),
        Message::DecrementCount => self.count = self.count.saturating_sub(1),
        Message::ThemeChanged(t) => self.theme = Some(t),
        Message::ItemAdded(s) => self.items.push(s),
    }
    iced::Task::none()
}
```

### 4. View Logic
Generates the UI based on current state. Called after every `update()`.
Returns `Element<'_, Message>`.

```rust
fn view(&self) -> iced::Element<'_, Message> {
    let row = widget::row![
        widget::button("-").on_press(Message::DecrementCount),
        widget::text!("Count: {}", self.count),
        widget::button("+").on_press(Message::IncrementCount),
    ]
    .spacing(10);

    widget::container(row).center(iced::Length::Fill).into()
}
```

---

## Application Setup

### Simple (no title/theme control)

```rust
fn main() -> iced::Result {
    iced::run(App::update, App::view)
}
```

State type is inferred. Uses `Default::default()` for initial state.

### Full control

```rust
fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("My App")
        .theme(App::theme)
        .subscription(App::subscription)
        .run()
}
```

- `App::new` → `(Self, Task<Message>)` — initial state + startup task
- `App::theme` → `fn(&self) -> Theme` — dynamic theme based on state
- `App::subscription` → `fn(&self) -> Subscription<Message>` — background listeners

### Initial state with startup task

```rust
impl App {
    fn new() -> (Self, iced::Task<Message>) {
        let app = Self { loading: true, ..Default::default() };
        let task = Task::perform(load_data(), Message::DataLoaded);
        (app, task)
    }
}
```

---

## Complete Counter Example

```rust
use iced::widget;

struct Counter {
    count: i32,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    IncrementCount,
    DecrementCount,
}

impl Counter {
    fn new() -> Self {
        Self { count: 0 }
    }

    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::IncrementCount => self.count += 1,
            Message::DecrementCount => self.count -= 1,
        }
        iced::Task::none()
    }

    fn view(&self) -> iced::Element<'_, Message> {
        let row = widget::row![
            widget::button("-").on_press(Message::DecrementCount),
            widget::text!("Count: {}", self.count),
            widget::button("+").on_press(Message::IncrementCount)
        ]
        .spacing(10);

        widget::container(row).center(iced::Length::Fill).into()
    }
}

fn main() -> iced::Result {
    iced::application(Counter::new, Counter::update, Counter::view)
        .title("Counter Example")
        .run()
}
```

---

## Key Rules

1. `iced::Result` is `Result<(), iced::Error>`
2. `view()` returns `Element<'_, Message>` — use `.into()` for conversion
3. State changes ONLY in `update()` — view is pure/read-only
4. `Task::none()` when update has no async work
5. Use `saturating_add`/`saturating_sub` for safer arithmetic
6. `text!("format {}", val)` macro for formatted text (0.14)
