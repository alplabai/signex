//! Filled closed loops — walks the line/arc graph, finds simple closed
//! cycles, and fills each polygon with a role-tinted plate under its role
//! tag (`role_tags`). Also
//! exposes the `ClosedLoop` records so the click handler can select an
//! entire loop from a single fill click.

use iced::Color;
use iced::widget::canvas::{self, Path};

use super::role_tags::{draw_loop_tag, loop_role, loop_tag, role_colour};
use crate::library::editor::footprint::canvas::FootprintCanvasState;
use crate::library::editor::footprint::state::FootprintEditorState;

/// The plate of a selected loop, of an idle loop without a role, and the
/// alpha a role's colour fills a loop at.
const SELECTED_PLATE: Color = Color::from_rgba(0.30, 0.55, 0.92, 0.45);
const IDLE_PLATE: Color = Color::from_rgba(0.55, 0.75, 0.98, 0.18);
const ROLE_ALPHA: f32 = 0.20;

/// v0.16.1 — Walk the sketch's line graph, find simple closed
/// cycles, and render each as a filled polygon. Skips cycles where
/// every Line is `construction = true` (those are pad-corner
/// outlines or user-authored guides — already rendered as dashed
/// strokes elsewhere; double-filling would obscure the rendered
/// pad). Arc-bounded loops are deferred to v0.16.2.
///
/// v0.16.2 — Looks up the role attr on every entity in the loop.
/// The first hit picks the fill colour (`role_tags::role_colour`). Loops
/// with no role assignment fall back to neutral grey.
/// v0.27 — closed-loop record exposed to the click handler so a
/// single click on the polygon fill can select every entity in the
/// loop; `draw_filled_closed_loops` fills and tags the same records.
pub(in crate::library::editor::footprint::canvas) struct ClosedLoop {
    pub lines: Vec<signex_sketch::id::SketchEntityId>,
    pub points: Vec<signex_sketch::id::SketchEntityId>,
    /// Vertex array shaped as `[[x, y]; n]` for direct hand-off to
    /// `super::super::geometry::point_in_polygon`.
    pub polygon: Vec<[f64; 2]>,
}

/// v0.27 — find every closed loop in the sketch. Same adjacency
/// walk as the fill renderer; centralised so the click handler can
/// reuse it. Skips loops where every line is bake-skipped (purely
/// construction loops); those are visible only as dashed strokes
/// and selecting them via fill would surprise the user.
pub(in crate::library::editor::footprint::canvas) fn find_closed_loops(
    sketch: &signex_sketch::SketchData,
    state: &FootprintEditorState,
) -> Vec<ClosedLoop> {
    use signex_sketch::entity::EntityKind;
    use signex_sketch::id::SketchEntityId;
    use std::collections::{HashMap, HashSet};

    fn pos(
        id: SketchEntityId,
        sketch: &signex_sketch::SketchData,
        state: &FootprintEditorState,
    ) -> Option<(f64, f64)> {
        if let Some(solve) = state.last_solve.as_ref()
            && let Some(p) = signex_sketch::solver::state::point_xy(
                id,
                &solve.result.state,
                &solve.result.index,
                sketch,
            )
        {
            return Some(p);
        }
        sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
    }

    // v0.27 — adjacency now includes Arc edges as well as Lines so
    // the rounded-rectangle (line + arc + line + arc + ...) walks
    // as a single closed loop. The fill renderer was missing
    // rounded shapes for the same reason; extending the walker
    // here keeps both in sync.
    let mut adj: HashMap<SketchEntityId, Vec<(SketchEntityId, SketchEntityId, bool)>> =
        HashMap::new();
    for e in &sketch.entities {
        match e.kind {
            EntityKind::Line { start, end } => {
                adj.entry(start)
                    .or_default()
                    .push((end, e.id, e.bake_skipped()));
                adj.entry(end)
                    .or_default()
                    .push((start, e.id, e.bake_skipped()));
            }
            EntityKind::Arc { start, end, .. } => {
                adj.entry(start)
                    .or_default()
                    .push((end, e.id, e.bake_skipped()));
                adj.entry(end)
                    .or_default()
                    .push((start, e.id, e.bake_skipped()));
            }
            _ => {}
        }
    }
    let mut visited: HashSet<SketchEntityId> = HashSet::new();
    let mut out: Vec<ClosedLoop> = Vec::new();
    for seed in &sketch.entities {
        let (s_a, s_b) = match seed.kind {
            EntityKind::Line { start, end } => (start, end),
            EntityKind::Arc { start, end, .. } => (start, end),
            _ => continue,
        };
        if visited.contains(&seed.id) {
            continue;
        }
        let mut points = vec![s_a];
        let mut lines = vec![seed.id];
        let mut all_skipped = seed.bake_skipped();
        let mut current = s_b;
        let mut prev_line = seed.id;
        let mut closed = false;
        for _ in 0..256 {
            if current == s_a {
                closed = true;
                break;
            }
            let neigh = match adj.get(&current) {
                Some(n) if n.len() == 2 => n,
                _ => break,
            };
            match neigh.iter().find(|(_, lid, _)| *lid != prev_line) {
                Some((other, lid, skipped)) => {
                    points.push(current);
                    lines.push(*lid);
                    all_skipped &= *skipped;
                    prev_line = *lid;
                    current = *other;
                }
                None => break,
            }
        }
        if !closed || points.len() < 3 || all_skipped {
            visited.insert(seed.id);
            continue;
        }
        for lid in &lines {
            visited.insert(*lid);
        }
        let polygon: Vec<[f64; 2]> = points
            .iter()
            .filter_map(|id| pos(*id, sketch, state).map(|(x, y)| [x, y]))
            .collect();
        if polygon.len() < 3 {
            continue;
        }
        out.push(ClosedLoop {
            lines,
            points,
            polygon,
        });
    }
    out
}

pub(super) fn draw_filled_closed_loops(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &signex_sketch::SketchData,
    state: &FootprintEditorState,
) {
    use signex_sketch::id::SketchEntityId;
    use std::collections::HashSet;

    // A loop with any line or point selected draws the selected plate.
    let selected: HashSet<SketchEntityId> = state
        .selected_sketch
        .into_iter()
        .chain(state.selected_sketch_secondary)
        .chain(state.selected_sketch_extra.iter().copied())
        .collect();
    let loops = find_closed_loops(sketch, state);
    for closed in &loops {
        let loop_selected = closed
            .lines
            .iter()
            .chain(closed.points.iter())
            .any(|id| selected.contains(id));
        // Selection wins; a role tints the loop in its layer's colour (the
        // courtyard in its own), else the idle pale plate.
        let fill = if loop_selected {
            SELECTED_PLATE
        } else {
            match role_colour(loop_role(sketch, closed)) {
                Some(c) => Color { a: ROLE_ALPHA, ..c },
                None => IDLE_PLATE,
            }
        };
        let path = Path::new(|builder| {
            let p0 = cstate.world_to_screen((closed.polygon[0][0], closed.polygon[0][1]));
            builder.move_to(p0);
            for pos in closed.polygon.iter().skip(1) {
                builder.line_to(cstate.world_to_screen((pos[0], pos[1])));
            }
            builder.close();
        });
        frame.fill(&path, fill);
    }
    // The role tags draw over every fill, selected or not (mockup row 190).
    for closed in &loops {
        if let Some(tag) = loop_tag(sketch, closed, state, cstate) {
            draw_loop_tag(frame, &tag);
        }
    }
}
