//! Component height and 3D body clearance (#2939). A part's 3D body is a
//! prism: an outline on the board and a span of heights above the face it
//! stands on. It comes from the library model of the footprint the part was
//! made from (an attached model, measured as the box of its points, or the
//! extruded 3D Body), else from the part's stored height over its checked
//! body. A part with neither has no height and is not checked. Two bodies
//! on one side are measured in 3D: the gap between their outlines and the
//! gap between their height spans, combined. Rules and sources:
//! `docs/DRC_RULES.md`, "Component height and 3D body clearance".

use std::collections::HashMap;

use pcb_geom::{
    GeomError, ItemRef, Placement, PointNm, RectNm, Rejected, Shape, Side, ccw_ring, checked_body,
    layer_bit, mm_to_nm, placed_box,
};
use signex_types::part::FootprintSourceRef;
use signex_types::pcb::{Footprint, PcbBoard, Point};
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

use crate::findings::mm;
use crate::near::{gap_under, near_pairs};
use crate::user_rules::LimitKind;
use crate::{DrcRules, Violation, ViolationKind};

/// Where a part's height came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeightSource {
    /// The model file attached to its library footprint.
    Model,
    /// The 3D Body of its library footprint.
    Body,
    /// The Height stored on the placed part.
    Stored,
}

impl HeightSource {
    /// Stable name in answers.
    pub const fn as_str(self) -> &'static str {
        match self {
            HeightSource::Model => "model",
            HeightSource::Body => "body",
            HeightSource::Stored => "stored",
        }
    }
}

/// The 3D body a library footprint gives its parts, in the footprint's
/// frame: mm, y down the board as the footprint's pads, z up from the face
/// the part stands on.
#[derive(Debug, Clone, PartialEq)]
pub enum BodyModel {
    /// The points of an attached model, placed by its attachment.
    Points(Vec<[f64; 3]>),
    /// An extruded 3D Body: its outline (`None` takes the part's checked
    /// body), how far above the face it starts and how tall it is.
    Extrusion {
        outline: Option<Vec<[f64; 2]>>,
        offset_z_mm: f64,
        height_mm: f64,
    },
}

/// The models read for the library footprints the board's parts came from.
pub type BodyModels = HashMap<FootprintSourceRef, BodyModel>;

/// One part's 3D body on the board.
#[derive(Debug, Clone, PartialEq)]
pub struct PartBody {
    pub footprint: Uuid,
    pub side: Side,
    pub outline: Shape,
    /// Where the body starts and ends above the part's face, nm.
    pub bottom_nm: i64,
    pub top_nm: i64,
    pub source: HeightSource,
}

/// The 3D body of `footprint`: its library model in `models`, else its
/// stored height; `None` when it has neither. An error when a coordinate
/// or height is no length.
pub fn part_body(
    footprint: &Footprint,
    models: &BodyModels,
) -> Result<Option<PartBody>, GeomError> {
    let placement = Placement::of_footprint(footprint)?;
    let model = footprint
        .library_ref
        .as_ref()
        .and_then(|source| models.get(source));
    let from_model = match model {
        Some(BodyModel::Points(points)) => from_points(&placement, points)?,
        Some(BodyModel::Extrusion {
            outline,
            offset_z_mm,
            height_mm,
        }) => from_extrusion(
            footprint,
            &placement,
            outline.as_deref(),
            *offset_z_mm,
            *height_mm,
        )?,
        None => None,
    };
    let found = match from_model {
        Some(found) => Some(found),
        None => from_stored(footprint)?,
    };
    Ok(found.map(|(outline, bottom_nm, top_nm, source)| PartBody {
        footprint: footprint.uuid,
        side: placement.side,
        outline,
        bottom_nm,
        top_nm,
        source,
    }))
}

/// A body before it is placed on a part: outline, bottom, top, source.
type Found = (Shape, i64, i64, HeightSource);

/// The box of a model's points; `None` for a model with no point above
/// the face.
fn from_points(placement: &Placement, points: &[[f64; 3]]) -> Result<Option<Found>, GeomError> {
    let mut local = Vec::with_capacity(points.len());
    let (mut low, mut high) = (i64::MAX, i64::MIN);
    for &[x, y, z] in points {
        local.push(PointNm::from_mm(Point::new(x, y), "model.point")?);
        let z = mm_to_nm(z, "model.point")?;
        low = low.min(z);
        high = high.max(z);
    }
    let Some(extent) = RectNm::bounding(local) else {
        return Ok(None);
    };
    if high <= 0 {
        return Ok(None);
    }
    let outline = placed_box(placement, extent);
    Ok(Some((outline, low.max(0), high, HeightSource::Model)))
}

/// An extruded 3D Body; `None` for one with no height.
fn from_extrusion(
    footprint: &Footprint,
    placement: &Placement,
    outline: Option<&[[f64; 2]]>,
    offset_mm: f64,
    height_mm: f64,
) -> Result<Option<Found>, GeomError> {
    if !(height_mm.is_finite() && height_mm > 0.0) {
        return Ok(None);
    }
    let offset = if offset_mm.is_finite() {
        offset_mm
    } else {
        0.0
    };
    let bottom = mm_to_nm(offset.max(0.0), "body_3d.offset_z")?;
    let top = mm_to_nm(offset + height_mm, "body_3d.height")?;
    if top <= 0 {
        return Ok(None);
    }
    let shape = match outline {
        Some(ring) if ring.len() >= 3 => outline_on_board(placement, ring)?,
        _ => checked_body(footprint)?,
    };
    Ok(Some((shape, bottom, top, HeightSource::Body)))
}

/// A 3D Body outline placed on the board, counter-clockwise.
fn outline_on_board(placement: &Placement, ring: &[[f64; 2]]) -> Result<Shape, GeomError> {
    let placed = ring
        .iter()
        .map(|&[x, y]| {
            PointNm::from_mm(Point::new(x, y), "body_3d.outline").map(|p| placement.to_world(p))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shape = Shape::Polygon {
        ring: ccw_ring(placed),
        radius: 0,
    };
    shape.check_limits()?;
    Ok(shape)
}

/// The stored height over the part's checked body; `None` when unset.
fn from_stored(footprint: &Footprint) -> Result<Option<Found>, GeomError> {
    let height = footprint.component.height_mm;
    if !(height.is_finite() && height > 0.0) {
        return Ok(None);
    }
    let top = mm_to_nm(height, "component.height")?;
    Ok(Some((
        checked_body(footprint)?,
        0,
        top,
        HeightSource::Stored,
    )))
}

/// Every part of `board` with a 3D body, and the parts whose body did not
/// convert.
pub fn part_bodies(board: &PcbBoard, models: &BodyModels) -> (Vec<PartBody>, Vec<Rejected>) {
    let mut bodies = Vec::new();
    let mut rejected = Vec::new();
    for footprint in &board.footprints {
        match part_body(footprint, models) {
            Ok(Some(body)) => bodies.push(body),
            Ok(None) => {}
            Err(error) => rejected.push(Rejected {
                item: ItemRef::Footprint(footprint.uuid),
                error,
            }),
        }
    }
    (bodies, rejected)
}

/// What the two checks found, and the parts they could not measure.
pub(crate) struct BodyFindings {
    pub(crate) violations: Vec<Violation>,
    pub(crate) rejected: Vec<Rejected>,
}

/// The copper layer bit a part's side is resolved on: a rule with
/// `layer == TopCopper` or `BottomCopper` is for that side's parts only.
fn side_bit(side: Side) -> u32 {
    let layer = match side {
        Side::Top => CopperLayer::Top,
        Side::Bottom => CopperLayer::Bottom,
    };
    1 << layer_bit(layer)
}

impl DrcRules {
    /// Whether a written rule sets a component height or 3D clearance
    /// limit: only then are the parts' library models worth reading.
    pub fn measures_bodies(&self) -> bool {
        self.sets(LimitKind::MaxComponentHeight) || self.sets(LimitKind::MinBodyClearance)
    }
}

/// The height and 3D clearance findings of `board`; nothing at all (no
/// part rejected) when no rule sets either limit.
pub(crate) fn violations(board: &PcbBoard, rules: &DrcRules, models: &BodyModels) -> BodyFindings {
    let heights = rules.sets(LimitKind::MaxComponentHeight);
    let clearance = rules.sets(LimitKind::MinBodyClearance);
    if !heights && !clearance {
        return BodyFindings {
            violations: Vec::new(),
            rejected: Vec::new(),
        };
    }
    let (bodies, rejected) = part_bodies(board, models);
    let mut found = Vec::new();
    if heights {
        found.extend(bodies.iter().filter_map(|b| too_tall(b, rules)));
    }
    if clearance {
        found.extend(too_close(&bodies, rules));
    }
    BodyFindings {
        violations: found,
        rejected,
    }
}

fn too_tall(body: &PartBody, rules: &DrcRules) -> Option<Violation> {
    let kind = LimitKind::MaxComponentHeight;
    let (limit, rule) = rules.object_limit(kind, 0, side_bit(body.side))?;
    kind.is_broken(body.top_nm, limit).then(|| Violation {
        kind: kind.violation(),
        items: vec![ItemRef::Footprint(body.footprint)],
        nets: (0, 0),
        layers: 0,
        at: centre(&body.outline),
        measured_nm: body.top_nm,
        required_nm: limit,
        keepout: None,
        rule: Some(rule),
    })
}

fn too_close(bodies: &[PartBody], rules: &DrcRules) -> Vec<Violation> {
    let kind = LimitKind::MinBodyClearance;
    let reach = rules.widest_limit(kind);
    let shapes: Vec<Shape> = bodies.iter().map(|b| b.outline.clone()).collect();
    let pairs = near_pairs(&shapes, reach, |i, j| bodies[i].side == bodies[j].side);
    let mut found = Vec::new();
    for (i, j) in pairs {
        let (a, b) = (&bodies[i], &bodies[j]);
        let Some((limit, rule)) = rules.object_limit(kind, 0, side_bit(a.side)) else {
            continue;
        };
        let Some((gap, at)) = gap_3d(a, b, limit) else {
            continue;
        };
        let mut items = vec![
            ItemRef::Footprint(a.footprint),
            ItemRef::Footprint(b.footprint),
        ];
        items.sort_unstable();
        found.push(Violation {
            kind: kind.violation(),
            items,
            nets: (0, 0),
            layers: 0,
            at,
            measured_nm: gap,
            required_nm: limit,
            keepout: None,
            rule: Some(rule),
        });
    }
    found
}

/// How close two bodies come in 3D, when under `limit` (a limit of 0 finds
/// bodies that touch): the outline gap and the height gap combined, and a
/// point between them on the board.
pub(crate) fn gap_3d(a: &PartBody, b: &PartBody, limit: i64) -> Option<(i64, PointNm)> {
    let (flat, at) = gap_under(&a.outline, &b.outline, limit.max(0))?;
    let rise = (a.bottom_nm.max(b.bottom_nm) - a.top_nm.min(b.top_nm)).max(0);
    let gap = (flat as f64).hypot(rise as f64).floor() as i64;
    let broken = if limit <= 0 { gap == 0 } else { gap < limit };
    broken.then_some((gap, at))
}

fn centre(shape: &Shape) -> PointNm {
    let r = shape.aabb();
    PointNm::new(r.min.x / 2 + r.max.x / 2, r.min.y / 2 + r.max.y / 2)
}

/// The parts the two rules apply to on `board`: every part with a stored
/// height or a library footprint (whose model may give it a body).
pub(crate) fn parts(board: &PcbBoard) -> impl Iterator<Item = Uuid> + '_ {
    board
        .footprints
        .iter()
        .filter(|f| f.library_ref.is_some() || f.component.height_mm > 0.0)
        .map(|f| f.uuid)
}

/// A height or 3D clearance finding in words, naming its parts by
/// designator; `None` for any other kind.
pub fn describe_on(board: &PcbBoard, v: &Violation) -> Option<String> {
    let names = designators(board, v);
    match v.kind {
        ViolationKind::ComponentHeight => Some(format!(
            "{} is {} mm tall, needs at most {} mm",
            names.first().map_or("a part", String::as_str),
            mm(v.measured_nm),
            mm(v.required_nm)
        )),
        ViolationKind::BodyClearance => {
            let pair = names.join(" / ");
            Some(if v.measured_nm == 0 {
                format!("3D bodies touch, {pair}")
            } else {
                format!(
                    "3D body clearance {} mm, needs {} mm, {pair}",
                    mm(v.measured_nm),
                    mm(v.required_nm)
                )
            })
        }
        _ => None,
    }
}

/// The designators of the parts `v` names, in its item order.
pub fn designators(board: &PcbBoard, v: &Violation) -> Vec<String> {
    v.items
        .iter()
        .filter_map(|item| match item {
            ItemRef::Footprint(id) => board.footprints.iter().find(|f| f.uuid == *id),
            _ => None,
        })
        .map(|f| f.reference.clone())
        .collect()
}

#[cfg(test)]
#[path = "bodies_tests.rs"]
mod tests;
