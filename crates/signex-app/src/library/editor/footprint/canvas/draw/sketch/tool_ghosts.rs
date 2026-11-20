//! Ghost previews of the Modify tools that work on existing geometry
//! (mockup row 187 / mockup row 191): the Line a Chamfer or Extend click would take, where an
//! extended Line stops, the mirror image of the cursor, and the instances
//! of a Linear Pattern; and the rubber band of the Slot and Polygon Create
//! tools, from the geometry they will place. Drawn like the drawing tools'
//! ghosts in `preview`: accent at low alpha, dashed.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point};
use signex_sketch::SketchData;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::geom::shapes;
use signex_sketch::geom::{Point2, extension_hit};
use signex_sketch::id::SketchEntityId;

use crate::library::editor::footprint::canvas::FootprintCanvasState;
use crate::library::editor::footprint::state::tool::{
    LINEAR_PATTERN_DEFAULT_COUNT, LINEAR_PATTERN_DEFAULT_SPACING_MM,
};
use crate::library::editor::footprint::state::{FootprintEditorState, SketchTool, ToolPending};

/// The ghost colour of `preview::draw_sketch_tool_preview`.
const GHOST: Color = Color {
    r: 0.40,
    g: 0.70,
    b: 1.00,
    a: 0.85,
};
const GHOST_PX: f32 = 1.5;
/// A picked or hovered Line is drawn this wide.
const PICKED_PX: f32 = 3.0;
const DASH_PX: f32 = 8.0;
const MARK_PX: f32 = 4.0;
/// How near the cursor a Line must be to count as hovered, in pixels.
const HOVER_PX: f32 = 6.0;

/// Positions of sketch Points: solved when a solve is in, else authored.
struct Positions<'a> {
    sketch: &'a SketchData,
    state: &'a FootprintEditorState,
}

impl Positions<'_> {
    fn of(&self, id: SketchEntityId) -> Option<(f64, f64)> {
        if let Some(solve) = self.state.last_solve.as_ref()
            && let Some(p) = signex_sketch::solver::state::point_xy(
                id,
                &solve.result.state,
                &solve.result.index,
                self.sketch,
            )
        {
            return Some(p);
        }
        self.sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
    }

    fn line(&self, id: SketchEntityId) -> Option<(SketchEntityId, SketchEntityId)> {
        self.sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| match e.kind {
                EntityKind::Line { start, end } => Some((start, end)),
                _ => None,
            })
    }

    /// The nearest Line `keep` accepts within `tol` mm of `at`.
    fn line_near(
        &self,
        at: (f64, f64),
        tol: f64,
        keep: impl Fn(&Entity) -> bool,
    ) -> Option<SketchEntityId> {
        let mut best: Option<(f64, SketchEntityId)> = None;
        for e in &self.sketch.entities {
            let EntityKind::Line { start, end } = e.kind else {
                continue;
            };
            if !keep(e) {
                continue;
            }
            let (Some(a), Some(b)) = (self.of(start), self.of(end)) else {
                continue;
            };
            let d = signex_sketch::geom::point_to_segment_distance(at, a, b);
            if d <= tol && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, e.id));
            }
        }
        best.map(|(_, id)| id)
    }
}

pub(in crate::library::editor::footprint::canvas::draw) fn draw_sketch_tool_ghosts(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &SketchData,
    state: &FootprintEditorState,
) {
    let Some(cursor) = state.cursor_mm else {
        return;
    };
    if state.placement_paused {
        return;
    }
    let at = Positions { sketch, state };
    let tol = f64::from(HOVER_PX / cstate.scale.max(1.0));
    match state.active_tool {
        SketchTool::Chamfer => chamfer_ghost(frame, cstate, &at, state, cursor, tol),
        SketchTool::Extend => extend_ghost(frame, cstate, &at, cursor, tol),
        SketchTool::Mirror => mirror_ghost(frame, cstate, &at, state, cursor, tol),
        SketchTool::LinearPattern => linear_pattern_ghost(frame, cstate, state, cursor),
        SketchTool::Slot => slot_ghost(frame, cstate, &at, state, cursor),
        SketchTool::Polygon => polygon_ghost(frame, cstate, &at, state, cursor),
        _ => {}
    }
}

/// Slot: the centre line from the first centre to the cursor, then the
/// outline the width click would place.
fn slot_ghost(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    at: &Positions<'_>,
    state: &FootprintEditorState,
    cursor: (f64, f64),
) {
    match state.tool_pending {
        ToolPending::SlotFirst { a } => {
            if let Some(a) = at.of(a) {
                dashed(
                    frame,
                    cstate.world_to_screen(a),
                    cstate.world_to_screen(cursor),
                );
            }
        }
        ToolPending::SlotSecond { a, b } => {
            if let (Some(a), Some(b)) = (at.of(a), at.of(b)) {
                dashed(frame, cstate.world_to_screen(a), cstate.world_to_screen(b));
                let half = shapes::slot_half_width(a, b, cursor);
                closed_outline(frame, cstate, &shapes::slot_outline(a, b, half));
            }
        }
        _ => {}
    }
}

/// Polygon: the polygon a first-vertex click at the cursor would place,
/// with the typed side count (else the default).
fn polygon_ghost(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    at: &Positions<'_>,
    state: &FootprintEditorState,
    cursor: (f64, f64),
) {
    let ToolPending::PolygonCentre { centre } = state.tool_pending else {
        return;
    };
    let Some(centre) = at.of(centre) else {
        return;
    };
    let sides =
        shapes::parse_side_count(&state.dimension_input).unwrap_or(shapes::POLYGON_SIDES_DEFAULT);
    closed_outline(
        frame,
        cstate,
        &shapes::polygon_vertices(centre, cursor, sides),
    );
}

fn closed_outline(frame: &mut canvas::Frame, cstate: &FootprintCanvasState, at: &[(f64, f64)]) {
    let Some((&first, rest)) = at.split_first() else {
        return;
    };
    let path = Path::new(|b| {
        b.move_to(cstate.world_to_screen(first));
        for &p in rest {
            b.line_to(cstate.world_to_screen(p));
        }
        b.close();
    });
    frame.stroke(&path, stroke(GHOST_PX));
}

fn stroke(width: f32) -> Stroke<'static> {
    Stroke::default().with_width(width).with_color(GHOST)
}

fn dashed(frame: &mut canvas::Frame, p0: Point, p1: Point) {
    let (dx, dy) = (p1.x - p0.x, p1.y - p0.y);
    let len = (dx * dx + dy * dy).sqrt();
    if len <= 0.5 {
        return;
    }
    let n = ((len / DASH_PX).ceil() as i32).max(2);
    for i in (0..n).step_by(2) {
        let t0 = i as f32 / n as f32;
        let t1 = ((i + 1) as f32 / n as f32).min(1.0);
        frame.stroke(
            &Path::line(
                Point::new(p0.x + dx * t0, p0.y + dy * t0),
                Point::new(p0.x + dx * t1, p0.y + dy * t1),
            ),
            stroke(GHOST_PX),
        );
    }
}

fn stroke_line(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    at: &Positions<'_>,
    line: SketchEntityId,
    width: f32,
) {
    if let Some((s, e)) = at.line(line)
        && let (Some(a), Some(b)) = (at.of(s), at.of(e))
    {
        frame.stroke(
            &Path::line(cstate.world_to_screen(a), cstate.world_to_screen(b)),
            stroke(width),
        );
    }
}

/// Chamfer: the first Line once picked, and the Line under the cursor the
/// next click would take.
fn chamfer_ghost(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    at: &Positions<'_>,
    state: &FootprintEditorState,
    cursor: (f64, f64),
    tol: f64,
) {
    if let ToolPending::ChamferFirst { line } = state.tool_pending {
        stroke_line(frame, cstate, at, line, PICKED_PX);
    }
    if let Some(hovered) = at.line_near(cursor, tol, |_| true) {
        stroke_line(frame, cstate, at, hovered, PICKED_PX);
    }
}

/// Extend: from the hovered Line's nearer end to where it would stop.
fn extend_ghost(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    at: &Positions<'_>,
    cursor: (f64, f64),
    tol: f64,
) {
    let Some(line) = at.line_near(cursor, tol, |_| true) else {
        return;
    };
    let Some((s, e)) = at.line(line) else {
        return;
    };
    let (Some(a), Some(b)) = (at.of(s), at.of(e)) else {
        return;
    };
    let near = |p: (f64, f64)| (p.0 - cursor.0).powi(2) + (p.1 - cursor.1).powi(2);
    let (fixed, moving) = if near(a) < near(b) { (b, a) } else { (a, b) };
    let point = |id| at.of(id).map(Point2::from);
    if let Some(hit) = extension_hit(at.sketch, line, fixed.into(), moving.into(), point) {
        let to = cstate.world_to_screen(hit.at.into());
        dashed(frame, cstate.world_to_screen(moving), to);
        frame.stroke(&Path::circle(to, MARK_PX), stroke(GHOST_PX));
    }
}

/// Mirror: with a mirror Line selected, the cursor's mirror image; else the
/// centerline under the cursor the first click would take.
fn mirror_ghost(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    at: &Positions<'_>,
    state: &FootprintEditorState,
    cursor: (f64, f64),
    tol: f64,
) {
    let mirror_line = state
        .selected_sketch
        .and_then(|id| at.line(id))
        .and_then(|(s, e)| Some((at.of(s)?, at.of(e)?)));
    let Some((a, b)) = mirror_line else {
        if let Some(centerline) = at.line_near(cursor, tol, |e| e.centerline) {
            stroke_line(frame, cstate, at, centerline, PICKED_PX);
        }
        return;
    };
    let (vx, vy) = (b.0 - a.0, b.1 - a.1);
    let vv = vx * vx + vy * vy;
    if vv <= 1e-12 {
        return;
    }
    let t = ((cursor.0 - a.0) * vx + (cursor.1 - a.1) * vy) / vv;
    let foot = (a.0 + t * vx, a.1 + t * vy);
    let image = (2.0 * foot.0 - cursor.0, 2.0 * foot.1 - cursor.1);
    let p = cstate.world_to_screen(image);
    dashed(frame, cstate.world_to_screen(cursor), p);
    frame.stroke(
        &Path::line(
            Point::new(p.x - MARK_PX, p.y - MARK_PX),
            Point::new(p.x + MARK_PX, p.y + MARK_PX),
        ),
        stroke(GHOST_PX),
    );
    frame.stroke(
        &Path::line(
            Point::new(p.x - MARK_PX, p.y + MARK_PX),
            Point::new(p.x + MARK_PX, p.y - MARK_PX),
        ),
        stroke(GHOST_PX),
    );
}

/// Linear Pattern: a mark where each further instance of a source at the
/// cursor would land. The spacing is the typed dimension when it reads as
/// a positive length; the click refuses any other typed value.
fn linear_pattern_ghost(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    state: &FootprintEditorState,
    cursor: (f64, f64),
) {
    let spacing = signex_types::numeric::parse_finite_f64(state.dimension_input.trim())
        .ok()
        .filter(|mm| *mm > 0.0)
        .unwrap_or(LINEAR_PATTERN_DEFAULT_SPACING_MM);
    for i in 1..LINEAR_PATTERN_DEFAULT_COUNT {
        let p = cstate.world_to_screen((cursor.0 + spacing * f64::from(i), cursor.1));
        frame.stroke(&Path::circle(p, MARK_PX), stroke(GHOST_PX));
    }
}
