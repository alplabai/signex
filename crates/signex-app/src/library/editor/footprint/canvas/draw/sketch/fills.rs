//! Filled closed loops — walks the line/arc graph, finds simple closed
//! cycles, and fills each polygon with a role-tinted plate. Also
//! exposes the `ClosedLoop` records so the click handler can select an
//! entire loop from a single fill click.

use iced::Color;
use iced::widget::canvas::{self, Path};

use crate::library::editor::footprint::canvas::FootprintCanvasState;
use crate::library::editor::footprint::layers::FpLayer;
use crate::library::editor::footprint::state::FootprintEditorState;

/// v0.16.1 — Walk the sketch's line graph, find simple closed
/// cycles, and render each as a filled polygon. Skips cycles where
/// every Line is `construction = true` (those are pad-corner
/// outlines or user-authored guides — already rendered as dashed
/// strokes elsewhere; double-filling would obscure the rendered
/// pad). Arc-bounded loops are deferred to v0.16.2.
///
/// v0.16.2 — Looks up the role attr on every entity in the loop.
/// The first hit picks the fill colour from the matching layer in
/// [`super::super::super::layers::FpLayer`]. Loops with no role assignment fall
/// back to neutral grey.
/// v0.27 — closed-loop record exposed to the click handler so a
/// single click on the polygon fill can select every entity in the
/// loop. Mirrors what `draw_filled_closed_loops` walks internally.
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
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_types::layer::SignexLayer;
    use std::collections::{HashMap, HashSet};

    // v0.16.2 — pick a fill colour for a loop by inspecting each
    // entity's role attr. Returns `None` when no entity in the loop
    // carries a role; the caller falls back to neutral grey.
    fn role_color(entity: &Entity) -> Option<FpLayer> {
        if entity.pad.is_some() {
            return Some(FpLayer::FCu);
        }
        if let Some(s) = entity.silk.as_ref() {
            return Some(if matches!(s.layer, SignexLayer::TopSilk) {
                FpLayer::FSilks
            } else {
                FpLayer::BSilks
            });
        }
        if entity.courtyard.is_some() {
            return Some(FpLayer::EdgeCuts);
        }
        if let Some(m) = entity.mask_opening.as_ref() {
            return Some(if matches!(m.layer, SignexLayer::TopSolderMask) {
                FpLayer::FFab
            } else {
                FpLayer::BFab
            });
        }
        if let Some(m) = entity.mask_exclude.as_ref() {
            return Some(if matches!(m.layer, SignexLayer::TopSolderMask) {
                FpLayer::FFab
            } else {
                FpLayer::BFab
            });
        }
        if let Some(p) = entity.paste_aperture.as_ref() {
            return Some(if matches!(p.layer, SignexLayer::TopPaste) {
                FpLayer::FFab
            } else {
                FpLayer::BFab
            });
        }
        if let Some(p) = entity.pour.as_ref() {
            return Some(if matches!(p.layer, SignexLayer::TopCopper) {
                FpLayer::FCu
            } else {
                FpLayer::BCu
            });
        }
        if entity.keepout.is_some() {
            return Some(FpLayer::EdgeCuts);
        }
        if entity.board_cutout.is_some() {
            return Some(FpLayer::EdgeCuts);
        }
        None
    }

    fn point_pos(
        id: SketchEntityId,
        sketch: &signex_sketch::SketchData,
        state: &FootprintEditorState,
    ) -> Option<(f64, f64)> {
        if let Some(solve) = state.last_solve.as_ref() {
            if let Some(p) = signex_sketch::solver::state::point_xy(
                id,
                &solve.result.state,
                &solve.result.index,
                sketch,
            ) {
                return Some(p);
            }
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

    // Build adjacency: Point ID -> Vec<(other_endpoint, edge_id, construction)>.
    // v0.27 — Arcs are now full participants alongside Lines so the
    // rounded-rectangle (line + arc + line + arc + ...) reads as
    // one closed loop and the fill paints across the whole shape.
    let mut adj: HashMap<SketchEntityId, Vec<(SketchEntityId, SketchEntityId, bool)>> =
        HashMap::new();
    for e in &sketch.entities {
        match e.kind {
            EntityKind::Line { start, end } => {
                adj.entry(start)
                    .or_default()
                    // v0.22 Phase A5 — Treat centerline lines the same
                    // as construction for closed-loop fill detection: a
                    // loop made entirely of skipped lines must not paint
                    // a profile fill (Altium / Fusion convention).
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

    let mut visited_lines: HashSet<SketchEntityId> = HashSet::new();
    for seed in &sketch.entities {
        let (seed_start, seed_end) = match seed.kind {
            EntityKind::Line { start, end } => (start, end),
            EntityKind::Arc { start, end, .. } => (start, end),
            _ => continue,
        };
        if visited_lines.contains(&seed.id) {
            continue;
        }
        // Walk: start at seed_start, follow seed → seed_end → next →
        // ... until we return to seed_start or fail.
        let mut points: Vec<SketchEntityId> = vec![seed_start];
        let mut lines: Vec<SketchEntityId> = vec![seed.id];
        let mut all_construction = seed.bake_skipped();
        let mut current = seed_end;
        let mut prev_line = seed.id;
        let mut closed = false;
        for _ in 0..256 {
            if current == seed_start {
                closed = true;
                break;
            }
            let neighbors = match adj.get(&current) {
                Some(n) if n.len() == 2 => n,
                _ => break,
            };
            let next = neighbors.iter().find(|(_, lid, _)| *lid != prev_line);
            match next {
                Some((other, lid, construction)) => {
                    points.push(current);
                    lines.push(*lid);
                    all_construction &= *construction;
                    prev_line = *lid;
                    current = *other;
                }
                None => break,
            }
        }
        if !closed || points.len() < 3 || all_construction {
            // Mark seed line visited so we don't re-walk it; but
            // don't fill.
            visited_lines.insert(seed.id);
            continue;
        }
        for lid in &lines {
            visited_lines.insert(*lid);
        }
        // Resolve to world positions, drop loops with missing data.
        let positions: Vec<(f64, f64)> = points
            .iter()
            .filter_map(|id| point_pos(*id, sketch, state))
            .collect();
        if positions.len() < 3 {
            continue;
        }
        // v0.16.2 — find the first role attr in the loop's lines or
        // points; use its layer colour for the fill. Falls back to
        // neutral grey when nothing in the loop carries a role.
        let loop_role: Option<FpLayer> = lines
            .iter()
            .chain(points.iter())
            .filter_map(|id| sketch.entities.iter().find(|e| e.id == *id))
            .find_map(role_color);
        // v0.27 — Fusion-style two-tone fill. The fill is the
        // primary visual cue of "this is a closed shape" — Fusion
        // shows pale-blue when idle and saturated-blue when the
        // whole loop is selected. We mirror that here: detect a
        // loop-wide selection by checking whether any line / point
        // ID is in the user's selection set, then pick the
        // saturated-blue plate; otherwise stay pale.
        //
        // Role-tagged loops (Pad / Silk / Courtyard / etc.)
        // override the pale fill with the layer colour at higher
        // alpha so the user can still distinguish role assignments
        // at a glance. Selection still wins to make the active
        // shape pop.
        let mut selected_set: std::collections::HashSet<SketchEntityId> =
            std::collections::HashSet::new();
        if let Some(id) = state.selected_sketch {
            selected_set.insert(id);
        }
        if let Some(id) = state.selected_sketch_secondary {
            selected_set.insert(id);
        }
        for id in &state.selected_sketch_extra {
            selected_set.insert(*id);
        }
        let loop_selected = !selected_set.is_empty()
            && (lines.iter().any(|l| selected_set.contains(l))
                || points.iter().any(|p| selected_set.contains(p)));
        let fill = if loop_selected {
            // Fusion saturated-blue selected fill.
            Color::from_rgba(0.30, 0.55, 0.92, 0.45)
        } else {
            match loop_role {
                Some(layer) => {
                    let c = layer.color();
                    Color {
                        r: c.r,
                        g: c.g,
                        b: c.b,
                        a: 0.20,
                    }
                }
                None => {
                    // Fusion idle pale-blue.
                    Color::from_rgba(0.55, 0.75, 0.98, 0.18)
                }
            }
        };
        let path = Path::new(|builder| {
            let p0 = cstate.world_to_screen(positions[0]);
            builder.move_to(p0);
            for pos in positions.iter().skip(1) {
                let p = cstate.world_to_screen(*pos);
                builder.line_to(p);
            }
            builder.close();
        });
        frame.fill(&path, fill);
    }
}
