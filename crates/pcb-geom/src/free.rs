//! The shapes of a board's free primitives (#2373) and of its strings:
//! what the pick index, the copper list and the design rule check read.
//! A free pad goes through the footprint pad geometry in the board's own
//! frame (`signex_types::pcb_free::free_pad_frame`); an arc is its chords,
//! each a capsule of the arc's width; a dimension is its strokes and the
//! box of its value; a string is the box its glyphs fill.

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{BoardText, Pad, PcbBoard};
use signex_types::pcb_arc::TrackArc;
use signex_types::pcb_dimension::LinearDimension;
use signex_types::pcb_dimension_style::GLYPH_ADVANCE;
use signex_types::pcb_free::free_pad_frame;

use crate::shape::Shape;
use crate::transform::pad_shape;
use crate::units::{GeomError, PointNm, mm_to_nm};

/// How far an arc's chords may stray from the arc, mm: 5 µm, well under
/// any manufacturing tolerance.
pub const ARC_CHORD_ERROR_MM: f64 = 0.005;

/// A free pad's copper.
pub fn free_pad_shape(pad: &Pad) -> Result<Shape, GeomError> {
    pad_shape(&free_pad_frame(), pad)
}

/// Whether `arc` lies on a copper layer this build knows.
pub fn arc_is_copper(arc: &TrackArc) -> bool {
    parse_pcb_layer(&arc.layer).is_some_and(|l| l.is_copper())
}

/// An arc as capsules of its width along its chords
/// ([`ARC_CHORD_ERROR_MM`]); a negative width counts as its magnitude.
pub fn arc_shapes(arc: &TrackArc) -> Result<Vec<Shape>, GeomError> {
    let radius = (mm_to_nm(arc.width, "arc.width")? / 2).abs();
    let points = arc
        .points(ARC_CHORD_ERROR_MM)
        .into_iter()
        .map(|p| PointNm::from_mm(p, "arc"))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(points
        .windows(2)
        .map(|pair| Shape::Capsule {
            a: pair[0],
            b: pair[1],
            radius,
        })
        .collect())
}

/// A dimension as the capsules of its strokes and the box of its value.
/// None for a dimension that draws nothing (`LinearDimension::drawing`).
pub fn dimension_shapes(dimension: &LinearDimension) -> Result<Vec<Shape>, GeomError> {
    let Some(drawing) = dimension.drawing() else {
        return Ok(Vec::new());
    };
    let mut shapes = drawing
        .strokes
        .iter()
        .enumerate()
        .map(|(index, (a, b))| {
            let width = dimension.stroke_width(index);
            Ok(Shape::Capsule {
                a: PointNm::from_mm(*a, "dimension")?,
                b: PointNm::from_mm(*b, "dimension")?,
                radius: (mm_to_nm(width, "dimension.line_width")? / 2).abs(),
            })
        })
        .collect::<Result<Vec<_>, GeomError>>()?;
    for arrow in &drawing.filled_arrows {
        shapes.push(triangle(arrow)?);
    }
    shapes.push(text_box(
        drawing.label_at,
        dimension.label_length(),
        dimension.text_height,
        drawing.label_rotation_deg,
    )?);
    Ok(shapes)
}

/// A filled arrowhead as a counter-clockwise polygon.
fn triangle(corners: &[signex_types::pcb::Point; 3]) -> Result<Shape, GeomError> {
    let mut ring = corners
        .iter()
        .map(|p| PointNm::from_mm(*p, "dimension.arrow"))
        .collect::<Result<Vec<_>, _>>()?;
    let [a, b, c] = corners;
    if (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x) < 0.0 {
        ring.reverse();
    }
    Ok(Shape::Polygon {
        ring: ring.into(),
        radius: 0,
    })
}

/// The box a board string's glyphs fill, centred on its position and
/// turned with it.
pub fn text_shape(text: &BoardText) -> Result<Shape, GeomError> {
    if !text.rotation.is_finite() {
        return Err(GeomError::Unrepresentable("text.rotation"));
    }
    let height = if text.font_size.is_finite() && text.font_size > 0.0 {
        text.font_size
    } else {
        signex_types::pcb::PCB_DEFAULT_TEXT_SIZE_MM
    };
    text_box_at(
        text.position,
        height,
        text.text.chars().count(),
        text.rotation,
    )
}

fn text_box_at(
    center: signex_types::pcb::Point,
    height: f64,
    chars: usize,
    rotation_deg: f64,
) -> Result<Shape, GeomError> {
    text_box(
        center,
        height * GLYPH_ADVANCE * chars.max(1) as f64,
        height,
        rotation_deg,
    )
}

fn text_box(
    center: signex_types::pcb::Point,
    width: f64,
    height: f64,
    rotation_deg: f64,
) -> Result<Shape, GeomError> {
    Ok(Shape::Rect {
        center: PointNm::from_mm(center, "text.position")?,
        half_w: (mm_to_nm(width, "text width")? / 2).abs(),
        half_h: (mm_to_nm(height, "text height")? / 2).abs(),
        rotation_deg,
    })
}

/// The copper arcs and free pads on `net`, in board order. Empty for net 0.
pub fn free_copper_on(board: &PcbBoard, net: u32) -> impl Iterator<Item = crate::ItemRef> + '_ {
    let arcs = board
        .free
        .arcs
        .iter()
        .filter(move |a| net != 0 && a.net == net && arc_is_copper(a))
        .map(|a| crate::ItemRef::Arc(a.uuid));
    let pads = board
        .free
        .pads
        .iter()
        .filter(move |p| net != 0 && p.net.as_ref().is_some_and(|n| n.number == net))
        .map(|p| crate::ItemRef::FreePad(p.uuid));
    arcs.chain(pads)
}
