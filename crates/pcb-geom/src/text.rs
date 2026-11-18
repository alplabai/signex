//! Where a footprint's property text (designator, comment) sits on the
//! board (#2368, #2434): its anchor, turn, mirror, layer, height and
//! justification, read by the canvas, the board print, the fabrication
//! outputs and the selection index. Rules: `docs/RENDERING_RULES.md`,
//! "pcb-footprint-text".
//!
//! A property's position and rotation are footprint-local, as a pad's are,
//! so the text follows the footprint's move, turn and flip through the same
//! [`Placement`]. A property with no position takes the default spot: the
//! designator [`PCB_FP_TEXT_OFFSET_MM`] above the origin, the comment that
//! far below it, any other text at the origin.

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{Footprint, PCB_DEFAULT_TEXT_SIZE_MM, PCB_FP_TEXT_OFFSET_MM, Point};
use signex_types::pcb_layer::{BoardSide, PcbLayer, TechnicalLayer};
use signex_types::property::PcbProperty;
use signex_types::schematic::HAlign;

use crate::shape::Shape;
use crate::stroke_font::{
    justify_start_mm, stroke_text_width_mm, text_stroke_width_mm, text_strokes,
};
use crate::transform::{Placement, Side, sin_cos_deg};
use crate::units::{GeomError, PointNm, mm_to_nm};

/// A footprint text the board shows: the designator or the comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoardTextField {
    Designator,
    Comment,
}

impl BoardTextField {
    pub const ALL: [BoardTextField; 2] = [BoardTextField::Designator, BoardTextField::Comment];

    /// The property key the text is saved under.
    pub fn key(self) -> &'static str {
        match self {
            BoardTextField::Designator => "Reference",
            BoardTextField::Comment => "Value",
        }
    }

    pub fn of_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|field| field.key() == key)
    }

    /// The word an operand or a report names it with.
    pub fn id(self) -> &'static str {
        match self {
            BoardTextField::Designator => "designator",
            BoardTextField::Comment => "comment",
        }
    }

    pub fn of_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|field| field.id() == id)
    }
}

/// Whether a property of `key` is a text the board shows (the designator or
/// the comment) rather than a parameter.
pub fn is_board_text_key(key: &str) -> bool {
    BoardTextField::of_key(key).is_some()
}

/// A text placed on the board.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextPlacement {
    /// The text's anchor.
    pub anchor: PointNm,
    /// The angle of the text's baseline with the board's x axis, degrees,
    /// in the sense a pad turns.
    pub rotation_deg: f64,
    /// Drawn mirrored: the footprint sits on the bottom side.
    pub mirrored: bool,
}

impl TextPlacement {
    /// The board point `along_mm` along the text's line and `down_mm` down
    /// from its anchor, in the text's own frame (mirrored with the text).
    pub fn offset_mm(&self, along_mm: f64, down_mm: f64) -> Result<PointNm, GeomError> {
        let along = if self.mirrored { -along_mm } else { along_mm };
        let (sin, cos) = sin_cos_deg(self.rotation_deg);
        let x = along * cos - down_mm * sin;
        let y = along * sin + down_mm * cos;
        Ok(self
            .anchor
            .plus(PointNm::new(mm_to_nm(x, "text.x")?, mm_to_nm(y, "text.y")?)))
    }
}

/// A designator or comment as the board draws it.
#[derive(Debug, Clone, PartialEq)]
pub struct FootprintText {
    pub field: BoardTextField,
    /// The property's index in `Footprint::properties`.
    pub index: usize,
    pub content: String,
    pub placement: TextPlacement,
    pub layer: PcbLayer,
    /// The cap height, mm.
    pub size_mm: f64,
    pub justify: HAlign,
}

impl FootprintText {
    /// The strokes that write it ([`text_strokes`]).
    pub fn strokes(&self) -> Result<Vec<Vec<PointNm>>, GeomError> {
        text_strokes(&self.content, &self.placement, self.size_mm, self.justify)
    }

    /// The box it fills: what a click picks it on.
    pub fn box_shape(&self) -> Result<Shape, GeomError> {
        text_box(&self.content, &self.placement, self.size_mm, self.justify)
    }
}

/// The index of `footprint`'s first property holding `field`.
pub fn property_index(footprint: &Footprint, field: BoardTextField) -> Option<usize> {
    footprint
        .properties
        .iter()
        .position(|p| p.key == field.key())
}

/// `field` of `footprint` placed on the board, shown or not; `None` when the
/// footprint has no such property.
pub fn footprint_text(
    footprint: &Footprint,
    field: BoardTextField,
) -> Option<Result<FootprintText, GeomError>> {
    let index = property_index(footprint, field)?;
    let property = &footprint.properties[index];
    Some(
        property_text_placement(footprint, property).map(|placement| FootprintText {
            field,
            index,
            content: property.value.clone(),
            placement,
            layer: property_text_layer(footprint, property),
            size_mm: property_text_size_mm(property),
            justify: property.justify,
        }),
    )
}

/// The designator and comment `footprint` shows: not hidden, not empty, and
/// placed. Parameters are not drawn.
pub fn footprint_texts(footprint: &Footprint) -> Vec<FootprintText> {
    shown_footprint_texts(footprint)
        .into_iter()
        .filter_map(Result::ok)
        .collect()
}

/// The designator and comment `footprint` shows (not hidden, not empty),
/// each placed or refused, for a caller that must account for a text it
/// cannot draw (the fabrication outputs).
pub fn shown_footprint_texts(footprint: &Footprint) -> Vec<Result<FootprintText, GeomError>> {
    BoardTextField::ALL
        .into_iter()
        .filter(|&field| {
            property_index(footprint, field).is_some_and(|index| {
                let property = &footprint.properties[index];
                !property.hidden && !property.value.trim().is_empty()
            })
        })
        .filter_map(|field| footprint_text(footprint, field))
        .collect()
}

/// The layer `property` is drawn on: its own, else the overlay of the
/// footprint's side.
pub fn property_text_layer(footprint: &Footprint, property: &PcbProperty) -> PcbLayer {
    if let Some(layer) = property.layer.as_deref().and_then(parse_pcb_layer) {
        return layer;
    }
    let side = parse_pcb_layer(&footprint.layer).and_then(PcbLayer::side);
    PcbLayer::Technical(match side {
        Some(BoardSide::Bottom) => TechnicalLayer::SilkscreenBottom,
        _ => TechnicalLayer::SilkscreenTop,
    })
}

/// The cap height of `property`, mm: its own, else the PCB default.
pub fn property_text_size_mm(property: &PcbProperty) -> f64 {
    property
        .font_size
        .filter(|size| size.is_finite() && *size > 0.0)
        .unwrap_or(PCB_DEFAULT_TEXT_SIZE_MM)
}

/// Where `property` of `footprint` is drawn. Refuses a non-finite position,
/// angle or footprint placement.
pub fn property_text_placement(
    footprint: &Footprint,
    property: &PcbProperty,
) -> Result<TextPlacement, GeomError> {
    if !property.rotation.is_finite() {
        return Err(GeomError::Unrepresentable("property.rotation"));
    }
    let placement = Placement::of_footprint(footprint)?;
    let local = property
        .position
        .unwrap_or_else(|| default_position(&property.key));
    let local = PointNm::from_mm(local, "property.position")?;
    Ok(place_text(&placement, local, property.rotation))
}

/// The box a line of `content` written `size_mm` high at `at` fills, with
/// half the pen around it.
pub fn text_box(
    content: &str,
    at: &TextPlacement,
    size_mm: f64,
    justify: HAlign,
) -> Result<Shape, GeomError> {
    if !(size_mm.is_finite() && size_mm > 0.0) {
        return Err(GeomError::Unrepresentable("text.size"));
    }
    let width = stroke_text_width_mm(content, size_mm);
    let pen = text_stroke_width_mm(size_mm);
    Ok(Shape::Rect {
        center: justified_centre(at, justify, width)?,
        half_w: mm_to_nm((width + pen) / 2.0, "text width")?.abs(),
        half_h: mm_to_nm((size_mm + pen) / 2.0, "text height")?.abs(),
        rotation_deg: at.rotation_deg,
    })
}

/// The middle of a line `width_mm` long justified by `justify` at `at`: a
/// drawer with its own glyph widths centres its line here.
pub fn justified_centre(
    at: &TextPlacement,
    justify: HAlign,
    width_mm: f64,
) -> Result<PointNm, GeomError> {
    at.offset_mm(justify_start_mm(justify, width_mm) + width_mm / 2.0, 0.0)
}

/// The footprint-local position a text of `footprint` takes to sit at the
/// board point `world`.
pub fn local_text_position(footprint: &Footprint, world: Point) -> Result<Point, GeomError> {
    let placement = Placement::of_footprint(footprint)?;
    Ok(placement
        .to_local(PointNm::from_mm(world, "text.position")?)
        .to_mm())
}

/// The text's own rotation that turns its baseline `world_deg` on the board
/// on `footprint`; the inverse of the turn [`property_text_placement`]
/// composes.
pub fn local_text_rotation(footprint: &Footprint, world_deg: f64) -> Result<f64, GeomError> {
    if !world_deg.is_finite() {
        return Err(GeomError::Unrepresentable("text.rotation"));
    }
    let placement = Placement::of_footprint(footprint)?;
    let own = match placement.side {
        Side::Top => world_deg - placement.rotation_deg,
        Side::Bottom => placement.rotation_deg - world_deg,
    };
    Ok(own.rem_euclid(360.0))
}

/// A text at the footprint-local `local`, turned by its own `rotation`
/// degrees, placed by `placement`: a property text or a text drawing.
pub(crate) fn place_text(placement: &Placement, local: PointNm, rotation: f64) -> TextPlacement {
    // The x mirror reverses the text's own turn (`M R(t) M = R(-t)`), as
    // for a pad (`pad_world_rotation_deg`).
    let (own, mirrored) = match placement.side {
        Side::Top => (rotation, false),
        Side::Bottom => (-rotation, true),
    };
    TextPlacement {
        anchor: placement.to_world(local),
        rotation_deg: placement.rotation_deg + own,
        mirrored,
    }
}

fn default_position(key: &str) -> Point {
    match BoardTextField::of_key(key) {
        Some(BoardTextField::Designator) => Point::new(0.0, -PCB_FP_TEXT_OFFSET_MM),
        Some(BoardTextField::Comment) => Point::new(0.0, PCB_FP_TEXT_OFFSET_MM),
        None => Point::new(0.0, 0.0),
    }
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
