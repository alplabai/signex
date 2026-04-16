# Iced 0.14 — Complete Widget Catalog (42 Widgets)

> Source: docs.rs/iced/0.14, official book, examples

## Layout Widgets

### `column![]` / `Column`
Vertical layout. Children stacked top-to-bottom.
```rust
column![widget1, widget2].spacing(10).padding(8).align_x(Center).width(Fill)
```
Methods: `.spacing()`, `.padding()`, `.align_x()`, `.width()`, `.height()`, `.wrap()`, `.push()`, `.push_maybe()`

### `row![]` / `Row`
Horizontal layout. Children side-by-side.
```rust
row![widget1, widget2].spacing(10).align_y(Center).width(Fill)
```
Methods: same as Column but `.align_y()` instead of `.align_x()`

### `stack![]` / `Stack`
Z-axis stacking. Later children rendered on top.
```rust
stack![background, foreground_overlay]
```

### `container` / `Container`
Single-child wrapper for alignment, padding, styling.
```rust
container(content)
    .padding(10)
    .center(Fill)                    // center both axes, expand to Fill
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .align_right(Fill)               // shorthand: right-align + Fill width
    .align_bottom(Fill)              // shorthand: bottom-align + Fill height
    .width(Fill).height(Fill)
    .max_width(400.0)
    .style(container::rounded_box)   // built-in style
```

### `scrollable` / `Scrollable`
Scrollable content area. Vertical by default.
```rust
scrollable(column![...]).width(Fill).height(300)
```

### `space` / `Space`
Explicit spacing widget.
```rust
Space::new().width(Fill)   // horizontal spacer
Space::new().height(20)    // vertical spacer
```

### Centering helpers
```rust
center(content)              // center both axes, Fill both
center_x(content, Fill)      // center horizontally
center_y(content, Fill)      // center vertically
```

### Positioning helpers
```rust
top(content)                 // align to top
bottom(content)              // align to bottom
left(content)                // align to left
right(content)               // align to right
```

---

## Input Widgets

### `button` / `Button`
```rust
button("Click me")
    .on_press(Message::Clicked)
    .on_press_maybe(if enabled { Some(msg) } else { None })
    .padding([8, 16])
    .width(Fill)
    .style(button::primary)     // primary, secondary, success, warning, danger, text
```

### `text_input` / `TextInput`
```rust
text_input("Placeholder...", &self.value)
    .on_input(Message::InputChanged)
    .on_submit(Message::Submitted)
    .on_paste(Message::Pasted)
    .size(14)
    .padding(8)
    .width(Fill)
    .secure(true)               // password mode
    .font(Font::MONOSPACE)
    .icon(text_input::Icon { ... })
```

### `text_editor` / `TextEditor`
Multi-line text editor.
```rust
text_editor(&self.content)
    .on_action(Message::EditorAction)
    .height(Fill)
    .font(Font::MONOSPACE)
    .highlight::<Highlighter>(settings, |h, _theme| h.to_format())
```

### `checkbox` / `Checkbox`
```rust
checkbox("Label", self.checked)
    .on_toggle(Message::Toggled)
    .size(16)
    .spacing(8)
    .text_size(14)
```

### `toggler` / `Toggler`
Toggle switch.
```rust
toggler(self.enabled)
    .on_toggle(Message::Toggled)
    .label("Enable feature")
    .size(20)
    .spacing(10)
```

### `radio` / `Radio`
```rust
column(choices.iter().map(|choice| {
    radio(choice.label, *choice, self.selected, Message::Selected).into()
}))
```

### `slider` / `Slider`
```rust
slider(0.0..=100.0, self.value, Message::ValueChanged)
    .step(0.5)
    .width(200)
```

### `pick_list` / `PickList`
Dropdown selection.
```rust
pick_list(&options[..], self.selected, Message::Selected)
    .placeholder("Choose...")
    .width(200)
```

### `combo_box` / `ComboBox`
Searchable dropdown with filtering.
```rust
combo_box(&self.state, "Search...", self.selected.as_ref(), Message::Selected)
    .on_input(Message::InputChanged)
    .width(200)
```

---

## Display Widgets

### `text` / `Text`
```rust
text("Hello!")
    .size(20)
    .font(Font::MONOSPACE)
    .line_height(1.5)
    .width(Fill)
    .center()                    // center text within bounds
    .style(text::primary)        // primary, secondary, success, danger
    .wrapping(text::Wrapping::None)
```
**`text!` macro:** `text!("Count: {}", self.count)` — like `format!` returning Text.

### `rich_text![]`
Rich text with styled spans.
```rust
rich_text![
    span("Bold").font(Font { weight: Weight::Bold, ..Font::DEFAULT }),
    span(" and "),
    span("colored").color(Color::from_rgb(1.0, 0.0, 0.0)),
]
```

### `image` / `Image`
Raster image (requires `image` feature).
```rust
image(Handle::from_path("photo.png")).width(200)
image(Handle::from_bytes(bytes))
```

### `svg` / `Svg`
Vector graphics (requires `svg` feature).
```rust
svg(svg::Handle::from_memory(include_bytes!("icon.svg")))
    .width(24).height(24)
```

### `progress_bar` / `ProgressBar`
```rust
progress_bar(0.0..=100.0, self.progress).width(Fill).height(8)
```

### `rule` / `Rule`
Horizontal/vertical divider.
```rust
rule::horizontal(1)    // 1px horizontal line
rule::vertical(1)      // 1px vertical line
```

### `tooltip` / `Tooltip`
```rust
tooltip(button("?"), "Help text", tooltip::Position::Top)
    .gap(5)
    .style(container::rounded_box)
```

### `markdown`
Markdown rendering (requires `markdown` feature).
```rust
markdown(&self.items).map(Message::LinkClicked)
```

---

## Advanced Widgets

### `canvas` / `Canvas`
See `references/advanced_widgets.md` for full Canvas API.
```rust
canvas(&self.my_program).width(Fill).height(Fill)
```

### `shader` / `Shader`
Custom wgpu shader widget. See `references/advanced_widgets.md`.

### `pane_grid` / `PaneGrid`
Dynamic pane splitting. See `references/advanced_widgets.md`.

### `responsive` / `Responsive`
Size-aware widget — rebuilds when container size changes.
```rust
responsive(|size| {
    if size.width > 600.0 { wide_layout() } else { narrow_layout() }
})
```

### `lazy` / `Lazy`
Deferred rendering — only rebuilds when dependency changes.
```rust
lazy(self.data_version, |_| { expensive_widget_tree() })
```

### `mouse_area` / `MouseArea`
Captures mouse events on a region.
```rust
mouse_area(content)
    .on_press(Message::Pressed)
    .on_release(Message::Released)
    .on_enter(Message::Entered)
    .on_exit(Message::Exited)
    .on_move(Message::Moved)
    .interaction(mouse::Interaction::Pointer)
```

### `hover`
Simple hover detection.
```rust
hover(content, |is_hovered| {
    if is_hovered { styled_content() } else { content }
})
```

### `opaque`
Marks a widget as opaque (prevents events passing through in Stack).
```rust
opaque(overlay_widget)
```

### `themer` / `Themer`
Override theme for a subtree.
```rust
themer(Theme::Dark, dark_themed_content)
```

### `keyed_column!` / `keyed`
Preserves widget state across rebuilds using keys.
```rust
keyed_column(items.iter().map(|item| {
    (item.id, item_widget(item))
}))
```
