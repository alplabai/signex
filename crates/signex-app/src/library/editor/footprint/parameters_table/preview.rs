//! The live bake preview under the parameters table (mockup row 189): what
//! the sketch bakes, drawn from the editor's current solve, with its driving
//! dimensions. It never solves. It reads `last_solve` and the parameters
//! that solve ran with, so while a row is in error the last good bake stays
//! on show, marked. Both hosts draw the baking edges; a footprint also
//! draws its baked pads. Drawing: `preview_canvas`.

use std::collections::{BTreeSet, HashMap};
use std::f64::consts::TAU;

use signex_library::primitive::footprint::{Pad, PadShape};
use signex_sketch::SketchData;
use signex_sketch::constraint::{ConstraintKind, DimTarget};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::parameter_refs::{RowValue, resolve_rows};
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::solver::residual::resolve_dim;
use signex_sketch::solver::state::{circle_radius, point_xy};
use signex_types::sketch::SketchHost;

use crate::app::FootprintDocument;
use crate::library::editor::footprint::sketch_mode::value_edit::bound_parameter;

/// Straight pieces a full circle is drawn with; an arc takes its share.
const CIRCLE_SEGMENTS: usize = 48;

/// A point in sketch millimetres.
pub(super) type Mm = (f64, f64);

/// What the preview draws, sketch mm.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct BakePreview {
    /// Every edge that bakes, as a polyline. Construction and centerline
    /// edges never bake and are left out.
    pub(super) edges: Vec<Vec<Mm>>,
    /// The baked pads; always empty on a board sketch.
    pub(super) pads: Vec<PreviewPad>,
    pub(super) dimensions: Vec<DrivingDimension>,
    /// A parameter is in error, so this is the last good bake.
    pub(super) is_last_good: bool,
}

/// A baked pad as drawn.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum PreviewPad {
    Round { centre: Mm, radius: f64 },
    Outline(Vec<Mm>),
}

/// A dimension that drives the sketch (not a reference one), at the middle
/// of what it measures.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct DrivingDimension {
    pub(super) at: Mm,
    /// The parameter its expression names, when it names exactly one.
    pub(super) name: Option<String>,
    /// Millimetres, or radians for an angle.
    pub(super) value: f64,
    pub(super) is_angle: bool,
    /// The parameter it names is in error: `value` is the last good one.
    pub(super) is_last_good: bool,
}

/// The preview of `editor`'s sketch at its current solve; `None` when there
/// is no sketch or no solve to draw.
pub(super) fn of_editor(editor: &FootprintDocument) -> Option<BakePreview> {
    let sketch = editor.primitive().sketch.as_ref()?;
    let solve = editor.state.last_solve.as_ref()?;
    Some(BakePreview::of(
        sketch,
        solve,
        editor.state.sketch_host(),
        &failing_parameters(sketch),
    ))
}

/// The parameters that do not resolve now, in error or blocked by one.
pub(super) fn failing_parameters(sketch: &SketchData) -> BTreeSet<String> {
    resolve_rows(&sketch.parameters)
        .into_iter()
        .filter(|(_, value)| !matches!(value, RowValue::Value(_)))
        .map(|(name, _)| name)
        .collect()
}

impl BakePreview {
    /// The bake of `sketch` at `solve`, with the parameters `solve` ran
    /// with. `failing` names the parameters in error now.
    pub(super) fn of(
        sketch: &SketchData,
        solve: &FullSolveOutput,
        host: SketchHost,
        failing: &BTreeSet<String>,
    ) -> Self {
        let at = Positions { sketch, solve };
        let edges = sketch
            .entities
            .iter()
            .filter(|entity| !entity.bake_skipped())
            .filter_map(|entity| at.edge(entity))
            .collect();
        let pads = match host {
            SketchHost::Footprint => baked_pads(sketch, solve)
                .iter()
                .map(PreviewPad::of)
                .collect(),
            SketchHost::Board => Vec::new(),
        };
        Self {
            edges,
            pads,
            dimensions: driving_dimensions(sketch, solve, &at, failing),
            is_last_good: !failing.is_empty(),
        }
    }

    /// The least and greatest corner round everything drawn.
    pub(super) fn extent(&self) -> Option<(Mm, Mm)> {
        let pad_points = self.pads.iter().flat_map(|pad| match pad {
            PreviewPad::Round { centre, radius } => vec![
                (centre.0 - radius, centre.1 - radius),
                (centre.0 + radius, centre.1 + radius),
            ],
            PreviewPad::Outline(points) => points.clone(),
        });
        self.edges
            .iter()
            .flatten()
            .copied()
            .chain(pad_points)
            .chain(self.dimensions.iter().map(|d| d.at))
            .fold(None, |acc, p| {
                Some(match acc {
                    None => (p, p),
                    Some((lo, hi)) => (
                        (lo.0.min(p.0), lo.1.min(p.1)),
                        (hi.0.max(p.0), hi.1.max(p.1)),
                    ),
                })
            })
    }
}

/// The pads `sketch` bakes at `solve`, by the bake the editor runs. The
/// editor's own bake already reported a refusal or warning, so a refused
/// bake here draws no pads and says nothing more.
fn baked_pads(sketch: &SketchData, solve: &FullSolveOutput) -> Vec<Pad> {
    let mut pads = Vec::new();
    let mut warnings = Vec::new();
    let baked = signex_bake::bake_pads(sketch, solve, &solve.params, &mut pads, &mut warnings)
        .and_then(|()| {
            signex_bake::bake_arrays(sketch, solve, &solve.params, &mut pads, &mut warnings)
        });
    if baked.is_err() {
        return Vec::new();
    }
    pads
}

impl PreviewPad {
    fn of(pad: &Pad) -> Self {
        let centre = (pad.position[0], pad.position[1]);
        let [w, h] = pad.size;
        if matches!(pad.shape, PadShape::Round) && (w - h).abs() < f64::EPSILON {
            return Self::Round {
                centre,
                radius: w / 2.0,
            };
        }
        let (sin, cos) = pad.rotation.to_radians().sin_cos();
        let corners = [(-w, -h), (w, -h), (w, h), (-w, h)].map(|(x, y)| {
            let (x, y) = (x / 2.0, y / 2.0);
            (centre.0 + x * cos - y * sin, centre.1 + x * sin + y * cos)
        });
        Self::Outline(corners.to_vec())
    }
}

/// Where the solve put things.
struct Positions<'a> {
    sketch: &'a SketchData,
    solve: &'a FullSolveOutput,
}

impl Positions<'_> {
    fn point(&self, id: SketchEntityId) -> Option<Mm> {
        let result = &self.solve.result;
        point_xy(id, &result.state, &result.index, self.sketch)
    }

    /// The polyline of an edge; `None` for a point or an edge the solve
    /// cannot place.
    fn edge(&self, entity: &Entity) -> Option<Vec<Mm>> {
        match entity.kind {
            EntityKind::Point { .. } => None,
            EntityKind::Line { start, end } => Some(vec![self.point(start)?, self.point(end)?]),
            EntityKind::Circle { center, radius } => {
                let result = &self.solve.result;
                let radius =
                    circle_radius(entity.id, &result.state, &result.index).unwrap_or(radius);
                Some(arc_points(self.point(center)?, radius, 0.0, TAU))
            }
            EntityKind::Arc {
                center,
                start,
                end,
                sweep_ccw,
            } => {
                let (c, s, e) = (self.point(center)?, self.point(start)?, self.point(end)?);
                let from = (s.1 - c.1).atan2(s.0 - c.0);
                let to = (e.1 - c.1).atan2(e.0 - c.0);
                let sweep = if sweep_ccw {
                    (to - from).rem_euclid(TAU)
                } else {
                    -(from - to).rem_euclid(TAU)
                };
                let radius = (s.0 - c.0).hypot(s.1 - c.1);
                Some(arc_points(c, radius, from, sweep))
            }
        }
    }

    /// Where a dimension on `id` is labelled: a point itself, a line's
    /// middle, an arc's or circle's centre.
    fn anchor(
        &self,
        id: SketchEntityId,
        entities: &HashMap<SketchEntityId, &Entity>,
    ) -> Option<Mm> {
        match entities.get(&id)?.kind {
            EntityKind::Point { .. } => self.point(id),
            EntityKind::Line { start, end } => {
                let (a, b) = (self.point(start)?, self.point(end)?);
                Some(((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0))
            }
            EntityKind::Arc { center, .. } | EntityKind::Circle { center, .. } => {
                self.point(center)
            }
        }
    }
}

/// Points along an arc of `sweep` radians from `from`, both ends included.
fn arc_points(centre: Mm, radius: f64, from: f64, sweep: f64) -> Vec<Mm> {
    let share = (sweep.abs() / TAU * CIRCLE_SEGMENTS as f64).ceil();
    let steps = (share as usize).max(1);
    (0..=steps)
        .map(|i| {
            let angle = from + sweep * i as f64 / steps as f64;
            (
                centre.0 + radius * angle.cos(),
                centre.1 + radius * angle.sin(),
            )
        })
        .collect()
}

/// The sketch's driving dimensions with the values `solve` ran with.
fn driving_dimensions(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    at: &Positions<'_>,
    failing: &BTreeSet<String>,
) -> Vec<DrivingDimension> {
    let entities: HashMap<SketchEntityId, &Entity> =
        sketch.entities.iter().map(|e| (e.id, e)).collect();
    sketch
        .constraints
        .iter()
        .filter(|c| !sketch.is_driven(c.id))
        .filter_map(|c| {
            let target = c.kind.dim_target()?;
            let value = resolve_dim(target, &solve.params).ok()?;
            let anchors: Vec<Mm> = c
                .kind
                .entities()
                .filter_map(|id| at.anchor(id, &entities))
                .collect();
            if anchors.is_empty() {
                return None;
            }
            let n = anchors.len() as f64;
            let (sx, sy) = anchors
                .iter()
                .fold((0.0, 0.0), |(x, y), p| (x + p.0, y + p.1));
            let name = match target {
                DimTarget::Expr(source) => bound_parameter(source, sketch).map(str::to_owned),
                DimTarget::Literal(_) => None,
            };
            Some(DrivingDimension {
                at: (sx / n, sy / n),
                is_last_good: name.as_ref().is_some_and(|n| failing.contains(n)),
                name,
                value,
                is_angle: matches!(c.kind, ConstraintKind::Angle { .. }),
            })
        })
        .collect()
}
