//! Footprint editor 2D canvas — pure CPU rendering via
//! `iced::widget::Canvas`. Pads are drawn as axis-aligned rectangles
//! coloured by their primary layer; courtyard renders as a yellow
//! outline; graphics (silk/fab) trace through their stored layer
//! colour.
//!
//! Input model — middle/right-drag pans, scroll-wheel zooms (cursor
//! anchored), left-click on a pad selects it, left-drag moves the
//! selected pad, left-click on empty canvas adds a pad. Delete-key
//! handling lives in `library/editor/footprint/mod.rs`'s key event
//! since Canvas doesn't surface keyboard events.

use iced::event::Event;
use iced::mouse;
use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Theme};

use crate::library::messages::{EditorMsg, LibraryMessage};

use super::layers::FpLayer;
use super::state::{EditorPad, FootprintEditorState, GraphicKind};

/// Drag threshold in screen pixels — below this we treat the press
/// as a click, above this as a drag.
const DRAG_THRESHOLD_PX: f32 = 3.0;

/// Pixel-per-mm at the canvas's "100%" zoom — picked so a 5×5 mm
/// SOT-23 fits comfortably in a 600px-wide tab.
const DEFAULT_PX_PER_MM: f32 = 30.0;

const MIN_SCALE: f32 = 5.0;
const MAX_SCALE: f32 = 400.0;
const ZOOM_FACTOR: f32 = 1.15;

/// Canvas-only state owned by `iced::widget::Canvas`. The editor's
/// model lives in `FootprintEditorState`; this struct only holds
/// per-instance interaction state (camera, drag flags).
#[derive(Debug)]
pub struct FootprintCanvasState {
    /// World→screen affine: `screen = world * scale + offset`.
    /// `scale` is in pixels-per-mm.
    pub scale: f32,
    pub offset: Point,
    /// Auto-fit on the first draw — toggled false once we've seen
    /// non-zero bounds at least once.
    pub did_initial_fit: bool,
    panning: bool,
    last_pan_pos: Option<Point>,
    /// Drag state — `Some` while the user is mid-drag on a pad.
    drag: Option<DragState>,
    /// The pad index reported as `selected_pad` on the model the
    /// last time we drew. Used so the press handler can tell whether
    /// the click was on the already-selected pad.
    last_known_selected: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
struct DragState {
    pad_idx: usize,
    /// World-mm offset between the drag origin and the pad centre.
    /// Subtract from cursor position to get the pad's new centre.
    grab_offset_mm: (f64, f64),
    /// Screen-pixel position the press started at. Used to gate
    /// "did this drag actually move?".
    press_screen: Point,
    moved: bool,
}

impl Default for FootprintCanvasState {
    fn default() -> Self {
        Self {
            scale: DEFAULT_PX_PER_MM,
            offset: Point::new(0.0, 0.0),
            did_initial_fit: false,
            panning: false,
            last_pan_pos: None,
            drag: None,
            last_known_selected: None,
        }
    }
}

impl FootprintCanvasState {
    fn world_to_screen(&self, world: (f64, f64)) -> Point {
        Point::new(
            world.0 as f32 * self.scale + self.offset.x,
            world.1 as f32 * self.scale + self.offset.y,
        )
    }

    fn screen_to_world(&self, screen: Point) -> (f64, f64) {
        (
            ((screen.x - self.offset.x) / self.scale) as f64,
            ((screen.y - self.offset.y) / self.scale) as f64,
        )
    }

    fn fit_to_bounds(&mut self, world_bbox: (f64, f64, f64, f64), viewport: Rectangle) {
        let (min_x, min_y, max_x, max_y) = world_bbox;
        let w = (max_x - min_x).max(1e-3);
        let h = (max_y - min_y).max(1e-3);
        let pad = 12.0_f32;
        let avail_w = (viewport.width - pad * 2.0).max(1.0);
        let avail_h = (viewport.height - pad * 2.0).max(1.0);
        let scale_x = avail_w / w as f32;
        let scale_y = avail_h / h as f32;
        self.scale = scale_x.min(scale_y).clamp(MIN_SCALE, MAX_SCALE);
        let cx = ((min_x + max_x) / 2.0) as f32;
        let cy = ((min_y + max_y) / 2.0) as f32;
        self.offset = Point::new(
            viewport.width / 2.0 - cx * self.scale,
            viewport.height / 2.0 - cy * self.scale,
        );
    }
}

/// The Canvas program. Holds a snapshot of the model — `view()`
/// rebuilds this every frame, so we only need a borrowed reference.
pub struct FootprintCanvas<'a> {
    pub state: &'a FootprintEditorState,
    pub window_id: iced::window::Id,
    pub bg_color: Color,
    pub grid_color: Color,
    /// Pre-allocated empty cache so `draw` can reuse iced's caching
    /// layer if profiling later motivates it.
    pub cache: &'a canvas::Cache,
}

impl<'a> canvas::Program<LibraryMessage> for FootprintCanvas<'a> {
    type State = FootprintCanvasState;

    fn update(
        &self,
        cstate: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<LibraryMessage>> {
        // First-draw fit. Skip for empty footprints — we'd just zoom
        // the origin pixel.
        if !cstate.did_initial_fit
            && bounds.width > 0.0
            && bounds.height > 0.0
            && let Some(bbox) = self.state.content_bbox_mm()
        {
            cstate.fit_to_bounds(bbox, bounds);
            cstate.did_initial_fit = true;
        }

        cstate.last_known_selected = self.state.selected_pad;

        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let scroll_y = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y / 50.0,
                };
                let Some(cursor_pos) = cursor.position_in(bounds) else {
                    return None;
                };
                if scroll_y == 0.0 {
                    return None;
                }
                let factor = if scroll_y > 0.0 {
                    ZOOM_FACTOR
                } else {
                    1.0 / ZOOM_FACTOR
                };
                let new_scale = (cstate.scale * factor).clamp(MIN_SCALE, MAX_SCALE);
                let actual_factor = new_scale / cstate.scale;
                cstate.offset.x =
                    cursor_pos.x - (cursor_pos.x - cstate.offset.x) * actual_factor;
                cstate.offset.y =
                    cursor_pos.y - (cursor_pos.y - cstate.offset.y) * actual_factor;
                cstate.scale = new_scale;
                self.cache.clear();
                return Some(canvas::Action::capture());
            }
            Event::Mouse(mouse::Event::ButtonPressed(button)) => {
                if matches!(button, mouse::Button::Right | mouse::Button::Middle) {
                    cstate.panning = true;
                    cstate.last_pan_pos = cursor.position_in(bounds);
                    return Some(canvas::Action::capture());
                }
                if *button == mouse::Button::Left
                    && let Some(cursor_pos) = cursor.position_in(bounds)
                {
                    let world = cstate.screen_to_world(cursor_pos);
                    if let Some(pad_idx) = self.state.pad_at(world.0, world.1) {
                        let pad = &self.state.pads[pad_idx];
                        cstate.drag = Some(DragState {
                            pad_idx,
                            grab_offset_mm: (
                                world.0 - pad.position_mm.0,
                                world.1 - pad.position_mm.1,
                            ),
                            press_screen: cursor_pos,
                            moved: false,
                        });
                        // Emit a select message so the model
                        // highlights the pad on press.
                        return Some(
                            canvas::Action::publish(LibraryMessage::EditorEvent {
                                window_id: self.window_id,
                                msg: EditorMsg::FootprintSelectPad(Some(pad_idx)),
                            })
                            .and_capture(),
                        );
                    }
                    // Empty area → pending click-add. We can't add yet
                    // because a drag may follow; commit on release.
                    cstate.drag = Some(DragState {
                        pad_idx: usize::MAX,
                        grab_offset_mm: (world.0, world.1),
                        press_screen: cursor_pos,
                        moved: false,
                    });
                    return Some(canvas::Action::capture());
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(button)) => {
                if matches!(button, mouse::Button::Right | mouse::Button::Middle) {
                    cstate.panning = false;
                    cstate.last_pan_pos = None;
                    return None;
                }
                if *button == mouse::Button::Left
                    && let Some(drag) = cstate.drag.take()
                {
                    if drag.pad_idx == usize::MAX {
                        if drag.moved {
                            // Cancelled click-add — drag in empty
                            // space without crossing a pad doesn't
                            // place anything.
                            return None;
                        }
                        // Click-add at the press position (world coords
                        // were stashed in grab_offset_mm).
                        return Some(canvas::Action::publish(
                            LibraryMessage::EditorEvent {
                                window_id: self.window_id,
                                msg: EditorMsg::FootprintAddPad {
                                    x_mm: drag.grab_offset_mm.0,
                                    y_mm: drag.grab_offset_mm.1,
                                },
                            },
                        ));
                    }
                    if drag.moved {
                        // Final move position is already published per
                        // CursorMoved tick — nothing to do on release.
                        self.cache.clear();
                    }
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let Some(cursor_pos) = cursor.position_in(bounds) else {
                    return None;
                };
                if cstate.panning
                    && let Some(last) = cstate.last_pan_pos
                {
                    cstate.offset.x += cursor_pos.x - last.x;
                    cstate.offset.y += cursor_pos.y - last.y;
                    cstate.last_pan_pos = Some(cursor_pos);
                    self.cache.clear();
                    return Some(canvas::Action::capture());
                }
                let world = cstate.screen_to_world(cursor_pos);
                if let Some(drag) = cstate.drag.as_mut() {
                    let dx = (cursor_pos.x - drag.press_screen.x).abs();
                    let dy = (cursor_pos.y - drag.press_screen.y).abs();
                    if !drag.moved && dx.max(dy) >= DRAG_THRESHOLD_PX {
                        drag.moved = true;
                    }
                    if drag.moved && drag.pad_idx != usize::MAX {
                        let new_x = world.0 - drag.grab_offset_mm.0;
                        let new_y = world.1 - drag.grab_offset_mm.1;
                        return Some(canvas::Action::publish(
                            LibraryMessage::EditorEvent {
                                window_id: self.window_id,
                                msg: EditorMsg::FootprintMovePad {
                                    idx: drag.pad_idx,
                                    x_mm: new_x,
                                    y_mm: new_y,
                                },
                            },
                        ));
                    }
                }
                // Background hover — push the cursor position so the
                // footer readout updates.
                return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                    window_id: self.window_id,
                    msg: EditorMsg::FootprintCursorAt {
                        x_mm: world.0,
                        y_mm: world.1,
                    },
                }));
            }
            _ => {}
        }
        None
    }

    fn draw(
        &self,
        cstate: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let geom = self.cache.draw(renderer, bounds.size(), |frame| {
            // Background.
            frame.fill_rectangle(Point::ORIGIN, bounds.size(), self.bg_color);

            // 1mm minor grid + 5mm major. Scale makes minor grid
            // disappear when zoomed out.
            let minor_step = 1.0_f32 * cstate.scale;
            let major_step = 5.0_f32 * cstate.scale;
            if minor_step >= 6.0 {
                draw_grid(frame, bounds, cstate.offset, minor_step, Color {
                    a: 0.10,
                    ..self.grid_color
                });
            }
            if major_step >= 8.0 {
                draw_grid(frame, bounds, cstate.offset, major_step, Color {
                    a: 0.30,
                    ..self.grid_color
                });
            }

            // Origin crosshair — small + at world (0, 0).
            let origin = cstate.world_to_screen((0.0, 0.0));
            frame.stroke(
                &Path::line(
                    Point::new(origin.x - 6.0, origin.y),
                    Point::new(origin.x + 6.0, origin.y),
                ),
                Stroke::default()
                    .with_width(1.0)
                    .with_color(Color::from_rgba(1.0, 1.0, 1.0, 0.30)),
            );
            frame.stroke(
                &Path::line(
                    Point::new(origin.x, origin.y - 6.0),
                    Point::new(origin.x, origin.y + 6.0),
                ),
                Stroke::default()
                    .with_width(1.0)
                    .with_color(Color::from_rgba(1.0, 1.0, 1.0, 0.30)),
            );

            // Graphics (silk / fab / etc.) — render below pads.
            for g in &self.state.graphics {
                if !self.state.layer_visibility.get(g.layer) {
                    continue;
                }
                let color = g.layer.color();
                let stroke = Stroke::default()
                    .with_width((g.width as f32 * cstate.scale).max(1.0))
                    .with_color(Color { a: 0.85, ..color });
                match &g.kind {
                    GraphicKind::Line { start, end } => {
                        let s = cstate.world_to_screen(*start);
                        let e = cstate.world_to_screen(*end);
                        frame.stroke(&Path::line(s, e), stroke);
                    }
                    GraphicKind::Circle { center, radius } => {
                        let c = cstate.world_to_screen(*center);
                        let r = (*radius as f32 * cstate.scale).max(1.0);
                        frame.stroke(&Path::circle(c, r), stroke);
                    }
                    GraphicKind::Polygon { points } => {
                        if points.len() >= 2 {
                            let path = Path::new(|b| {
                                let p0 = cstate.world_to_screen(points[0]);
                                b.move_to(p0);
                                for pt in &points[1..] {
                                    b.line_to(cstate.world_to_screen(*pt));
                                }
                                b.close();
                            });
                            frame.stroke(&path, stroke);
                        }
                    }
                }
            }

            // Courtyard — drawn as a hollow rectangle on Edge.Cuts.
            if self.state.layer_visibility.get(FpLayer::EdgeCuts)
                && let Some(c) = self.state.courtyard_mm
            {
                let p0 = cstate.world_to_screen((c.min_x, c.min_y));
                let p1 = cstate.world_to_screen((c.max_x, c.max_y));
                let rect = Path::rectangle(
                    Point::new(p0.x, p0.y),
                    iced::Size::new(p1.x - p0.x, p1.y - p0.y),
                );
                frame.stroke(
                    &rect,
                    Stroke::default()
                        .with_width(1.5)
                        .with_color(FpLayer::EdgeCuts.color()),
                );
            }

            // Pads — render last so they sit on top.
            for (idx, pad) in self.state.pads.iter().enumerate() {
                if !self.state.layer_visibility.get(pad.primary_layer()) {
                    continue;
                }
                draw_pad(
                    frame,
                    cstate,
                    pad,
                    self.state.selected_pad == Some(idx),
                );
            }
        });

        vec![geom]
    }

    fn mouse_interaction(
        &self,
        cstate: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cstate.panning {
            return mouse::Interaction::Grabbing;
        }
        if cstate.drag.is_some() {
            return mouse::Interaction::Grab;
        }
        if cursor.is_over(bounds) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

fn draw_grid(
    frame: &mut canvas::Frame,
    bounds: Rectangle,
    offset: Point,
    step: f32,
    color: Color,
) {
    let stroke = Stroke::default().with_width(0.5).with_color(color);
    let mut x = offset.x.rem_euclid(step) - step;
    while x <= bounds.width + step {
        frame.stroke(
            &Path::line(Point::new(x, 0.0), Point::new(x, bounds.height)),
            stroke,
        );
        x += step;
    }
    let mut y = offset.y.rem_euclid(step) - step;
    while y <= bounds.height + step {
        frame.stroke(
            &Path::line(Point::new(0.0, y), Point::new(bounds.width, y)),
            stroke,
        );
        y += step;
    }
}

fn draw_pad(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    pad: &EditorPad,
    is_selected: bool,
) {
    let layer = pad.primary_layer();
    let color = layer.color();
    let (x0, y0, x1, y1) = pad.bbox_mm();
    let p0 = cstate.world_to_screen((x0, y0));
    let p1 = cstate.world_to_screen((x1, y1));
    let size = iced::Size::new(p1.x - p0.x, p1.y - p0.y);
    let rect = Path::rectangle(p0, size);
    frame.fill(&rect, Color { a: 0.85, ..color });
    let outline_color = if is_selected {
        Color::from_rgb(1.0, 1.0, 1.0)
    } else {
        Color { a: 1.0, ..color }
    };
    frame.stroke(
        &rect,
        Stroke::default()
            .with_width(if is_selected { 1.6 } else { 0.8 })
            .with_color(outline_color),
    );

    // Pad number — only when zoomed in enough to read.
    if cstate.scale >= 25.0 && !pad.number.is_empty() {
        let centre = cstate.world_to_screen(pad.position_mm);
        let text_size = (cstate.scale * 0.35).clamp(8.0, 16.0);
        frame.fill_text(canvas::Text {
            content: pad.number.clone(),
            position: Point::new(centre.x, centre.y - text_size / 2.0),
            size: text_size.into(),
            color: Color::from_rgb(0.05, 0.05, 0.05),
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Top,
            ..canvas::Text::default()
        });
    }
}
