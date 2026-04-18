# Canvas — Advanced Patterns

> Source: iced/examples/bezier_tool, iced/examples/solar_system,
> iced/examples/sierpinski_triangle

---

## canvas::Cache — performant rendering

`canvas::Cache` stores the rendered result and only redraws when `.clear()` is called
or the canvas size changes. Essential for static-heavy content like schematics and PCBs.

```rust
use iced::widget::canvas;

#[derive(Default)]
pub struct SchematicLayer {
    cache: canvas::Cache,
}

impl SchematicLayer {
    /// Call when state changes to trigger a redraw on the next frame.
    pub fn request_redraw(&mut self) {
        self.cache.clear();
    }
}
```

### Multiple cache layers

Split layers into separate caches to minimise redraw work:

```rust
#[derive(Default)]
pub struct BoardView {
    copper_cache:  canvas::Cache,  // tracks / pads / zones — rarely changes
    overlay_cache: canvas::Cache,  // selection highlights — changes often
    grid_cache:    canvas::Cache,  // grid lines — redrawn on pan/zoom
}

impl canvas::Program<Message> for BoardView {
    type State = ViewState;

    fn draw(
        &self,
        state: &ViewState,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: canvas::Cursor,
    ) -> Vec<canvas::Geometry> {
        let copper = self.copper_cache.draw(renderer, bounds.size(), |frame| {
            draw_copper_layers(frame, &state.board, &state.viewport);
        });

        let grid = self.grid_cache.draw(renderer, bounds.size(), |frame| {
            draw_grid(frame, &state.viewport, bounds.size());
        });

        let overlay = self.overlay_cache.draw(renderer, bounds.size(), |frame| {
            draw_selection_overlay(frame, &state.selected, &state.viewport);
        });

        // Draw order: grid → copper → overlay
        vec![grid, copper, overlay]
    }
}
```

---

## canvas::Program — full interactive implementation

From bezier_tool and adapted for EDA use:

```rust
use iced::mouse;
use iced::widget::canvas::{self, Canvas, Event, Frame, Geometry, Path, Stroke};
use iced::{Color, Element, Fill, Point, Rectangle, Renderer, Theme};

/// Internal per-canvas state stored in the widget tree (not in app State).
#[derive(Default)]
pub struct InteractionState {
    drag_start: Option<Point>,
    cursor_pos: Option<Point>,
}

pub struct SchematicCanvas<'a> {
    cache:    &'a canvas::Cache,
    items:    &'a [SchItem],
    viewport: &'a Viewport,
}

#[derive(Debug, Clone, Copy)]
pub enum CanvasEvent {
    Pan(f32, f32),          // dx, dy in canvas pixels
    ZoomAt(f32, Point),     // factor, pivot
    ItemClicked(ItemId),
    CursorMoved(Point),     // schematic coordinates
}

impl<'a> canvas::Program<CanvasEvent> for SchematicCanvas<'a> {
    type State = InteractionState;

    fn update(
        &self,
        state: &mut InteractionState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<CanvasEvent>> {
        let pos = cursor.position_in(bounds)?;

        match event {
            // Right-button drag → pan
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                state.drag_start = Some(pos);
                Some(canvas::Action::request_redraw())
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Right)) => {
                state.drag_start = None;
                Some(canvas::Action::request_redraw())
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                state.cursor_pos = Some(pos);
                if let Some(drag) = state.drag_start {
                    let dx = pos.x - drag.x;
                    let dy = pos.y - drag.y;
                    state.drag_start = Some(pos);
                    Some(
                        canvas::Action::publish(CanvasEvent::Pan(dx, dy))
                            .and(canvas::Action::request_redraw()),
                    )
                } else {
                    Some(canvas::Action::request_redraw())
                }
            }
            // Scroll wheel → zoom around cursor
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let factor = match delta {
                    mouse::ScrollDelta::Lines  { y, .. } => 1.0 + y * 0.1,
                    mouse::ScrollDelta::Pixels { y, .. } => 1.0 + y * 0.001,
                };
                Some(canvas::Action::publish(CanvasEvent::ZoomAt(factor, pos)))
            }
            // Left-click → item selection
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let sch_pt = self.viewport.to_schematic(pos);
                if let Some(id) = hit_test(self.items, sch_pt) {
                    Some(
                        canvas::Action::publish(CanvasEvent::ItemClicked(id))
                            .and_capture(),
                    )
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &InteractionState,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        // Cached main content
        let content = self.cache.draw(renderer, bounds.size(), |frame| {
            // Background
            frame.fill_rectangle(
                Point::ORIGIN,
                bounds.size(),
                Color::from_rgb(0.08, 0.08, 0.12),
            );
            // Items
            for item in self.items {
                if self.viewport.visible_bounds(bounds.size()).intersects(&item.bounds()) {
                    draw_item(frame, item, self.viewport);
                }
            }
        });

        // Crosshair — not cached, drawn every frame
        let mut overlay = Frame::new(renderer, bounds.size());
        if let Some(pos) = cursor.position_in(bounds) {
            let lines = Path::new(|b| {
                b.move_to(Point::new(0.0, pos.y));
                b.line_to(Point::new(bounds.width, pos.y));
                b.move_to(Point::new(pos.x, 0.0));
                b.line_to(Point::new(pos.x, bounds.height));
            });
            overlay.stroke(
                &lines,
                Stroke::default()
                    .with_color(Color::from_rgba(1.0, 1.0, 1.0, 0.15))
                    .with_width(0.5),
            );
        }

        vec![content, overlay.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &InteractionState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.drag_start.is_some() {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(bounds) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}
```

---

## canvas::Action API

```rust
// Trigger a redraw only:
canvas::Action::request_redraw()

// Publish a message to the app:
canvas::Action::publish(MyMsg::Clicked)

// Publish + redraw:
canvas::Action::publish(msg).and(canvas::Action::request_redraw())

// Capture event so it does not propagate to other widgets:
canvas::Action::publish(msg).and_capture()

// Capture without publishing:
canvas::Action::capture()
```

---

## Frame drawing API

```rust
use iced::widget::canvas::{Frame, Path, Stroke, Fill, LineCap, LineJoin};
use iced::{Color, Point, Size, Vector};

fn draw_something(frame: &mut Frame) {
    // Build a path
    let path = Path::new(|b| {
        b.move_to(Point::new(10.0, 10.0));
        b.line_to(Point::new(100.0, 10.0));
        b.close();

        b.circle(Point::new(50.0, 50.0), 20.0);

        b.move_to(Point::new(0.0, 0.0));
        b.quadratic_curve_to(Point::new(50.0, -30.0), Point::new(100.0, 0.0));

        b.bezier_curve_to(
            Point::new(20.0, 80.0),
            Point::new(80.0, 80.0),
            Point::new(100.0, 0.0),
        );

        b.rectangle(Point::new(10.0, 10.0), Size::new(80.0, 60.0));
    });

    // Stroke
    frame.stroke(
        &path,
        Stroke {
            style: canvas::stroke::Style::Solid(Color::from_rgb(0.0, 0.8, 0.0)),
            width: 1.5,
            line_cap:  LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        },
    );

    // Fill
    frame.fill(&path, Fill::from(Color::from_rgba(0.0, 0.8, 0.0, 0.3)));

    // Quick filled rectangle
    frame.fill_rectangle(Point::new(20.0, 20.0), Size::new(60.0, 40.0), Color::RED);

    // Text
    frame.fill_text(canvas::Text {
        content:              "R1".to_string(),
        position:             Point::new(55.0, 55.0),
        color:                Color::WHITE,
        size:                 14.0.into(),
        font:                 iced::Font::MONOSPACE,
        horizontal_alignment: iced::alignment::Horizontal::Center,
        vertical_alignment:   iced::alignment::Vertical::Center,
        ..canvas::Text::default()
    });

    // Scoped transform
    frame.with_save(|f| {
        f.translate(Vector::new(100.0, 100.0));
        f.rotate(std::f32::consts::FRAC_PI_4);
        f.fill_rectangle(Point::ORIGIN, Size::new(50.0, 10.0), Color::RED);
    });
}
```

---

## Viewport — nanometer-aware pan/zoom

Internal coordinates use **nanometers** (`i64`). The viewport converts to canvas pixels
only at the render boundary.

```rust
/// Viewport converts between schematic nanometers and canvas pixels.
#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    /// Pixels per nanometer (zoom level).
    pub scale:    f64,
    /// Canvas-pixel offset of the schematic origin.
    pub offset_x: f64,
    pub offset_y: f64,
}

impl Viewport {
    pub fn new(scale: f64) -> Self {
        Self { scale, offset_x: 0.0, offset_y: 0.0 }
    }

    /// Fit the viewport to a bounding box in nanometers.
    pub fn fit(bbox_nm: BoundingBox, canvas_w: f32, canvas_h: f32) -> Self {
        let w_nm = bbox_nm.width()  as f64;
        let h_nm = bbox_nm.height() as f64;
        let scale = (canvas_w as f64 / w_nm)
            .min(canvas_h as f64 / h_nm) * 0.9;
        let offset_x = canvas_w as f64 / 2.0 - (bbox_nm.cx() as f64) * scale;
        let offset_y = canvas_h as f64 / 2.0 + (bbox_nm.cy() as f64) * scale; // Y flip
        Self { scale, offset_x, offset_y }
    }

    /// Schematic nanometers → canvas pixels (Y axis flipped).
    #[inline]
    pub fn to_canvas(&self, x_nm: i64, y_nm: i64) -> Point {
        Point::new(
             x_nm as f64 * self.scale + self.offset_x,
            -y_nm as f64 * self.scale + self.offset_y,
        ) as _
    }

    /// Canvas pixels → schematic nanometers.
    #[inline]
    pub fn to_schematic(&self, canvas: Point) -> (i64, i64) {
        let x = ((canvas.x as f64 - self.offset_x) / self.scale) as i64;
        let y = -((canvas.y as f64 - self.offset_y) / self.scale) as i64;
        (x, y)
    }

    /// Zoom around a canvas pivot point.
    pub fn zoom_at(&mut self, factor: f64, pivot: Point) {
        let px = pivot.x as f64;
        let py = pivot.y as f64;
        self.offset_x = px + (self.offset_x - px) * factor;
        self.offset_y = py + (self.offset_y - py) * factor;
        self.scale = (self.scale * factor).clamp(1e-9, 1e3); // nm limits
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.offset_x += dx as f64;
        self.offset_y += dy as f64;
    }

    /// Compute the visible bounding box in schematic nanometers.
    pub fn visible_bounds(&self, canvas_size: iced::Size) -> BoundingBox {
        let (x0, y0) = self.to_schematic(Point::ORIGIN);
        let (x1, y1) = self.to_schematic(Point::new(canvas_size.width, canvas_size.height));
        BoundingBox::from_corners(x0, y0, x1, y1)
    }

    /// Width of one canvas pixel in nanometers (useful for grid snapping).
    pub fn nm_per_pixel(&self) -> f64 {
        1.0 / self.scale
    }
}
```

---

## Hit testing

```rust
const HIT_RADIUS_PX: f32 = 5.0; // pixels

fn hit_test(items: &[SchItem], viewport: &Viewport, canvas_pos: Point) -> Option<ItemId> {
    let hit_nm = (HIT_RADIUS_PX as f64 / viewport.scale) as i64;

    // Iterate in reverse so top-most item wins
    for item in items.iter().rev() {
        match &item.kind {
            ItemKind::Wire { start, end } => {
                if distance_to_segment_nm(
                    canvas_pos, viewport, *start, *end
                ) < hit_nm {
                    return Some(item.id);
                }
            }
            ItemKind::Symbol { bounds } => {
                let c = viewport.to_canvas(bounds.cx(), bounds.cy());
                let w = bounds.width() as f64 * viewport.scale;
                let h = bounds.height() as f64 * viewport.scale;
                let rect = iced::Rectangle::new(
                    Point::new((c.x - w as f32 / 2.0), (c.y - h as f32 / 2.0)),
                    iced::Size::new(w as f32, h as f32),
                );
                if rect.contains(canvas_pos) {
                    return Some(item.id);
                }
            }
            _ => {}
        }
    }
    None
}

fn distance_to_segment_nm(
    canvas_pos: Point,
    vp: &Viewport,
    a_nm: (i64, i64),
    b_nm: (i64, i64),
) -> i64 {
    let a = vp.to_canvas(a_nm.0, a_nm.1);
    let b = vp.to_canvas(b_nm.0, b_nm.1);
    let ab = iced::Vector::new(b.x - a.x, b.y - a.y);
    let ap = iced::Vector::new(canvas_pos.x - a.x, canvas_pos.y - a.y);
    let len2 = ab.x * ab.x + ab.y * ab.y;
    let t = if len2 < 1e-10 {
        0.0_f32
    } else {
        ((ap.x * ab.x + ap.y * ab.y) / len2).clamp(0.0, 1.0)
    };
    let closest = Point::new(a.x + t * ab.x, a.y + t * ab.y);
    let d = iced::Vector::new(canvas_pos.x - closest.x, canvas_pos.y - closest.y);
    ((d.x * d.x + d.y * d.y).sqrt() / vp.scale as f32) as i64
}
```
