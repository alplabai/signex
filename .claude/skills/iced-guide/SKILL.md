---
name: iced-guide
description: >
  Comprehensive Iced 0.14 GUI framework guide — ELM architecture, layout, styling,
  runtime (Tasks/Subscriptions), app structuring patterns (View-Helper, Viewable,
  Composition), custom Widget trait, Canvas/Shader, PaneGrid, file dialogs, SVG, WASM,
  theming, render backends, window management, keyboard/mouse events.
  Covers every aspect of building Iced applications from simple counters to complex
  multi-composition apps with custom widgets. Sources: official iced book (book.iced.rs),
  docs.rs/iced/0.14, unofficial guide (jl710.github.io/iced-guide).
  "iced widget", "iced layout", "iced theme", "iced task", "iced subscription",
  "iced canvas", "iced style", "iced pattern", "iced composition", "iced elm",
  "iced wasm", "iced custom widget", "iced overlay", "iced application",
  "iced element", "iced message", "iced pane_grid", "iced shader",
  "iced keyboard", "iced window" should trigger this skill.
---

# Iced 0.14 — Comprehensive Guide

> Sources: Official book (book.iced.rs), docs.rs/iced/0.14, Unofficial Guide
> Rust 1.88+ required

---

## Which reference to read

| Task | Reference |
|------|-----------|
| ELM architecture, State/Message/Update/View, app setup | `references/architecture.md` |
| Layout: Length, Row, Column, Container, alignment, spacing | `references/layout.md` |
| Themes, custom themes, per-widget styling, Catalog | `references/styling.md` |
| Tasks, Subscriptions, blocking code, streams, channels | `references/runtime.md` |
| App structure: View-Helper, Viewable, Composition patterns | `references/patterns.md` |
| Custom Widget trait: state, layout, draw, update, overlay | `references/widgets.md` |
| Canvas, Shader, PaneGrid, Window, Keyboard APIs | `references/advanced_widgets.md` |
| SVG, render backends, WASM, file dialogs, Comet debugging | `references/platform.md` |
| Complete widget module: all 42 widgets with API | `references/widget_catalog.md` |
| iced_aw widgets: MenuBar, NumberInput, ContextMenu, Tabs | `references/iced_aw.md` |

**Building a new app:** Read architecture + layout + styling.
**Adding async/background work:** Read runtime.
**Structuring a large app:** Read patterns.
**Building a custom widget:** Read widgets.
**Canvas / interactive 2D graphics:** Read advanced_widgets.
**Cross-platform / deployment:** Read platform.

---

## Core Architecture (quick reference)

Iced uses the **ELM (MVU)** architecture:

```
State (struct) → view() → Element tree → user interaction → Message (enum) → update() → mutate State → loop
```

### Simple app (State = Default)

```rust
fn main() -> iced::Result {
    iced::run(App::update, App::view)
}
```

### Full app (with boot, title, theme, subscriptions)

```rust
fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .subscription(App::subscription)
        .window_size(iced::Size::new(1400.0, 900.0))
        .font(include_bytes!("font.ttf"))
        .run()
}
```

---

## All 42 Built-in Widgets

### Layout
`column!`, `row!`, `stack!`, `grid!`, `container`, `scrollable`, `space`, `float`, `pin`,
`center`, `center_x`, `center_y`, `top`, `bottom`, `left`, `right`

### Input
`button`, `text_input`, `text_editor`, `checkbox`, `toggler`, `radio`,
`slider`, `vertical_slider`, `pick_list`, `combo_box`

### Display
`text`, `rich_text!`, `image`, `svg`, `progress_bar`, `rule`, `qr_code`, `markdown`, `tooltip`

### Advanced
`canvas`, `shader`, `pane_grid`, `responsive`, `lazy`, `mouse_area`, `sensor`,
`hover`, `opaque`, `themer`, `keyed_column`

---

## Key Types

| Type | Purpose |
|------|---------|
| `Element<'a, Message>` | Type-erased widget, all widgets convert via `.into()` |
| `Task<Message>` | Async action from `update()`. `Task::none()`, `.perform()`, `.future()`, `.batch()` |
| `Subscription<Message>` | Long-running listener. `::run()`, `::batch()`, `.map()`, `.filter_map()` |
| `Length` | Sizing: `Fill`, `FillPortion(n)`, `Shrink`, `Fixed(f32)` |
| `Theme` | 23 built-in + `Theme::custom()` + `Theme::custom_with_fn()` |
| `Color` | sRGB. `::from_rgb8()`, `::from_rgba()`, `color!()` macro |
| `Font` | `Font::DEFAULT`, `Font::MONOSPACE`, custom via `.font()` |

---

## Task API (complete)

```rust
Task::none()                          // no-op
Task::done(msg)                       // instant message
Task::perform(future, map_fn)         // run Future, map result
Task::future(future)                  // run Future returning Message directly
Task::run(stream, map_fn)             // run Stream, map items
Task::stream(stream)                  // run Stream producing Messages
Task::batch(vec![t1, t2])             // parallel tasks
task.map(fn)                          // transform output
task.then(fn)                         // monadic chain
task.chain(task2)                     // sequential chain
task.collect()                        // collect Vec
task.discard()                        // discard result
task.abortable()                      // returns (Task, AbortHandle)
```

---

## Subscription API (complete)

```rust
Subscription::none()                  // empty
Subscription::run(builder_fn)         // from Stream builder
Subscription::run_with(data, fn)      // with data for identity
Subscription::batch(vec)              // combine
sub.with(value)                       // add to identity
sub.map(fn)                           // transform
sub.filter_map(fn)                    // transform + filter
```

**Built-in subscriptions:**
- `keyboard::listen()` — all keyboard events
- `iced::event::listen()` — all UI events
- `window::events()`, `window::resize_events()`, `window::close_events()`
- `time::every(duration)` — periodic timer

---

## Application Builder (complete)

```rust
iced::application(boot, update, view)
    .title(fn)                        // window title
    .theme(fn)                        // Theme from state
    .style(fn)                        // application background style
    .subscription(fn)                 // subscriptions from state
    .scale_factor(fn)                 // DPI scaling
    .antialiasing(true)               // MSAA
    .centered()                       // center window on screen
    .window_size(Size)                // initial window size
    .window(window::Settings{..})     // full window settings
    .transparent(bool)                // transparent window
    .resizable(bool)                  // resizable window
    .decorations(bool)                // window decorations
    .default_font(Font)               // default font
    .font(bytes)                      // register font bytes
    .run()                            // start the app
```

---

## Critical Notes

1. **No margins** — use `spacing()` on Row/Column, `padding()` on Container
2. **`view()` is called after every `update()`** — widgets are recreated each frame
3. **All state changes through Message → update()** — no interior mutability
4. **`Task` replaces old `Command`** — same concept, new name
5. **`Component` trait is deprecated** — use Composition pattern
6. **SVG handles: use `LazyLock`** — create once, clone cheaply
7. **Reactive rendering since 0.14** — no constant redraw
8. **Theme `None` is reactive** — follows OS dark/light preference
9. **String slices implement `Into<Element>`** — use `container("text")` directly
10. **`text!` macro** — like `format!` but returns `Text` widget
