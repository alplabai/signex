//! A footprint's body: the area a click or a marquee selects it by (#2240).
//!
//! The body is the footprint-local bounding box of its courtyard, or of its
//! pads when it has no courtyard, placed on the board like a pad is (mirror,
//! rotate, translate). So it is a rectangle that turns with the footprint.
//! A footprint with neither is a point at its origin, reached within the
//! pointer's tolerance.

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{Footprint, FpGraphic, Pad, Point};
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};

use crate::graphic::circle_radius;
use crate::rect::RectNm;
use crate::shape::Shape;
use crate::transform::{Placement, sin_cos_deg};
use crate::units::{GeomError, PointNm, mm_to_nm};

/// Whether a courtyard box takes in half of each drawing's line width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stroke {
    Included,
    Excluded,
}

/// The body of `footprint` on the board, see the module notes.
pub fn footprint_body(footprint: &Footprint) -> Result<Shape, GeomError> {
    let placement = Placement::of_footprint(footprint)?;
    let local = match courtyard_extent(footprint, Stroke::Included)? {
        Some(rect) => Some(rect),
        None => pad_extent(&footprint.pads)?,
    };
    let Some(local) = local else {
        return Ok(Shape::Disc {
            center: placement.origin,
            radius: 0,
        });
    };
    Ok(placed_box(&placement, local))
}

/// The courtyard DRC measures: the turned box of the courtyard drawings'
/// outline, their line width left out, since the stroke is drawn for
/// display only. `None` when `footprint` has no courtyard drawing.
pub fn courtyard_outline(footprint: &Footprint) -> Result<Option<Shape>, GeomError> {
    let placement = Placement::of_footprint(footprint)?;
    Ok(courtyard_extent(footprint, Stroke::Excluded)?.map(|local| placed_box(&placement, local)))
}

/// The body DRC measures a part by: its [`courtyard_outline`], or without a
/// courtyard the [`footprint_body`] of its pads (or origin).
pub fn checked_body(footprint: &Footprint) -> Result<Shape, GeomError> {
    match courtyard_outline(footprint)? {
        Some(outline) => Ok(outline),
        None => footprint_body(footprint),
    }
}

/// The footprint-local box `local` placed on the board, as a turned
/// rectangle (a part's 3D body box reads it too, #2939).
pub fn placed_box(placement: &Placement, local: RectNm) -> Shape {
    let centre = PointNm::new(
        local.min.x / 2 + local.max.x / 2,
        local.min.y / 2 + local.max.y / 2,
    );
    Shape::Rect {
        center: placement.to_world(centre),
        half_w: local.width() / 2,
        half_h: local.height() / 2,
        rotation_deg: placement.rotation_deg,
    }
}

/// Whether `graphic` is on a courtyard layer, which the body is taken from.
pub fn is_courtyard(graphic: &FpGraphic) -> bool {
    matches!(
        parse_pcb_layer(&graphic.layer),
        Some(PcbLayer::Technical(
            TechnicalLayer::CourtyardTop | TechnicalLayer::CourtyardBottom
        ))
    )
}

/// The local box of every courtyard graphic, half its stroke included when
/// `stroke` says so. A width that is not a length is refused either way.
fn courtyard_extent(footprint: &Footprint, stroke: Stroke) -> Result<Option<RectNm>, GeomError> {
    let mut extent: Option<RectNm> = None;
    for graphic in footprint.graphics.iter().filter(|g| is_courtyard(g)) {
        let half_width = (mm_to_nm(graphic.width, "courtyard.width")? / 2).abs();
        let stroke = match stroke {
            Stroke::Included => half_width,
            Stroke::Excluded => 0,
        };
        let mut grow = |p: Point, reach: i64| -> Result<(), GeomError> {
            let p = PointNm::from_mm(p, "courtyard.point")?;
            let rect = RectNm::point(p).inflate(reach + stroke);
            extent = Some(extent.map_or(rect, |e| e.union(&rect)));
            Ok(())
        };
        for point in [graphic.start, graphic.end, graphic.mid, graphic.position]
            .into_iter()
            .flatten()
        {
            grow(point, 0)?;
        }
        if let Some(center) = graphic.center {
            let radius = circle_radius(center, graphic.radius, graphic.start, graphic.end)?;
            grow(center, radius)?;
        }
        for &point in &graphic.points {
            grow(point, 0)?;
        }
    }
    Ok(extent)
}

/// The local box of every pad, each taken as its turned rectangle.
fn pad_extent(pads: &[Pad]) -> Result<Option<RectNm>, GeomError> {
    let mut extent: Option<RectNm> = None;
    for pad in pads {
        if !pad.rotation.is_finite() {
            return Err(GeomError::Unrepresentable("pad.rotation"));
        }
        let centre = PointNm::from_mm(pad.position, "pad.position")?;
        let half_w = (mm_to_nm(pad.size.x, "pad.size")? / 2).abs() as f64;
        let half_h = (mm_to_nm(pad.size.y, "pad.size")? / 2).abs() as f64;
        let (sin, cos) = sin_cos_deg(pad.rotation);
        let reach_x = (cos.abs() * half_w + sin.abs() * half_h).round() as i64;
        let reach_y = (sin.abs() * half_w + cos.abs() * half_h).round() as i64;
        let rect = RectNm::around(centre, reach_x, reach_y);
        extent = Some(extent.map_or(rect, |e| e.union(&rect)));
    }
    Ok(extent)
}

#[cfg(test)]
#[path = "body_tests.rs"]
mod tests;
