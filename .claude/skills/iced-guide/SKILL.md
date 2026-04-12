---
name: iced-guide
description: >
  Comprehensive Iced 0.14 GUI framework guide — ELM architecture, layout, styling,
  runtime (Tasks/Subscriptions), app structuring patterns (View-Helper, Viewable,
  Composition), custom Widget trait, file dialogs, SVG, WASM, theming, render backends.
  Covers every aspect of building Iced applications from simple counters to complex
  multi-composition apps with custom widgets. "iced widget", "iced layout", "iced theme",
  "iced task", "iced subscription", "iced canvas", "iced style", "iced pattern",
  "iced composition", "iced elm", "iced wasm", "iced custom widget", "iced overlay",
  "iced application", "iced element", "iced message" should trigger this skill.
---

# Iced 0.14 — Comprehensive Guide

> Source: Unofficial Iced Guide (jl710.github.io/iced-guide), targeting Iced 0.14
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
| SVG, render backends, WASM, file dialogs, Comet debugging | `references/platform.md` |
| iced_aw widgets: MenuBar, NumberInput, ContextMenu, Tabs, etc. | `references/iced_aw.md` |

**Building a new app:** Read architecture + layout + styling.
**Adding async/background work:** Read runtime.
**Structuring a large app:** Read patterns.
**Building a custom widget:** Read widgets.
**Cross-platform / deployment:** Read platform.

---

## Core Architecture (quick reference)

Iced uses the **ELM (MVU)** architecture:

```
State (struct) → view() → Element tree → user interaction → Message (enum) → update() → mutate State → loop
```

```rust
// Minimal app skeleton
fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("My App")
        .theme(|_| iced::Theme::Dark)
        .run()
}
```

---

## Key Types

| Type | Purpose |
|------|---------|
| `Element<'a, Message>` | Type-erased widget. All widgets convert to this via `.into()` |
| `Task<Message>` | Async action returned from `update()`. `Task::none()` for no-op |
| `Subscription<Message>` | Long-running background listener |
| `Length` | Sizing: `Fill`, `FillPortion(n)`, `Shrink`, `Fixed(f32)` |
| `Theme` | Built-in or custom theme. Implements `Base` trait |

---

## Companion Crates

| Crate | Purpose |
|-------|---------|
| `rfd` | Native file dialogs (open/save) |
| `iced_aw` | Additional community widgets |
| `iced_split` | Draggable split panels |
| `iced_table` | Table widget |
| `iced_nodegraph` | Node graph editor |
| `iced_drop` | Drag and drop |
| `iced_dialog` | Native message dialogs |

---

## Critical Notes

1. **No margins in Iced** — use `spacing()` on Row/Column, or `padding()` on Container
2. **`view()` is called after every `update()`** — widgets are recreated each frame
3. **All state changes go through `Message` → `update()`** — no interior mutability
4. **`Task` replaces old `Command`** — same concept, new name
5. **`Component` trait is deprecated** — use Composition pattern instead
6. **SVG handles should use `LazyLock`** — create once, clone the handle
7. **WASM needs `web-colors` feature** — enabled by default, fixes color rendering
8. **Theme `None` is reactive** — follows OS dark/light preference
