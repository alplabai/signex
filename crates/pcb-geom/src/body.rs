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

use crate::rect::RectNm;
use crate::shape::Shape;
use crate::transform::{Placement, sin_cos_deg};
use crate::units::{GeomError, PointNm, mm_to_nm};

/// The body of `footprint` on the board, see the module notes.
pub fn footprint_body(footprint: &Footprint) -> Result<Shape, GeomError> {
    let placement = Placement::of_footprint(footprint)?;
    let local = match courtyard_extent(footprint)? {
        Some(rect) => Some(rect),
        None => pad_extent(&footprint.pads)?,
    };
    let Some(local) = local else {
        return Ok(Shape::Disc {
            center: placement.origin,
            radius: 0,
        });
    };
    let centre = PointNm::new(
        local.min.x / 2 + local.max.x / 2,
        local.min.y / 2 + local.max.y / 2,
    );
    Ok(Shape::Rect {
        center: placement.to_world(centre),
        half_w: local.width() / 2,
        half_h: local.height() / 2,
        rotation_deg: placement.rotation_deg,
    })
}

fn is_courtyard(graphic: &FpGraphic) -> bool {
    matches!(
        parse_pcb_layer(&graphic.layer),
        Some(PcbLayer::Technical(
            TechnicalLayer::CourtyardTop | TechnicalLayer::CourtyardBottom
        ))
    )
}

/// The local box of every courtyard graphic, its stroke included.
fn courtyard_extent(footprint: &Footprint) -> Result<Option<RectNm>, GeomError> {
    let mut extent: Option<RectNm> = None;
    for graphic in footprint.graphics.iter().filter(|g| is_courtyard(g)) {
        let stroke = (mm_to_nm(graphic.width, "courtyard.width")? / 2).abs();
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
            grow(center, mm_to_nm(graphic.radius, "courtyard.radius")?.abs())?;
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
