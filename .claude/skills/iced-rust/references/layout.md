# Layout System — iced 0.14

> iced has no unified layout engine. Each widget implements its own layout strategy.

---

## Length — sizing strategy

```rust
use iced::{Length, Fill, Shrink};

widget.width(Fill)          // = Length::Fill — expand to fill available space
widget.width(Shrink)        // = Length::Shrink — intrinsic content size (default)
widget.width(200)           // = Length::Fixed(200.0) — fixed pixels
widget.width(FillPortion(2))// proportional share relative to other Fill siblings
```

| Value | Behaviour |
|-------|-----------|
| `Fill` | Take all available space on this axis |
| `Shrink` | Use only as much space as the content needs (default) |
| `Fixed(f32)` / `200` | Exact pixel size |
| `FillPortion(u16)` | Proportional fill — `FillPortion(2)` gets twice the space of `FillPortion(1)` |

**Default**: most widgets use `Shrink`. `Fill` propagates upward — a child with `Fill` pulls its parent to `Fill` as well.

---

## column and row layout

```rust
use iced::widget::{column, row};
use iced::{Alignment, Fill};

column![widget_a, widget_b]
    .spacing(12)                    // gap between children (px)
    .padding(20)                    // inner edge spacing
    .width(Fill)
    .height(Fill)
    .align_x(Alignment::Center)     // horizontal alignment for a column

row![left, right]
    .spacing(8)
    .align_y(Alignment::Center)     // vertical alignment for a row
    .width(Fill)
```

### Unequal space distribution (FillPortion)

```rust
row![
    widget_a.width(FillPortion(1)),  // 1/3 of available width
    widget_b.width(FillPortion(2)),  // 2/3 of available width
]
```

---

## Padding

```rust
use iced::padding;

.padding(20)                                    // all four sides equal
.padding([10, 20])                              // [vertical, horizontal]
.padding([5, 10, 15, 20])                       // [top, right, bottom, left]
.padding(padding::top(10).bottom(20))
.padding(padding::vertical(30).left(20).right(80))
.padding(padding::all(10).right(0))
```

---

## Alignment

```rust
use iced::Alignment;

// Values: Center, Start, End
column![...].align_x(Alignment::Center)
row![...].align_y(Alignment::End)
```

---

## container positioning

`Container` aligns a single widget within its own bounds. The `align_*` methods
implicitly set the corresponding dimension to `Fill`:

```rust
use iced::widget::container;
use iced::Fill;

// Bottom-right corner
container(widget)
    .align_bottom(Fill)   // .align_y(Bottom).height(Fill)
    .align_right(Fill)    // .align_x(Right).width(Fill)
    .padding(10)

// Centre on both axes
container(widget).center(Fill)   // shorthand for center_x + center_y

// Top-left with explicit bounds
container(widget)
    .width(400)
    .height(300)
    .padding(20)
    .style(container::bordered_box)
```

---

## Space — flexible gaps

```rust
use iced::widget::Space;

// Flex spacer pushing items apart in a row:
row![
    left_widget,
    Space::with_width(Fill),  // push right_widget to the far right
    right_widget,
]

// Fixed gap:
Space::new(20, 10)       // 20 px wide, 10 px tall
Space::with_height(20)   // fixed vertical gap in a column
```

---

## stack — absolute overlay

```rust
use iced::widget::stack;

stack![
    background_widget,   // rendered first (bottom)
    foreground_widget,   // rendered on top
]
```

Used for the menu bar (8-menu dropdown drawn over content), modal dialogs, and
selection overlays. The `Stack` widget is the idiomatic way to implement the
`stack![]` macro overlay pattern.

---

## pin — absolute position inside stack

```rust
use iced::widget::{pin, stack};

stack![
    base_layer,
    pin(overlay_widget)
        .x(100.0)
        .y(50.0)
]
```

---

## PaneGrid — resizable split panes

```rust
use iced::widget::pane_grid::{self, PaneGrid};

PaneGrid::new(&state.panes, |_pane, content, _maximised| {
    pane_grid::Content::new(content.view())
})
.on_drag(Message::PaneDragged)
.on_resize(10, Message::PaneResized)
```

Dock systems (flat tabs with drag-to-resize panels) are typically implemented
on top of `PaneGrid` combined with custom tab bar widgets.

---

## responsive — size-aware layout

```rust
use iced::widget::responsive;

responsive(|size| {
    if size.width > 800.0 {
        two_column_layout().into()
    } else {
        single_column_layout().into()
    }
})
```

---

## Practical layout patterns

### Toolbar above scrollable content

```rust
column![
    toolbar_row,             // Shrink height
    scrollable(content),     // Fill height
]
.height(Fill)
```

### Fixed sidebar + expanding main area

```rust
row![
    sidebar.width(250),
    main_content.width(Fill),
]
.height(Fill)
```

### Modal / overlay dialog

```rust
stack![
    main_content,
    container(
        container(dialog_content)
            .padding(24)
            .style(container::bordered_box)
    )
    .center(Fill)
    .style(|theme: &Theme| container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.5).into()),
        ..Default::default()
    })
]
```

### Title bar with right-aligned action button

```rust
row![
    text("Title").size(20),
    Space::with_width(Fill),
    button("Close").on_press(Message::Close),
]
.align_y(Center)
.padding(10)
```

---

## scrollable — programmatic scroll

```rust
use iced::widget::scrollable::{self, Direction, Scrollbar};

scrollable(long_content)
    .direction(Direction::Vertical(
        Scrollbar::default()
            .width(10)
            .scroller_width(8)
    ))
    .height(Fill)
    .id(scrollable::Id::new("log-view"))

// Scroll to bottom via Task:
Task::done(scrollable::scroll_to(
    scrollable::Id::new("log-view"),
    scrollable::AbsoluteOffset { x: 0.0, y: f32::MAX },
))
```
