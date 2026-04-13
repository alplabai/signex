---
name: iced-rust
description: >
  iced kütüphanesiyle Rust GUI uygulaması geliştirmek için kapsamlı rehber.
  The Elm Architecture (state/message/update/view), iced::run ve
  iced::application API, widget'lar (button, text, column, row, container,
  scrollable, slider, text_input, canvas, vb.), layout (Length, Fill, Shrink,
  spacing, padding), styling (Theme, extended_palette, Status), Task (async),
  Subscription (stream), custom widget (Widget trait), wgpu entegrasyonu,
  EDA/CAD uygulamalar için nanometre koordinat sistemi, undo/redo (command
  pattern), multi-tab belge yönetimi, crate workspace mimarisi, Cargo.toml
  feature flag'leri gibi konuları kapsar. "iced", "iced-rs", "iced gui",
  "rust gui", "elm architecture rust", "iced widget", "iced layout",
  "iced theme", "iced task", "iced subscription" gibi ifadelerde tetiklenmeli.
---

# iced — Rust GUI Library Reference

> Source: book.iced.rs, docs.rs/iced 0.14 (December 2025), iced-rs/iced GitHub

---

## Reference map

| Topic | File |
|-------|------|
| Core architecture, run/application, full counter | This file |
| All widgets — button, text, column, row, container... | `references/widgets.md` |
| Layout — Length, Fill, Shrink, spacing, padding | `references/layout.md` |
| Styling — Theme, Palette, custom styles | `references/styling.md` |
| Task, Subscription, async, channels | `references/async.md` |
| Custom Widget — Widget trait implementation | `references/custom-widget.md` |
| Canvas advanced — cache, pan/zoom, hit-test, bezier | `references/canvas-advanced.md` |
| wgpu integration — GPU shaders inside iced | `references/wgpu-integration.md` |
| EDA patterns — nanometer coords, undo/redo, multi-tab | `references/eda-patterns.md` |
| iced_aw — tabs, menu, number_input, card, context_menu | `references/iced-aw.md` |

---

## The Elm Architecture

iced is built on **The Elm Architecture**. Four parts:

| Part | Rust | Description |
|------|------|-------------|
| **State** | `struct` | All application data |
| **Message** | `enum` | Possible interactions |
| **Update** | `fn update(&mut self, msg: Message)` | Apply message to state |
| **View** | `fn view(&self) -> Element<Message>` | Produce widget tree from state |

---

## Minimal example — Counter (iced 0.14)

```rust
use iced::widget::{button, column, text, Column};

pub fn main() -> iced::Result {
    iced::run(Counter::update, Counter::view)
}

#[derive(Default)]
struct Counter {
    value: i64,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    Increment,
    Decrement,
}

impl Counter {
    fn update(&mut self, message: Message) {
        match message {
            Message::Increment => self.value += 1,
            Message::Decrement => self.value -= 1,
        }
    }

    fn view(&self) -> Column<'_, Message> {
        column![
            button("+").on_press(Message::Increment),
            text(self.value),
            button("-").on_press(Message::Decrement),
        ]
    }
}
```

---

## iced::run vs iced::application

### `iced::run` — minimal

```rust
// State initialised from Default; update and view only
pub fn main() -> iced::Result {
    iced::run(MyApp::update, MyApp::view)
}
```

### `iced::application` — full builder

```rust
use iced::Theme;

pub fn main() -> iced::Result {
    iced::application("App Title", MyApp::update, MyApp::view)
        .theme(|_state| Theme::CatppuccinMocha)
        .window_size((1400.0, 900.0))
        .subscription(MyApp::subscription)
        .centered()
        .run()
}
```

Builder methods:

| Method | Description |
|--------|-------------|
| `.theme(fn)` | `fn(&State) -> Theme` — dynamic theme |
| `.title(fn)` | `fn(&State) -> String` — dynamic window title |
| `.subscription(fn)` | `fn(&State) -> Subscription<Message>` |
| `.window_size((w, h))` | Initial window dimensions |
| `.centered()` | Centre on screen at startup |
| `.antialiasing(bool)` | Anti-aliasing toggle |
| `.run()` | Start the runtime |

### Boot with init task

```rust
pub fn main() -> iced::Result {
    iced::application(MyApp::new, MyApp::update, MyApp::view).run()
}

impl MyApp {
    fn new() -> (Self, Task<Message>) {
        let app = MyApp::default();
        let task = Task::perform(load_config(), Message::ConfigLoaded);
        (app, task)
    }
}
```

---

## Element, Widget, `.into()`

`Element<'_, Message>` is a generic wrapper around any widget.

```rust
fn view(state: &State) -> Element<'_, Message> {
    button("Click").on_press(Message::Clicked).into()  // .into() required
}
```

- `column![]` and `row![]` macros already produce `Element` — no `.into()` needed.
- Single-widget view functions need `.into()`.
- `.map(MessageVariant)` converts child message type to parent message type.

---

## Cargo.toml

```toml
[dependencies]
iced = { version = "0.14", features = ["tokio", "canvas", "wgpu"] }
```

Key feature flags:

| Flag | Adds |
|------|------|
| `tokio` | Tokio runtime (required for tokio-dependent async Tasks) |
| `canvas` | Canvas widget — 2D CPU drawing via Frame/Path API |
| `wgpu` | wgpu backend + custom shader / geometry support |
| `image` | Image widget (PNG/JPEG display) |
| `svg` | SVG widget |
| `tiny-skia` | Software renderer fallback |
| `highlighter` | Syntax highlighting for TextEditor |
| `qr_code` | QR code widget |
| `debug` | F12 debug overlay |

---

## Runtime loop

```
State::default()  (or new())
    ↓
loop {
    view(&state)       →  widget tree
    render(widgets)
    interact(widgets)  →  messages
    for msg in messages { update(&mut state, msg) }
    subscription(&state)  →  re-evaluated
}
```

- `view` and `subscription` run after every batch of messages.
- iced 0.14: reactive rendering — only changed regions redrawn.

---

## Critical rules

1. **`#[derive(Debug, Clone, Copy)]` on every Message** — `on_press` requires `Clone`.
2. **`.into()` for `Element` returns** — single-widget view functions need it.
3. **`#[derive(Default)]` on State** — `iced::run` calls `Default::default()`.
4. **`features = ["tokio"]`** — required when Task wraps tokio-dependent futures.
5. **Macro syntax** — `column![]` and `row![]` use `,` not `;` between children.
6. **`view` must be pure** — no side effects, no global state, no mutexes.
7. **Separate types from rendering** — domain types crate must have zero iced/wgpu deps.
8. **Nanometer precision** — use `i64` nm internally; convert at parse/render boundary only.
9. **Canvas for schematics, wgpu shader for PCB** — CPU tessellation is fine for <10K elements; PCB needs GPU instanced rendering for 100K+ tracks/pads.
