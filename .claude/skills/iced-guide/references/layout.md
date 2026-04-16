# Iced Layout — Length, Row, Column, Container

## Length Enum

Controls how widgets fill space in a given dimension.

| Variant | Behavior |
|---------|----------|
| `Length::Fill` | Fill all available space (= `FillPortion(1)`) |
| `Length::FillPortion(n)` | Fill proportional share. Two widgets with `FillPortion(3)` and `FillPortion(2)` get 3:2 ratio |
| `Length::Shrink` | Minimum size needed to display content |
| `Length::Fixed(f32)` | Exact pixel size |

```rust
// Shorthands
Length::Fixed(15.0)
Length::from(15.0)    // Fixed
Length::from(15)      // Fixed
iced::Fill            // Length::Fill
iced::Shrink          // Length::Shrink
```

### FillPortion Example

```rust
let my_row = row![
    container("Left")
        .width(Length::FillPortion(2)),    // 40%
    container("Right")
        .width(Length::FillPortion(3)),    // 60%
].height(50.0);
```

---

## Column and Row

The two most important layout widgets. Column = vertical, Row = horizontal.

### Creating

```rust
// Macro syntax (most common)
let col = column![widget1, widget2, widget3];
let row = row![widget1, widget2, widget3];

// Function syntax
let col = Column::new().push(widget1).push(widget2);
let row = Row::new().push(widget1).push(widget2);

// From iterator
let col = Column::from_vec(items);
let col = Column::with_children(items.iter().map(|i| text(i).into()));
```

### Alignment

```rust
// Column: controls horizontal alignment of children
column![...].align_x(iced::Alignment::Center)
// Options: Start, Center, End

// Row: controls vertical alignment of children  
row![...].align_y(iced::Alignment::Center)
```

### Spacing

No margins in Iced — use `spacing()` for gaps between children.

```rust
column![widget1, widget2, widget3]
    .spacing(20)   // 20px gap between each child
```

### Wrapping

Children wrap to new lines when space runs out.

```rust
row![...].wrap()    // wraps horizontally
column![...].wrap() // wraps vertically
```

### Padding

```rust
column![...].padding(10)           // all sides
column![...].padding([10, 20])     // vertical, horizontal
column![...].padding([5, 10, 15, 20]) // top, right, bottom, left
```

### Width/Height

```rust
column![...].width(Length::Fill).height(Length::Shrink)
```

---

## Container

Wraps a single child element. Useful for alignment and centering.

```rust
use iced::{widget, Length};
use iced::alignment::{Horizontal, Vertical};

// Center content
let centered = widget::Container::new("Some Text")
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .width(Length::Fill)
    .height(Length::Fill);

// Shorthand centering
widget::container(content).center(Length::Fill)

// Alignment helpers
container(content).align_top(Length::Fill)
container(content).align_bottom(Length::Fill)
container(content).align_left(Length::Fill)
container(content).align_right(Length::Fill)
```

**Important:** `width`/`height` must provide extra space for alignment to have any visible effect. Without extra space, alignment does nothing.

---

## Debugging Layout

Use `Element::explain` to draw debug borders around elements and children:

```rust
iced::Element::new(your_widget).explain(iced::Color::BLACK)
```

This draws outlines showing how spacing and sizing are applied. Equivalent to browser inspector "show layout" in web dev.

---

## Common Layout Patterns

### Sidebar + Content

```rust
row![
    container(sidebar_content)
        .width(Length::Fixed(250.0))
        .height(Length::Fill),
    container(main_content)
        .width(Length::Fill)
        .height(Length::Fill),
]
```

### Header + Body + Footer

```rust
column![
    container(header).height(Length::Fixed(48.0)).width(Length::Fill),
    container(body).height(Length::Fill).width(Length::Fill),
    container(footer).height(Length::Fixed(24.0)).width(Length::Fill),
]
```

### Centered Card

```rust
container(
    column![title, body, buttons]
        .spacing(16)
        .padding(20)
        .width(Length::Fixed(400.0))
)
.center(Length::Fill)
```

### Equal-width columns

```rust
row![
    container(col1).width(Length::Fill),
    container(col2).width(Length::Fill),
    container(col3).width(Length::Fill),
]
.spacing(8)
```
