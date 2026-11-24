//! Filled closed loops — walks the line/arc graph, finds simple closed
//! cycles, and fills each polygon with a role-tinted plate under its role
//! tag (`role_tags`). Also
//! exposes the `ClosedLoop` records so the click handler can select an
//! entire loop from a single fill click.

use iced::Color;
use iced::widget::canvas::{self, Path};

use super::role_tags::{draw_loop_tag, loop_tag};
use crate::library::editor::footprint::canvas::FootprintCanvasState;
use crate::library::editor::footprint::layers::{FpLayer, LayerPalette};
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
        let fill = loop_fill(sketch, closed, loop_selected);
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

/// The plate a loop is filled with: the selected plate, else the colour of
/// the first of its edges, then corners, that has a fill colour, else the
/// idle pale plate.
fn loop_fill(sketch: &signex_sketch::SketchData, closed: &ClosedLoop, selected: bool) -> Color {
    if selected {
        return SELECTED_PLATE;
    }
    closed
        .lines
        .iter()
        .chain(closed.points.iter())
        .filter_map(|id| sketch.entities.iter().find(|e| e.id == *id))
        .find_map(fill_colour)
        .map_or(IDLE_PLATE, |c| Color { a: ROLE_ALPHA, ..c })
}

/// The fill colour of one entity's role attributes, in the fill's own
/// priority (pad, silk, courtyard, mask opening, mask exclude, paste, pour,
/// keepout, cutout); `None` when none of them is set, so the next entity of
/// the loop decides. Only courtyard has its own colour (mockup row 190); the
/// rest keep their layer's.
fn fill_colour(entity: &signex_sketch::entity::Entity) -> Option<Color> {
    use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
    let sided = |top: bool, front: FpLayer, back: FpLayer| if top { front } else { back };
    let layer = if entity.pad.is_some() {
        FpLayer::FCu
    } else if let Some(s) = entity.silk.as_ref() {
        let top = matches!(s.layer, PcbLayer::Technical(TechnicalLayer::SilkscreenTop));
        sided(top, FpLayer::FSilks, FpLayer::BSilks)
    } else if entity.courtyard.is_some() {
        return Some(LayerPalette::Editor.courtyard_colour());
    } else if let Some(m) = entity.mask_opening.as_ref() {
        let top = matches!(m.layer, PcbLayer::Technical(TechnicalLayer::SolderMaskTop));
        sided(top, FpLayer::FFab, FpLayer::BFab)
    } else if let Some(m) = entity.mask_exclude.as_ref() {
        let top = matches!(m.layer, PcbLayer::Technical(TechnicalLayer::SolderMaskTop));
        sided(top, FpLayer::FFab, FpLayer::BFab)
    } else if let Some(p) = entity.paste_aperture.as_ref() {
        let top = matches!(p.layer, PcbLayer::Technical(TechnicalLayer::PasteTop));
        sided(top, FpLayer::FFab, FpLayer::BFab)
    } else if let Some(p) = entity.pour.as_ref() {
        let top = matches!(p.layer, PcbLayer::Copper(CopperLayer::Top));
        sided(top, FpLayer::FCu, FpLayer::BCu)
    } else if entity.keepout.is_some() || entity.board_cutout.is_some() {
        FpLayer::EdgeCuts
    } else {
        return None;
    };
    Some(layer.color())
}

#[cfg(test)]
mod tests {
    use signex_library::Footprint;
    use signex_sketch::SketchData;
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_types::sketch::SketchRole;

    use super::*;
    use crate::library::editor::footprint::sketch_dispatch::set_entity_role;

    /// A 4 x 2 mm rectangle of four lines, and its lines.
    fn rectangle() -> (Footprint, Vec<SketchEntityId>) {
        let plane = PlaneId::new();
        let mut sketch = SketchData::default();
        sketch.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let corners = [(1.0, 1.0), (5.0, 1.0), (5.0, 3.0), (1.0, 3.0)].map(|(x, y)| {
            let id = SketchEntityId::new();
            sketch
                .entities
                .push(Entity::new(id, plane, EntityKind::Point { x, y }));
            id
        });
        let lines = (0..4)
            .map(|k| {
                let id = SketchEntityId::new();
                let kind = EntityKind::Line {
                    start: corners[k],
                    end: corners[(k + 1) % 4],
                };
                sketch.entities.push(Entity::new(id, plane, kind));
                id
            })
            .collect();
        let mut fp = Footprint::empty("fills");
        fp.sketch = Some(sketch);
        (fp, lines)
    }

    /// The idle fill of the rectangle's one loop.
    fn fill_of(fp: &Footprint) -> Color {
        let sketch = fp.sketch.as_ref().expect("a sketch");
        let state = FootprintEditorState::from_footprint(fp);
        let loops = find_closed_loops(sketch, &state);
        assert_eq!(loops.len(), 1);
        loop_fill(sketch, &loops[0], false)
    }

    fn tinted(layer: FpLayer) -> Color {
        Color {
            a: ROLE_ALPHA,
            ..layer.color()
        }
    }

    /// An entity with several roles fills in the fill's own priority: a
    /// mask opening beats the keepout on the same edge.
    #[test]
    fn a_multi_role_loop_keeps_its_fill_priority() {
        let (mut fp, lines) = rectangle();
        set_entity_role(&mut fp, lines[0], SketchRole::Keepout);
        let keepout = fill_of(&fp);
        assert_eq!(keepout, tinted(FpLayer::EdgeCuts));
        let edge = fp
            .sketch
            .as_mut()
            .and_then(|s| s.entities.iter_mut().find(|e| e.id == lines[0]))
            .expect("the edge");
        edge.mask_opening = Some(signex_sketch::attr::MaskOpeningAttr {
            layer: signex_types::pcb_layer::PcbLayer::Technical(
                signex_types::pcb_layer::TechnicalLayer::SolderMaskTop,
            ),
        });
        assert_eq!(fill_of(&fp), tinted(FpLayer::FFab));
    }

    /// A first edge whose role has no fill colour (Mechanical, Component
    /// Anchor) leaves the fill to the next edge that has one.
    #[test]
    fn a_role_without_a_fill_colour_defers_to_the_next_edge() {
        for first in [SketchRole::MechanicalLayer, SketchRole::ComponentAnchor] {
            let (mut fp, lines) = rectangle();
            set_entity_role(&mut fp, lines[0], first);
            if first == SketchRole::ComponentAnchor {
                // An anchor takes only a point: set it on the edge directly.
                let edge = fp
                    .sketch
                    .as_mut()
                    .and_then(|s| s.entities.iter_mut().find(|e| e.id == lines[0]))
                    .expect("the edge");
                edge.component_anchor = Some(Default::default());
            }
            set_entity_role(&mut fp, lines[1], SketchRole::SilkTop);
            assert_eq!(fill_of(&fp), tinted(FpLayer::FSilks), "{first:?}");
        }
    }

    /// Courtyard is the one fill that changed: its own colour, not the
    /// Keep-Out one (mockup row 190).
    #[test]
    fn a_courtyard_loop_fills_in_the_courtyard_colour() {
        let (mut fp, lines) = rectangle();
        set_entity_role(&mut fp, lines[0], SketchRole::Courtyard);
        let want = Color {
            a: ROLE_ALPHA,
            ..LayerPalette::Editor.courtyard_colour()
        };
        assert_eq!(fill_of(&fp), want);
        assert_ne!(fill_of(&fp), tinted(FpLayer::EdgeCuts));
    }

    /// No role: the idle plate; selected: the selected plate.
    #[test]
    fn unassigned_and_selected_loops_keep_their_plates() {
        let (fp, _) = rectangle();
        assert_eq!(fill_of(&fp), IDLE_PLATE);
        let sketch = fp.sketch.as_ref().expect("a sketch");
        let state = FootprintEditorState::from_footprint(&fp);
        let loops = find_closed_loops(sketch, &state);
        assert_eq!(loop_fill(sketch, &loops[0], true), SELECTED_PLATE);
    }
}
