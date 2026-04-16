# Iced 0.14 — Canvas, Shader, PaneGrid, Window, Keyboard

> Source: Official docs.rs/iced/0.14, iced examples (clock, bezier_tool, game_of_life, pane_grid)

## Canvas Widget

Interactive 2D graphics. Requires `canvas` feature.

### The `canvas::Program` Trait

```rust
impl canvas::Program<Message> for MyProgram {
    type State = MyCanvasState;  // must be Default + 'static

    // Required: render geometry
    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry>;

    // Optional: handle events
    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>>;

    // Optional: cursor style
    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction;
}
```

### Canvas Events
```rust
canvas::Event::Mouse(mouse::Event)
canvas::Event::Touch(touch::Event)
canvas::Event::Keyboard(keyboard::Event)
```

### Canvas Actions
```rust
canvas::Action::publish(message)       // send message to app
canvas::Action::capture()              // capture event (stop propagation)
canvas::Action::request_redraw()       // request a redraw
action.and_capture()                   // chain: publish + capture
```

### Frame Drawing API
```rust
let mut frame = canvas::Frame::new(renderer, bounds.size());

// Shapes
frame.fill(&path, color);
frame.stroke(&path, stroke);
frame.fill_rectangle(point, size, color);

// Text
frame.fill_text(canvas::Text {
    content: "Hello".into(),
    position: Point::new(10.0, 20.0),
    color: Color::WHITE,
    size: Pixels(14.0),
    font: Font::DEFAULT,
    ..canvas::Text::default()
});

// Transforms
frame.translate(Vector::new(dx, dy));
frame.rotate(radians);
frame.scale(factor);
frame.with_save(|frame| { /* scoped transform */ });

// Info
frame.center()       // Point at center
frame.width()        // f32
frame.height()       // f32
frame.size()         // Size

frame.into_geometry()  // finalize
```

### Path Construction
```rust
Path::line(from, to)
Path::circle(center, radius)
Path::rectangle(point, size)
Path::new(|p| {
    p.move_to(point);
    p.line_to(point2);
    p.quadratic_curve_to(control, to);
    p.bezier_curve_to(cp1, cp2, to);
    p.arc_to(point, radius);
    p.arc(arc);          // Arc { center, radius, start_angle, end_angle }
    p.ellipse(ellipse);  // Ellipse { center, radii, rotation, start_angle, end_angle }
    p.close();
})
```

### Stroke Configuration
```rust
canvas::Stroke::default()
    .with_color(Color::WHITE)
    .with_width(2.0)
    .with_line_cap(LineCap::Round)
    .with_line_join(LineJoin::Round)
    .with_line_dash(LineDash { segments: &[5.0, 3.0], offset: 0 })
```

### Cache Pattern (critical for performance)
```rust
struct MyApp {
    cache: canvas::Cache,  // cached geometry
}

// Clear when data changes (in update):
self.cache.clear();

// Use in draw (only re-renders when cleared):
let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
    // expensive drawing here
});
```

### Multi-Layer Cache Pattern (used in signex)
```rust
// Layer 1: background (grid) — clear on zoom/pan
let bg = self.bg_cache.draw(renderer, size, |frame| { ... });
// Layer 2: content (schematic) — clear on data change
let content = self.content_cache.draw(renderer, size, |frame| { ... });
// Layer 3: overlay (selection, cursor) — clear every frame
let overlay = canvas::Frame::new(renderer, size);
// ... draw cursor, selection ...
vec![bg, content, overlay.into_geometry()]
```

### Pan/Zoom Pattern (from game_of_life)
```rust
// State
struct Camera {
    offset: Vector,
    scale: f32,
}

// In canvas::Program::update:
// Right-drag → pan
mouse::Event::ButtonPressed(mouse::Button::Right) => {
    state.panning = true;
    state.last_pan_pos = Some(cursor_pos);
}
mouse::Event::CursorMoved { .. } if state.panning => {
    let delta = cursor_pos - last_pos;
    camera.pan(delta.x, delta.y);
}
// Scroll → zoom toward cursor
mouse::Event::WheelScrolled { delta } => {
    let factor = 1.0 + scroll_y / 30.0;
    camera.zoom_at(cursor_pos, factor);
}

// In canvas::Program::draw:
// Apply camera transform to frame
let screen_pos = Point::new(
    (world.x - camera.offset.x) * camera.scale + bounds.width / 2.0,
    (world.y - camera.offset.y) * camera.scale + bounds.height / 2.0,
);
```

---

## Shader Widget

Custom wgpu rendering. Requires `wgpu` feature.

```rust
use iced::widget::shader;

struct MyShader { /* app data */ }

impl shader::Program<Message> for MyShader {
    type State = ();
    type Primitive = MyPrimitive;

    fn draw(&self, state: &(), bounds: Rectangle, cursor: Cursor) -> Self::Primitive { ... }
    fn update(&self, state: &mut (), event: &Event, bounds: Rectangle, cursor: Cursor) -> Option<Action<Message>> { ... }
}

struct MyPrimitive { /* vertex data */ }

impl shader::Primitive for MyPrimitive {
    fn prepare(&self, device: &Device, queue: &Queue, format: TextureFormat, storage: &mut Storage, bounds: &Rectangle, viewport: &Viewport) { ... }
    fn render(&self, encoder: &mut CommandEncoder, storage: &Storage, target: &TextureView, clip_bounds: &Rectangle) { ... }
}
```

---

## PaneGrid Widget

Dynamic pane splitting (tiling window manager pattern).

```rust
use iced::widget::pane_grid;

struct App {
    panes: pane_grid::State<PaneData>,
}

// Create
let (state, first_pane) = pane_grid::State::new(PaneData::new());

// View
PaneGrid::new(&self.panes, |id, pane, is_maximized| {
    let title_bar = pane_grid::TitleBar::new(text(&pane.title))
        .controls(pane_grid::Controls::dynamic(controls_view, close_btn))
        .padding(10);

    pane_grid::Content::new(pane_content)
        .title_bar(title_bar)
})
.width(Fill).height(Fill)
.spacing(10)
.on_click(Message::PaneClicked)
.on_drag(Message::PaneDragged)
.on_resize(10, Message::PaneResized)

// State API
state.split(Axis::Horizontal, pane_id, new_data)  // split pane
state.close(pane_id)                                // close pane
state.resize(split_id, ratio)                       // resize split
state.drop(dragged, target)                         // drag-drop pane
state.maximize(pane_id)                             // maximize
state.restore()                                     // un-maximize
state.adjacent(pane_id, Direction::Right)            // find neighbor
state.panes()                                        // iterate all panes
```

---

## Window Module

### Window Tasks (return Task<Message>)
```rust
window::open(settings)                // open new window → returns Id
window::close(id)                     // close window
window::move_to(id, point)            // move window
window::resize(id, size)              // resize
window::gain_focus(id)                // focus
window::minimize(id, bool)            // minimize
window::maximize(id, bool)            // maximize
window::toggle_maximize(id)
window::set_icon(id, icon)
window::toggle_decorations(id)
window::set_level(id, Level)          // AlwaysOnTop etc.
window::drag(id)                      // start OS drag
window::screenshot(id)                // capture screenshot
```

### Window Queries (return Task<T>)
```rust
window::size(id)                      // → Size
window::position(id)                  // → Point
window::scale_factor(id)              // → f64
window::is_maximized(id)              // → bool
window::is_minimized(id)              // → bool
```

### Window Subscriptions
```rust
window::events()                      // all window events
window::resize_events()               // (Id, Size)
window::close_events()                // Id
window::close_requests()              // Id (before close)
window::open_events()                 // Id
window::frames()                      // animation frames
```

---

## Keyboard Module

### Listen to keyboard events
```rust
fn subscription(&self) -> Subscription<Message> {
    keyboard::listen().filter_map(|event| {
        let keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
            return None;
        };
        match (key.as_ref(), modifiers.command()) {
            (keyboard::Key::Character("s"), true) => Some(Message::Save),
            (keyboard::Key::Character("z"), true) => Some(Message::Undo),
            (keyboard::Key::Named(keyboard::key::Named::Delete), false) => Some(Message::Delete),
            (keyboard::Key::Named(keyboard::key::Named::Escape), false) => Some(Message::Cancel),
            _ => None,
        }
    })
}
```

### Key types
```rust
keyboard::Key::Character("a")         // letter/number keys
keyboard::Key::Named(Named::Enter)    // named keys
keyboard::Key::Named(Named::Tab)
keyboard::Key::Named(Named::Space)
keyboard::Key::Named(Named::Delete)
keyboard::Key::Named(Named::Escape)
keyboard::Key::Named(Named::Home)
keyboard::Key::Named(Named::ArrowUp)
keyboard::Key::Named(Named::F1)       // function keys
```

### Modifiers
```rust
modifiers.command()    // Ctrl on Windows/Linux, Cmd on macOS
modifiers.shift()
modifiers.alt()
modifiers.control()    // always Ctrl (even on macOS)
```

---

## Mouse Module

### Mouse interactions (cursor styles)
```rust
mouse::Interaction::default()          // arrow
mouse::Interaction::Pointer           // hand
mouse::Interaction::Grab              // open hand
mouse::Interaction::Grabbing          // closed hand
mouse::Interaction::Crosshair         // crosshair
mouse::Interaction::Text              // I-beam
mouse::Interaction::Move              // 4-way arrow
mouse::Interaction::ResizingHorizontally
mouse::Interaction::ResizingVertically
```

### Cursor position in canvas
```rust
cursor.position_in(bounds)            // Option<Point> — relative to bounds
cursor.position()                     // Option<Point> — absolute
cursor.is_over(bounds)                // bool
```

---

## Overlay API

For widgets that display content on top of other widgets (tooltips, dropdowns, popups).

```rust
trait Overlay<Message, Theme, Renderer> {
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> Node;
    fn draw(&self, renderer: &mut Renderer, theme: &Theme, style: &Style, layout: Layout, cursor: Cursor);

    // Optional
    fn update(&mut self, event: Event, layout: Layout, cursor: Cursor, ...) { }
    fn mouse_interaction(&self, ...) -> mouse::Interaction { default() }
    fn overlay(&mut self, ...) -> Option<Element> { None }  // nested overlays
}
```

**Helper:** `overlay::from_children()` — delegates to child widget overlays.

---

## Time Module

```rust
use iced::time;

// Periodic subscription (animation timer, polling)
fn subscription(&self) -> Subscription<Message> {
    time::every(Duration::from_millis(16))  // ~60fps
        .map(|_| Message::Tick)
}
```
