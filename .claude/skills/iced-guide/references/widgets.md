# Iced Custom Widget API

> Based on Iced 0.14. The Widget trait API may change in newer versions.

## When to Use

Custom widgets give full control over layout and drawing. Only use when:
- You need custom rendering (Canvas, shapes, complex visuals)
- You need custom layout logic (not achievable with Row/Column/Container)
- You need custom event handling at the primitive level

For most cases, the **Composition pattern** or **Viewable pattern** is simpler.

---

## Widget Trait Generics

```rust
impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for MyWidget
```

| Generic | Purpose | Common value |
|---------|---------|-------------|
| `Message` | Message type the widget can emit | Leave generic |
| `Theme` | Theme type for styling | `iced::Theme` or generic with bounds |
| `Renderer` | Renderer type | Generic with `impl iced::advanced::Renderer` |

### Message generic — two patterns

**Store message directly** (like Button):
```rust
pub fn on_press(mut self, on_press: Message) -> Self {
    self.on_press = Some(OnPress::Direct(on_press));
    self
}
```

**Take a closure** (like TextInput):
```rust
pub fn on_input(mut self, on_input: impl Fn(String) -> Message + 'a) -> Self {
    self.on_input = Some(Box::new(on_input));
    self
}
```

---

## Widget Trait Methods

### `state()` and `tag()` — Widget State

Widgets can be stateful or stateless. State persists across view rebuilds.
Example: ScrollBar saves scroll position.

```rust
fn state(&self) -> iced::advanced::widget::tree::State {
    iced::advanced::widget::tree::State::new(MyState::default())
}

fn tag(&self) -> iced::advanced::widget::tree::Tag {
    iced::advanced::widget::tree::Tag::of::<MyState>()
}
```

**Note:** Two widget states with the same type can be accidentally swapped.
Implement `diff()` to fix this.

### `children()` — Child Widget Trees

If your widget contains other widgets, return their state trees.
Order determines indexing in `tree.children`.

```rust
fn children(&self) -> Vec<Tree> {
    vec![Tree::new(&self.content)]
}
```

### `diff()` — State Reconciliation

Compares/reconciles old state Tree with expected state.

```rust
fn diff(&self, tree: &mut Tree) {
    tree.diff_children(&self.children);
}
```

### `size()` — Widget Size Hint

Returns the size hint used by parent layouts.

```rust
fn size(&self) -> iced::Size<iced::Length> {
    iced::Size::new(iced::Length::Shrink, iced::Length::Fill)
}
```

### `layout()` — Layout Calculation

Defines layout for this widget and children. Returns a `Node`.
Given `Limits` (min/max size) and current state `Tree`.

```rust
fn layout(
    &self,
    tree: &mut Tree,
    renderer: &Renderer,
    limits: &Limits,
) -> Node {
    // For widgets with children, call their layout methods
    let child_layout = self.content.as_widget().layout(
        &mut tree.children[0],
        renderer,
        limits,
    );
    // Return node with size and child positions
    Node::with_children(size, vec![child_layout])
}
```

**Limits:** Contains min/max space. The widget should fit within max.
If the returned Node exceeds max, subsequent widgets get less space.

**Node:** A rectangle with position, size, and optional children.
Use `node.move_to(point)` or `node.translate(vector)` to position.

### `draw()` — Rendering

Uses the Renderer to draw the widget. Access child positions via `layout.children()`.

```rust
fn draw(
    &self,
    tree: &Tree,
    renderer: &mut Renderer,
    theme: &Theme,
    style: &renderer::Style,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    viewport: &Rectangle,
) {
    // Draw background
    renderer.fill_quad(
        renderer::Quad {
            bounds: layout.bounds(),
            border: Border::default(),
            shadow: Shadow::default(),
        },
        theme.extended_palette().background.base.color,
    );

    // Draw child widgets
    self.content.as_widget().draw(
        &tree.children[0],
        renderer,
        theme,
        style,
        layout.children().next().unwrap(),
        cursor,
        viewport,
    );
}
```

### `update()` — Event Processing

Processes events. Only method that can emit messages via `Shell`.

```rust
fn update(
    &mut self,
    tree: &mut Tree,
    event: Event,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    renderer: &Renderer,
    clipboard: &mut dyn Clipboard,
    shell: &mut Shell<'_, Message>,
    viewport: &Rectangle,
) {
    // Check if cursor is over this widget
    if cursor.is_over(layout.bounds()) {
        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
            if let Some(on_press) = &self.on_press {
                shell.publish(on_press.clone());
            }
        }
    }

    // Forward events to children FIRST, then check if captured
    self.content.as_widget_mut().update(
        &mut tree.children[0],
        event,
        layout.children().next().unwrap(),
        cursor,
        renderer,
        clipboard,
        shell,
        viewport,
    );
}
```

**Important:** Call `update` on children first, then check `shell.is_event_captured()`.

**For local child messages:** Create a new `Shell` and pass it to children.

**For animations:** Request redraws with `shell.request_redraw()` until animation completes.

### `mouse_interaction()` — Cursor Style

```rust
fn mouse_interaction(
    &self,
    tree: &Tree,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    viewport: &Rectangle,
    renderer: &Renderer,
) -> mouse::Interaction {
    if cursor.is_over(layout.bounds()) {
        mouse::Interaction::Pointer
    } else {
        mouse::Interaction::default()
    }
}
```

### `overlay()` — Overlay Elements (tooltips, dropdowns)

```rust
fn overlay<'b>(
    &'b mut self,
    tree: &'b mut Tree,
    layout: Layout<'b>,
    renderer: &Renderer,
    viewport: &Rectangle,
    translation: Vector,
) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
    overlay::from_children(
        &mut self.children,
        tree,
        layout,
        renderer,
        viewport,
        translation,
    )
}
```

### `operate()` — Widget Operations

Applies operations like focus, text editing. Pass state + Id to Operation functions.

```rust
fn operate(
    &self,
    tree: &mut Tree,
    layout: Layout<'_>,
    renderer: &Renderer,
    operation: &mut dyn Operation,
) {
    operation.container(self.id.as_ref(), layout.bounds());
    operation.traverse(&mut |operation| {
        self.content.as_widget().operate(
            tree,
            layout.children().next().unwrap(),
            renderer,
            operation,
        );
    });
}
```

---

## Operations

Operations query and update widget state.

### Built-in operations
- `Focusable` — focus/unfocus
- `TextInput` — text manipulation

### Custom operations

1. Define a trait for your operation
2. Implement `Operation` trait with `custom` method
3. Downcast state to your trait in `custom`
4. Launch via `Task::operate(...)`

```rust
// Convention: create a trait
trait Scrollable {
    fn scroll_to(&mut self, offset: f32);
}

// Your state implements it
impl Scrollable for MyState {
    fn scroll_to(&mut self, offset: f32) {
        self.scroll_offset = offset;
    }
}
```

---

## Storing Children

Store child widgets as `Element<'a, Message>` in your widget struct.
When calling methods on children, pass correct child tree from `tree.children`.

```rust
pub struct MyWidget<'a, Message> {
    content: Element<'a, Message>,
    header: Element<'a, Message>,
}
```

---

## Displaying Text

Displaying text directly via renderer is complex.
**Recommended:** Use a `text` widget as a child element instead.
