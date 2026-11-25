//! The link between a pad and the sketch geometry that drives it (mockup
//! row 192, items D5 and D6). Edit in Sketch from a pad focuses its driving entity;
//! the canvas highlights the driving profile and a floating link goes back
//! to the pad. A selected pad entity offers Jump to pad. See
//! docs/FOOTPRINT_SKETCH.md, "Pads and the sketch".

use std::collections::{HashMap, HashSet};

use signex_sketch::SketchData;
use signex_sketch::entity::EntityKind;
use signex_sketch::geom::{Point2, point_in_polygon, signed_area};
use signex_sketch::id::SketchEntityId;
use signex_types::sketch::SketchHost;

use super::{EditorMode, FootprintEditorState, SketchOverlay};

/// The pad an Edit in Sketch came from and the entity that drives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PadLinkFocus {
    pub pad_idx: usize,
    pub entity: SketchEntityId,
    /// The pad is a pattern instance of `entity`, which is its source.
    pub is_instance: bool,
}

/// Events of the floating pad links on the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadLinkMsg {
    /// Back to pad or Jump to pad was pressed: show the pad `entity`
    /// drives, in Footprint mode.
    PadLinkPressed { entity: SketchEntityId },
    /// Edit in Sketch was pressed on the Footprint canvas: pad `pad_idx`'s
    /// driving entity, in sketch mode.
    EditInSketchPressed { pad_idx: usize },
}

/// What the floating link offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadLinkOffer {
    /// After Edit in Sketch: back to the pad it came from.
    BackToPad {
        pad_idx: usize,
        entity: SketchEntityId,
    },
    /// A pad's driving entity is selected: jump to its pad.
    JumpToPad { entity: SketchEntityId },
}

impl PadLinkOffer {
    /// The entity whose pad the link shows.
    pub fn entity(self) -> SketchEntityId {
        match self {
            Self::BackToPad { entity, .. } | Self::JumpToPad { entity } => entity,
        }
    }
}

impl FootprintEditorState {
    /// The Edit in Sketch focus while it holds: in sketch mode, with its
    /// entity still the selection and still driving its pad. A mode change
    /// or a selection elsewhere ends it.
    pub fn live_pad_link(&self) -> Option<PadLinkFocus> {
        self.pad_link.filter(|focus| {
            self.mode == EditorMode::Sketch
                && self.selected_sketch == Some(focus.entity)
                && self.pads.get(focus.pad_idx).is_some_and(|pad| {
                    focus.is_instance || pad.sketch_entity_id == Some(focus.entity)
                })
        })
    }

    /// The outline of the pattern instance an Edit in Sketch came from
    /// (mockup row 192): drawn dashed, apart from its source profile.
    /// `None` when the focused pad is the source itself.
    pub fn pad_link_instance_outline(&self) -> Option<[(f64, f64); 4]> {
        let focus = self.live_pad_link().filter(|focus| focus.is_instance)?;
        Some(self.pads.get(focus.pad_idx)?.rotated_corners_mm())
    }

    /// The entity driving pad `pad_idx`, and whether the pad is a pattern
    /// instance of it: a baked instance carries no entity of its own, so
    /// its pattern's source answers.
    fn driving_entity(
        &self,
        sketch: Option<&SketchData>,
        pad_idx: usize,
    ) -> Option<(SketchEntityId, bool)> {
        let pad = self.pads.get(pad_idx)?;
        if let Some(id) = pad.sketch_entity_id {
            return Some((id, false));
        }
        let (sketch, solve) = (sketch?, self.last_solve.as_ref()?);
        let (pattern, _, _) = signex_bake::instance_at(sketch, solve, pad.position_mm)?;
        Some((sketch.innermost_source(pattern)?, true))
    }

    /// The pad `entity` drives, if any.
    pub fn pad_driven_by(&self, entity: SketchEntityId) -> Option<usize> {
        self.pads
            .iter()
            .position(|pad| pad.sketch_entity_id == Some(entity))
    }

    /// What the floating link on the sketch canvas offers, if anything.
    /// A board sketch has no pads.
    pub fn pad_link_offer(&self) -> Option<PadLinkOffer> {
        if self.mode != EditorMode::Sketch || self.sketch_host() == SketchHost::Board {
            return None;
        }
        if let Some(focus) = self.live_pad_link() {
            return Some(PadLinkOffer::BackToPad {
                pad_idx: focus.pad_idx,
                entity: focus.entity,
            });
        }
        let entity = self.selected_sketch?;
        self.pad_driven_by(entity)?;
        Some(PadLinkOffer::JumpToPad { entity })
    }

    /// Edit in Sketch (mockup row 192, item D5): sketch mode with pad `pad_idx`'s driving entity
    /// selected and focused.
    pub fn edit_pad_in_sketch(&mut self, pad_idx: usize, sketch: Option<&SketchData>) {
        let driving = self.driving_entity(sketch, pad_idx);
        self.mode = EditorMode::Sketch;
        self.selected_pad = None;
        self.selected_sketch = driving.map(|(entity, _)| entity);
        self.pad_link = driving.map(|(entity, is_instance)| PadLinkFocus {
            pad_idx,
            entity,
            is_instance,
        });
        self.sketch_overlay = SketchOverlay::default();
    }

    /// Back to pad / Jump to pad (mockup row 192, item D6): Footprint mode with the pad `entity`
    /// drives selected; after an Edit in Sketch, the pad it came from, which
    /// may be one instance of many.
    pub fn return_to_pad(&mut self, entity: SketchEntityId) {
        let pad_idx = self
            .live_pad_link()
            .filter(|focus| focus.entity == entity)
            .map(|focus| focus.pad_idx)
            .or_else(|| self.pad_driven_by(entity));
        self.mode = EditorMode::Normal;
        self.selected_sketch = None;
        self.selected_sketch_secondary = None;
        self.selected_pad = pad_idx;
        self.pad_link = None;
        self.sketch_overlay = SketchOverlay::default();
    }

    pub fn apply_pad_link(&mut self, msg: PadLinkMsg, sketch: Option<&SketchData>) {
        match msg {
            PadLinkMsg::PadLinkPressed { entity } => self.return_to_pad(entity),
            PadLinkMsg::EditInSketchPressed { pad_idx } => self.edit_pad_in_sketch(pad_idx, sketch),
        }
    }
}

/// The entities drawn as `entity`'s driving profile: the entity itself, the
/// Lines joining its pad's `corners`, and the closed loop of Lines it is an
/// edge of or, for a Point, the smallest one it lies inside.
pub fn driving_profile(
    sketch: &SketchData,
    entity: SketchEntityId,
    corners: Option<[SketchEntityId; 4]>,
) -> Vec<SketchEntityId> {
    let lines: Vec<(SketchEntityId, SketchEntityId, SketchEntityId)> = sketch
        .entities
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Line { start, end } => Some((e.id, start, end)),
            _ => None,
        })
        .collect();
    let mut out = vec![entity];
    if let Some(corners) = corners {
        out.extend(
            lines
                .iter()
                .filter(|(_, s, e)| corners.contains(s) && corners.contains(e))
                .map(|(id, _, _)| *id),
        );
    }
    let loops = closed_loops(&lines);
    let own_loop = loops.iter().find(|lp| lp.contains(&entity)).or_else(|| {
        let at = point_at(sketch, entity)?;
        loops
            .iter()
            .filter_map(|lp| {
                let polygon = loop_polygon(sketch, &lines, lp)?;
                point_in_polygon(at, &polygon).then(|| (signed_area(&polygon).abs(), lp))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, lp)| lp)
    });
    if let Some(lp) = own_loop {
        out.extend(lp.iter().copied());
    }
    let mut seen = HashSet::new();
    out.retain(|id| seen.insert(*id));
    out
}

fn point_at(sketch: &SketchData, id: SketchEntityId) -> Option<Point2> {
    sketch
        .entities
        .iter()
        .find(|e| e.id == id)
        .and_then(|e| match e.kind {
            EntityKind::Point { x, y } => Some(Point2::new(x, y)),
            _ => None,
        })
}

/// The Lines of every closed loop: a set of Lines joined end to end in
/// which every endpoint joins exactly two of them.
fn closed_loops(
    lines: &[(SketchEntityId, SketchEntityId, SketchEntityId)],
) -> Vec<Vec<SketchEntityId>> {
    let mut by_point: HashMap<SketchEntityId, Vec<usize>> = HashMap::new();
    for (i, (_, s, e)) in lines.iter().enumerate() {
        by_point.entry(*s).or_default().push(i);
        by_point.entry(*e).or_default().push(i);
    }
    let mut seen = vec![false; lines.len()];
    let mut loops = Vec::new();
    for first in 0..lines.len() {
        if seen[first] {
            continue;
        }
        let mut component = Vec::new();
        let mut stack = vec![first];
        seen[first] = true;
        while let Some(i) = stack.pop() {
            component.push(i);
            let (_, s, e) = lines[i];
            for j in by_point[&s].iter().chain(&by_point[&e]) {
                if !seen[*j] {
                    seen[*j] = true;
                    stack.push(*j);
                }
            }
        }
        let closed = component.len() >= 3
            && component.iter().all(|&i| {
                let (_, s, e) = lines[i];
                by_point[&s].len() == 2 && by_point[&e].len() == 2
            });
        if closed {
            loops.push(component.iter().map(|&i| lines[i].0).collect());
        }
    }
    loops
}

/// The vertices of a closed loop, in walking order.
fn loop_polygon(
    sketch: &SketchData,
    lines: &[(SketchEntityId, SketchEntityId, SketchEntityId)],
    lp: &[SketchEntityId],
) -> Option<Vec<Point2>> {
    let edges: Vec<(SketchEntityId, SketchEntityId)> = lines
        .iter()
        .filter(|(id, _, _)| lp.contains(id))
        .map(|(_, s, e)| (*s, *e))
        .collect();
    let (start, mut at) = *edges.first()?;
    let mut used = vec![false; edges.len()];
    used[0] = true;
    let mut polygon = vec![point_at(sketch, start)?];
    while at != start {
        polygon.push(point_at(sketch, at)?);
        let next = edges
            .iter()
            .enumerate()
            .find(|(i, (s, e))| !used[*i] && (*s == at || *e == at))?;
        used[next.0] = true;
        let (s, e) = *next.1;
        at = if s == at { e } else { s };
    }
    Some(polygon)
}
