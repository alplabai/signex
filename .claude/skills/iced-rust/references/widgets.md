# Widget Reference — iced 0.14

---

## Core concepts

- Widgets are **values** — constructing them has no side effects.
- Helper function → concrete type: `button("x")` → `Button<Message>`
- Every widget converts to `Element<Message>` via `.into()`.
- Most widgets use a builder pattern: `button("x").on_press(msg).style(...)`.

---

## text

```rust
use iced::widget::text;
use iced::{Fill, Font};

text("Hello!")
    .font(Font::MONOSPACE)
    .size(24)           // logical pixels
    .line_height(1.5)   // multiple of font size
    .width(Fill)
    .height(Fill)
    .center()           // shorthand for .align_x(Center).align_y(Center)
    .color(Color::from_rgb(1.0, 0.0, 0.0))

// Dynamic format (same as format! but returns Text):
text!("Value: {}", state.value)

// Built-in theme styles:
text("Warning!").style(text::warning)
// Others: text::danger, text::success, text::secondary, text::primary

// Custom color from current theme:
text("Primary color").style(|theme: &Theme| text::Style {
    color: Some(theme.palette().primary),
})
```

`&str` converts to `Element` automatically — `container("Hello")` works without `text()`.

---

## button

```rust
use iced::widget::button;

button("Click")
    .on_press(Message::Clicked)
    // .on_press_maybe(Option<Message>) — None disables the button
    .padding(10)
    .style(button::primary)  // built-in: primary, secondary, success, danger, text

// Custom style with status:
button("Custom").style(|theme: &Theme, status| {
    let p = theme.extended_palette();
    match status {
        button::Status::Active   => button::Style::default()
            .with_background(p.success.strong.color),
        button::Status::Hovered  => button::primary(theme, status),
        button::Status::Pressed  => button::primary(theme, status),
        button::Status::Disabled => button::primary(theme, status),
    }
})
```

---

## column and row

```rust
use iced::widget::{column, row};
use iced::Alignment;

// Macro syntax (preferred):
column![
    widget_a,
    widget_b,
    widget_c,
].spacing(10).padding(20)

row![
    left_widget,
    right_widget,
].spacing(8).align_y(Alignment::Center)

// Programmatic (iterator):
let items: Vec<Element<_>> = data.iter().map(|d| text(d).into()).collect();
Column::with_children(items).spacing(5).into()
```

`column!` / `row!` methods:

| Method | Description |
|--------|-------------|
| `.spacing(f32)` | Gap between children (px) |
| `.padding(impl Into<Padding>)` | Inner edge spacing |
| `.width(Length)` | Width strategy |
| `.height(Length)` | Height strategy |
| `.align_x(Alignment)` | Horizontal alignment (column) |
| `.align_y(Alignment)` | Vertical alignment (row) |
| `.push(widget)` | Append a child programmatically |
| `.extend(iter)` | Append from an iterator |

---

## container

```rust
use iced::widget::container;
use iced::{Fill, padding};

container(content_widget)
    .padding(20)
    .padding(padding::vertical(30).left(20).right(80))
    .width(Fill)
    .height(Fill)
    .align_x(Center)
    .align_y(Center)
    .center(Fill)           // shorthand for both axes
    .style(container::bordered_box)
    // built-in: bordered_box, rounded_box, primary, background, transparent
```

---

## scrollable

```rust
use iced::widget::scrollable::{self, Direction, Scrollbar};

scrollable(
    column![/* long content */].spacing(10)
)
.direction(Direction::Vertical(Scrollbar::default()))
.height(Fill)
.id(scrollable::Id::new("my-list"))
```

---

## text_input

```rust
use iced::widget::text_input;

text_input("Placeholder...", &state.input_value)
    .on_input(Message::InputChanged)   // fn(String) -> Message
    .on_submit(Message::Submitted)
    .password()                         // mask input
    .padding(10)
    .width(Fill)
    .id(text_input::Id::new("search-box"))
```

---

## slider

```rust
use iced::widget::slider;

slider(0..=100, state.value, Message::ValueChanged)
    .step(1u8)
    .width(Fill)
```

---

## checkbox and toggler

```rust
use iced::widget::{checkbox, toggler};

checkbox("Option A", state.is_checked)
    .on_toggle(Message::Toggled)   // fn(bool) -> Message

toggler(state.is_on)
    .on_toggle(Message::Toggled)
    .label("On / Off")
```

---

## pick_list

```rust
use iced::widget::pick_list;

pick_list(
    &["Option A", "Option B", "Option C"][..],
    state.selected.as_ref(),       // Option<&T>
    Message::Selected,             // fn(T) -> Message
)
.placeholder("Select...")
```

---

## radio

```rust
use iced::widget::radio;

column![
    radio("Option 1", MyEnum::A, Some(state.selected), Message::Selected),
    radio("Option 2", MyEnum::B, Some(state.selected), Message::Selected),
]
```

---

## Space

```rust
use iced::widget::Space;

Space::new(10, 20)           // fixed width x height
Space::with_width(Fill)      // flexible horizontal spacer (flex-grow)
Space::with_height(20)       // fixed vertical gap
```

---

## image

```rust
use iced::widget::image;
// Feature: iced = { features = ["image"] }

image("assets/logo.png")
    .width(200)
    .height(200)
    .content_fit(ContentFit::Cover)  // Cover, Contain, Fill, ScaleDown, None
```

---

## svg

```rust
use iced::widget::svg;
// Feature: iced = { features = ["svg"] }

svg("assets/icon.svg")
    .width(100)
    .height(100)
```

---

## canvas

```rust
use iced::widget::canvas::{self, Canvas, Frame, Path, Stroke};
use iced::{Point, Color};
// Feature: iced = { features = ["canvas"] }

struct MyDrawing;

impl<Message> canvas::Program<Message> for MyDrawing {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: canvas::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let circle = Path::circle(frame.center(), 50.0);
        frame.fill(&circle, Color::from_rgb(0.0, 0.5, 1.0));
        frame.stroke(&circle, Stroke::default().with_width(2.0));
        vec![frame.into_geometry()]
    }
}

// In view:
Canvas::new(MyDrawing)
    .width(Fill)
    .height(Fill)
```

See `references/canvas-advanced.md` for cache layers, pan/zoom, hit testing.

---

## tooltip

```rust
use iced::widget::tooltip;

tooltip(
    button("?"),
    "Hint text shown on hover",
    tooltip::Position::Bottom,  // Top, Bottom, Left, Right, FollowCursor
)
```

---

## responsive

```rust
use iced::widget::responsive;

responsive(|size| {
    // size: iced::Size — available space
    if size.width > 800.0 {
        wide_layout().into()
    } else {
        narrow_layout().into()
    }
})
```

---

## lazy

```rust
use iced::widget::lazy;

// Only recomputes when expensive_data changes.
lazy(&state.expensive_data, |data| {
    build_expensive_widget(data)
})
```

---

## Widget list (quick reference)

| Widget | Function / Macro | Notes |
|--------|-----------------|-------|
| Text | `text(content)` | `text!("fmt {}", v)` |
| Button | `button(content)` | `.on_press(msg)` |
| Vertical layout | `column![...]` | `.spacing()` |
| Horizontal layout | `row![...]` | `.spacing()` |
| Wrapper | `container(content)` | padding, align, style |
| Scroll area | `scrollable(content)` | |
| Text input | `text_input("ph", &val)` | `.on_input(fn)` |
| Password field | `text_input(...).password()` | |
| Multi-line editor | `text_editor(&content)` | |
| Slider | `slider(range, val, fn)` | |
| Checkbox | `checkbox("label", val)` | `.on_toggle(fn)` |
| Toggle switch | `toggler(val)` | `.on_toggle(fn)` |
| Dropdown | `pick_list(opts, sel, fn)` | |
| Radio button | `radio("l", v, sel, fn)` | |
| Progress bar | `progress_bar(range, val)` | |
| Spacer | `Space::new(w, h)` | |
| Image | `image("path")` | feature: image |
| SVG | `svg("path")` | feature: svg |
| Canvas | `Canvas::new(program)` | feature: canvas |
| Tooltip | `tooltip(w, "txt", pos)` | |
| Responsive | `responsive(\|sz\| w)` | |
| Lazy | `lazy(&dep, \|d\| w)` | |
| QR Code | `qr_code(&data)` | feature: qr_code |
| Stack (layers) | `stack![bottom, top]` | absolute overlay |
| Mouse area | `mouse_area(content)` | click/hover region |
| Pin | `pin(content)` | absolute position inside stack |
| Pane grid | `pane_grid(&state, fn)` | resizable split panes |
