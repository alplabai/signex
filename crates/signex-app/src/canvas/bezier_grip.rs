//! Handles of a selected Bezier on the sheet (mockup row 157): square handles
//! on the ends, round handles on the control points, dashed control lines.
//! A drag moves one handle (Shift mirrors the other control point through
//! the chord's middle) and the drop is one undo step through
//! `AnnotationEdit::BezierPoints`. A locked curve has no handles.

use super::*;
use signex_types::schematic::{
    Point as SheetPoint, SchBezier, SchDrawing, SchematicSheet, SelectedKind,
};

/// How near (screen px) a press must land to a handle to take it.
const HANDLE_HIT_PX: f32 = 6.0;
/// Half the side of a square handle and the radius of a round one, screen px.
const HANDLE_HALF_PX: f32 = 3.5;
/// The handle outline and the control lines, screen px.
const HANDLE_LINE_PX: f32 = 1.0;
/// The dash of a control line, screen px.
const CONTROL_DASH_PX: [f32; 2] = [3.0, 3.0];
/// The control lines, each from an end to its control point.
const CONTROL_LINES: [(usize, usize); 2] = [(0, 1), (3, 2)];

/// A handle drag in flight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BezierDrag {
    pub drawing: uuid::Uuid,
    /// The held point: 0 start, 1 C1, 2 C2, 3 end.
    pub point: usize,
    /// Where the press landed, world mm.
    origin: (f64, f64),
    /// The curve's points when the drag began.
    from: [SheetPoint; 4],
    /// The points as the drag has them now.
    pub points: [SheetPoint; 4],
}

impl BezierDrag {
    /// What the release does: one edit when a point moved, else nothing.
    pub(in crate::canvas) fn dropped(self) -> canvas::Action<Message> {
        if self.points == self.from {
            return canvas::Action::capture();
        }
        canvas::Action::publish(Message::CanvasEvent(CanvasEvent::BezierPointsDragged {
            drawing: self.drawing,
            points: self.points,
        }))
    }
}

/// `from` with point `point` moved by `delta`. With `mirrored`, a dragged
/// control point takes the other one to its reflection through the middle of
/// the chord; an end moves alone.
pub(in crate::canvas) fn dragged_points(
    from: [SheetPoint; 4],
    point: usize,
    delta: (f64, f64),
    mirrored: bool,
) -> [SheetPoint; 4] {
    let mut points = from;
    let moved = SheetPoint::new(from[point].x + delta.0, from[point].y + delta.1);
    points[point] = moved;
    if mirrored && matches!(point, 1 | 2) {
        points[3 - point] = SheetPoint::new(
            from[0].x + from[3].x - moved.x,
            from[0].y + from[3].y - moved.y,
        );
    }
    points
}

/// The unlocked Beziers among `selected`.
fn selected_beziers<'s>(
    sheet: &'s SchematicSheet,
    selected: &'s [signex_types::schematic::SelectedItem],
) -> impl Iterator<Item = &'s SchBezier> {
    selected
        .iter()
        .filter(|item| item.kind == SelectedKind::Drawing)
        .filter_map(|item| sheet.drawings.iter().find(|d| d.uuid() == item.uuid))
        .filter_map(|drawing| match drawing {
            SchDrawing::Bezier(b) if !b.locked => Some(b),
            _ => None,
        })
}

impl SchematicCanvas<'_> {
    fn sheet_to_screen(&self, p: SheetPoint, bounds: Rectangle) -> iced::Point {
        self.camera()
            .world_to_screen(iced::Point::new(p.x as f32, p.y as f32), bounds)
    }

    /// The drag a press at the screen point `at` starts, if it lands on a
    /// handle of a selected Bezier; the nearest handle wins.
    pub(in crate::canvas) fn bezier_handle_at(
        &self,
        at: iced::Point,
        bounds: Rectangle,
        world: (f64, f64),
    ) -> Option<BezierDrag> {
        let sheet = self.active_snapshot()?;
        let mut best: Option<(f32, BezierDrag)> = None;
        for bezier in selected_beziers(sheet, &self.selected) {
            let from = bezier.points();
            for (point, p) in from.iter().enumerate() {
                let screen = self.sheet_to_screen(*p, bounds);
                let distance = (screen.x - at.x).hypot(screen.y - at.y);
                if distance <= HANDLE_HIT_PX && best.is_none_or(|(d, _)| distance < d) {
                    let drag = BezierDrag {
                        drawing: bezier.uuid,
                        point,
                        origin: world,
                        from,
                        points: from,
                    };
                    best = Some((distance, drag));
                }
            }
        }
        best.map(|(_, drag)| drag)
    }

    /// Follow the cursor while a handle is held: the delta steps on the snap
    /// grid, so a handle on the grid stays on it.
    pub(in crate::canvas) fn update_bezier_drag(
        &self,
        state: &mut CanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let mirrored = state.shift_held;
        let drag = state.bezier_drag.as_mut()?;
        let at = cursor.position_in(bounds)?;
        let world = self.camera().screen_to_world(at, bounds);
        let mut delta = (
            f64::from(world.x) - drag.origin.0,
            f64::from(world.y) - drag.origin.1,
        );
        if self.prefs.snap_enabled && self.prefs.snap_grid_mm > 0.0 {
            let grid = self.prefs.snap_grid_mm;
            delta = (
                (delta.0 / grid).round() * grid,
                (delta.1 / grid).round() * grid,
            );
        }
        drag.points = dragged_points(drag.from, drag.point, delta, mirrored);
        let event = CanvasEvent::CursorAt {
            x: world.x,
            y: world.y,
            zoom_pct: self.camera().zoom_percent(),
        };
        Some(canvas::Action::publish(Message::CanvasEvent(event)).and_capture())
    }

    /// The handles of every selected, unlocked Bezier.
    pub(in crate::canvas) fn draw_bezier_handles(
        &self,
        frame: &mut canvas::Frame,
        snapshot: &SchematicSheet,
        bounds: Rectangle,
    ) {
        for bezier in selected_beziers(snapshot, &self.selected) {
            self.draw_handles(frame, bezier.points(), None, bounds);
        }
    }

    /// The curve and its handles as the drag in flight has them.
    pub(in crate::canvas) fn draw_bezier_drag(
        &self,
        frame: &mut canvas::Frame,
        state: &CanvasState,
        bounds: Rectangle,
    ) {
        let Some(drag) = state.bezier_drag else {
            return;
        };
        let Some(sheet) = self.active_snapshot() else {
            return;
        };
        let Some(mut bezier) = selected_beziers(sheet, &self.selected)
            .find(|b| b.uuid == drag.drawing)
            .cloned()
        else {
            return;
        };
        [bezier.start, bezier.c1, bezier.c2, bezier.end] = drag.points;
        let ink = crate::render_config::to_iced(&self.prefs.canvas_colors.selection);
        let curve = canvas::Path::new(|path| {
            for (i, p) in bezier.flatten().into_iter().enumerate() {
                let at = self.sheet_to_screen(p, bounds);
                if i == 0 {
                    path.move_to(at);
                } else {
                    path.line_to(at);
                }
            }
        });
        let stroke = canvas::Stroke::default()
            .with_color(ink)
            .with_width(HANDLE_LINE_PX);
        frame.stroke(&curve, stroke);
        self.draw_handles(frame, drag.points, Some(drag.point), bounds);
    }

    /// Dashed control lines, then square end and round control handles.
    /// Handles are hollow; the `held` one is filled.
    fn draw_handles(
        &self,
        frame: &mut canvas::Frame,
        points: [SheetPoint; 4],
        held: Option<usize>,
        bounds: Rectangle,
    ) {
        let ink = crate::render_config::to_iced(&self.prefs.canvas_colors.selection);
        let paper = crate::render_config::to_iced(&self.prefs.canvas_colors.paper);
        let screen = points.map(|p| self.sheet_to_screen(p, bounds));
        let outline = canvas::Stroke::default()
            .with_color(ink)
            .with_width(HANDLE_LINE_PX);
        let dashed = canvas::Stroke {
            line_dash: canvas::LineDash {
                segments: &CONTROL_DASH_PX,
                offset: 0,
            },
            ..outline
        };
        for (end, control) in CONTROL_LINES {
            frame.stroke(&canvas::Path::line(screen[end], screen[control]), dashed);
        }
        for (i, at) in screen.into_iter().enumerate() {
            let handle = if matches!(i, 1 | 2) {
                canvas::Path::circle(at, HANDLE_HALF_PX)
            } else {
                canvas::Path::rectangle(
                    iced::Point::new(at.x - HANDLE_HALF_PX, at.y - HANDLE_HALF_PX),
                    iced::Size::new(2.0 * HANDLE_HALF_PX, 2.0 * HANDLE_HALF_PX),
                )
            };
            frame.fill(&handle, if held == Some(i) { ink } else { paper });
            frame.stroke(&handle, outline);
        }
    }
}
