# Iced App Structure Patterns

> These patterns are suggestions, not rules. Mix and match to fit your needs.

## Pattern Overview

| Pattern | State? | Update? | Complexity | When to use |
|---------|--------|---------|-----------|-------------|
| **View-Helper** | No | No | Trivial | Simple reusable view fragments |
| **Viewable** | No | No | Low | Builder-pattern pseudo-widgets |
| **Composition** | Yes | Yes | Medium | Self-contained interactive modules |
| **Widget** | Yes | Yes | High | Full custom rendering/layout control |

---

## 1. View-Helper Pattern

Just a function that returns an `Element`. Simplest reusability.

```rust
pub fn list<'a>(
    items: &'a [String],
    on_delete: impl Fn(usize) -> Message + 'a,
) -> iced::Element<'a, Message> {
    iced::widget::column(
        items.iter()
            .enumerate()
            .map(|(index, item)| {
                iced::widget::row![
                    iced::widget::text(item),
                    iced::widget::button("Delete")
                        .style(iced::widget::button::danger)
                        .on_press(on_delete(index)),
                ].into()
            })
    ).into()
}

// Usage in view:
fn view(&self) -> Element<'_, Message> {
    list(&self.items, |index| Message::Delete(index))
}
```

**Pros:** Simple, zero boilerplate, reusable.
**Cons:** Complex views need many function parameters.

---

## 2. Viewable Pattern

A struct with builder pattern that implements `Into<Element>`.
Behaves like a widget but without Widget trait complexity.

```rust
pub struct ListItem<'a, Message> {
    item: iced::Element<'a, Message>,
    on_delete: Option<Message>,
    on_edit: Option<Message>,
}

impl<'a, Message> ListItem<'a, Message> {
    pub fn new(item: impl Into<iced::Element<'a, Message>>) -> Self {
        Self {
            item: item.into(),
            on_delete: None,
            on_edit: None,
        }
    }

    pub fn on_delete(mut self, message: Message) -> Self {
        self.on_delete = Some(message);
        self
    }

    pub fn on_edit(mut self, message: Message) -> Self {
        self.on_edit = Some(message);
        self
    }
}

impl<'a, Message: Clone + 'a> From<ListItem<'a, Message>> for iced::Element<'a, Message> {
    fn from(item: ListItem<'a, Message>) -> Self {
        let mut row = iced::widget::row![item.item].spacing(10);

        if let Some(on_delete) = item.on_delete {
            row = row.push(
                iced::widget::button("Delete").on_press(on_delete)
            );
        }
        if let Some(on_edit) = item.on_edit {
            row = row.push(
                iced::widget::button("Edit").on_press(on_edit)
            );
        }

        row.into()
    }
}
```

### Usage

```rust
fn view(&self) -> Element<'_, Message> {
    let items: Vec<Element<'_, Message>> = self.items.iter()
        .enumerate()
        .map(|(i, item)| {
            ListItem::new(text(item))
                .on_delete(Message::Delete(i))
                .on_edit(Message::Edit(i))
                .into()
        })
        .collect();

    Column::from_vec(items).spacing(4).into()
}
```

**Pros:** Ergonomic builder pattern, feels like a real widget.
**Cons:** No internal state — for that, use Composition.

---

## 3. Composition Pattern

> Previously called "Component Pattern." Used by Halloy, icebreaker.

A self-contained module with its own State + Message + Action + View.
Like a mini iced application embedded in the parent.

### Structure

```
src/
├── main.rs           # Parent App
├── new_joke.rs       # Composition module
└── list_item.rs      # Viewable (optional)
```

### The Composition Module

```rust
// new_joke.rs

// 1. State
pub struct NewJoke {
    joke: String,
}

impl NewJoke {
    pub fn new() -> Self {
        Self { joke: String::new() }
    }
}

// 2. Internal Message
#[derive(Debug, Clone)]
pub enum Message {
    ChangeContent(String),
    RandomJoke,
    Submit,
    Cancel,
}

// 3. Action (output to parent)
pub enum Action {
    Submit(String),
    Cancel,
    Run(iced::Task<Message>),
    None,
}

// 4. Update — returns Action, not Task
impl NewJoke {
    #[must_use]
    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Submit => Action::Submit(self.joke.clone()),
            Message::Cancel => Action::Cancel,
            Message::ChangeContent(content) => {
                self.joke = content;
                Action::None
            }
            Message::RandomJoke => {
                Action::Run(Self::random_joke_task())
            }
        }
    }

    // 5. View
    pub fn view(&self) -> iced::Element<'_, Message> {
        iced::widget::column![
            iced::widget::text_input("Enter joke...", &self.joke)
                .on_input(Message::ChangeContent),
            iced::widget::row![
                iced::widget::button("Submit").on_press(Message::Submit),
                iced::widget::button("Random").on_press(Message::RandomJoke),
                iced::widget::button("Cancel").on_press(Message::Cancel),
            ].spacing(8),
        ]
        .spacing(12)
        .into()
    }

    fn random_joke_task() -> iced::Task<Message> {
        iced::Task::future(async {
            // fetch from API...
            Message::ChangeContent("Why did the chicken...".to_string())
        })
    }
}
```

### Embedding in Parent

```rust
// main.rs
mod new_joke;

#[derive(Debug, Clone)]
enum Message {
    NewJoke(new_joke::Message),     // wrap child messages
    OpenNewJoke,
    Delete(usize),
}

#[derive(Default)]
enum View {
    #[default]
    ListJokes,
    NewJoke(new_joke::NewJoke),
}

struct App {
    view: View,
    items: Vec<String>,
}

impl App {
    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::NewJoke(msg) => {
                if let View::NewJoke(composition) = &mut self.view {
                    match composition.update(msg) {
                        new_joke::Action::None => {}
                        new_joke::Action::Run(task) => {
                            // Map child task messages to parent
                            return task.map(Message::NewJoke);
                        }
                        new_joke::Action::Cancel => {
                            self.view = View::ListJokes;
                        }
                        new_joke::Action::Submit(joke) => {
                            self.view = View::ListJokes;
                            self.items.push(joke);
                        }
                    }
                }
            }
            Message::OpenNewJoke => {
                self.view = View::NewJoke(new_joke::NewJoke::new());
            }
            Message::Delete(i) => { self.items.remove(i); }
        }
        iced::Task::none()
    }

    fn view(&self) -> iced::Element<'_, Message> {
        match &self.view {
            View::ListJokes => {
                // ... list view ...
                column![
                    button("New").on_press(Message::OpenNewJoke),
                    // ... items ...
                ].into()
            }
            View::NewJoke(composition) => {
                // Map child view messages to parent
                composition.view().map(Message::NewJoke)
            }
        }
    }
}
```

### Key Points

1. **`Action` enum** — replaces `Task` return from update. Common variants: `None`, `Run(Task)`, domain-specific outputs
2. **`.map(Message::Child)`** — converts child messages/tasks/views to parent types
3. **`#[must_use]` on update** — forces parent to handle the Action
4. **Halloy calls `Action` → `Event`** — same pattern, different name
5. **Lazy loading** — use enum state: `Loading | Loaded { data } | Error(String)`

### Composition with startup Task

```rust
impl LazyImage {
    pub fn new(url: String) -> (Self, iced::Task<Message>) {
        let task = Task::perform(fetch_image(url), Message::Loaded);
        (Self::Loading, task)
    }
}
```

**Pros:** Clean separation, scalable, testable.
**Cons:** Boilerplate (Message, Action, mapping). Use Viewable if no internal state needed.

---

## Choosing a Pattern

```
Need reusable view fragment?
├── No internal state needed?
│   ├── Simple (few params) → View-Helper
│   └── Complex (many options) → Viewable
└── Has its own state/logic?
    ├── Standard interactions → Composition
    └── Custom rendering/layout → Widget (see widgets.md)
```
