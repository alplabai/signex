//! A linear board dimension (#2373): the distance between two points, drawn
//! as a dimension line standing off them, two extension lines, an arrow at
//! each end and the measured value. The board keeps them in
//! `PcbBoard::free` (see docs/PCB_TOOLS.md, "Dimension").

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::format::{parse_pcb_layer, pcb_layer_token};
use crate::pcb::{PCB_DEFAULT_GRAPHIC_WIDTH_MM, PCB_DEFAULT_TEXT_SIZE_MM};
use crate::pcb_layer::{PcbLayer, TechnicalLayer};
use crate::schematic::Point;

/// Decimals of the value a new dimension shows.
pub const DIMENSION_DEFAULT_PRECISION: u8 = 2;

/// How far an extension line runs past the dimension line, in text heights.
const EXTENSION_OVERSHOOT: f64 = 0.5;
/// How long an arrow is, in text heights.
const ARROW_LENGTH: f64 = 1.0;
/// Half the width of an arrow's open end, in arrow lengths.
const ARROW_SPREAD: f64 = 0.3;
/// How far the value's centre stands off the dimension line, in text
/// heights: half a height of gap and half the text.
const LABEL_STANDOFF: f64 = 1.0;

/// A dimension measuring the straight distance from `start` to `end`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinearDimension {
    pub uuid: Uuid,
    pub start: Point,
    pub end: Point,
    /// How far the dimension line stands off `start`–`end`, in mm, along the
    /// quarter turn counter-clockwise from the direction `start` to `end`;
    /// negative is the other side.
    #[serde(default)]
    pub offset: f64,
    #[serde(default)]
    pub layer: String,
    #[serde(default)]
    pub line_width: f64,
    #[serde(default)]
    pub text_height: f64,
    #[serde(default)]
    pub precision: u8,
}

/// The strokes and the value of a dimension, in board mm.
#[derive(Debug, Clone, PartialEq)]
pub struct DimensionDrawing {
    /// Strokes of the dimension's line width: the two extension lines, the
    /// dimension line, then the two sides of each arrow.
    pub strokes: Vec<(Point, Point)>,
    /// The centre of the value's text.
    pub label_at: Point,
    /// The value's turn in degrees, above -90 and at most 90, so it never
    /// reads upside down.
    pub label_rotation_deg: f64,
    /// The measured distance in mm.
    pub value_mm: f64,
}

impl LinearDimension {
    /// A dimension from `start` to `end` standing `offset` mm off them, on
    /// the Dimensions layer with the PCB default line width and text height.
    pub fn new(id: Uuid, start: Point, end: Point, offset: f64) -> Self {
        Self {
            uuid: id,
            start,
            end,
            offset,
            layer: pcb_layer_token(PcbLayer::Technical(TechnicalLayer::Dimensions)),
            line_width: PCB_DEFAULT_GRAPHIC_WIDTH_MM,
            text_height: PCB_DEFAULT_TEXT_SIZE_MM,
            precision: DIMENSION_DEFAULT_PRECISION,
        }
    }

    /// The typed layer, or `None` for a name this build does not know.
    pub fn pcb_layer(&self) -> Option<PcbLayer> {
        parse_pcb_layer(&self.layer)
    }

    /// The measured distance in mm.
    pub fn value_mm(&self) -> f64 {
        (self.end.x - self.start.x).hypot(self.end.y - self.start.y)
    }

    /// The value as drawn: the distance to `precision` decimals, in mm.
    pub fn value_text(&self) -> String {
        format!("{:.*}mm", usize::from(self.precision), self.value_mm())
    }

    /// What the dimension draws; `None` when its points are the same, a
    /// number in it is not finite or its text height is not above zero.
    pub fn drawing(&self) -> Option<DimensionDrawing> {
        let length = self.value_mm();
        let numbers = [length, self.offset, self.text_height];
        if numbers.iter().any(|v| !v.is_finite()) || length == 0.0 || self.text_height <= 0.0 {
            return None;
        }
        let along = Point::new(
            (self.end.x - self.start.x) / length,
            (self.end.y - self.start.y) / length,
        );
        let normal = Point::new(-along.y, along.x);
        let side = if self.offset < 0.0 { -1.0 } else { 1.0 };
        let shift = |p: Point, by: Point, k: f64| Point::new(p.x + by.x * k, p.y + by.y * k);

        let a = shift(self.start, normal, self.offset);
        let b = shift(self.end, normal, self.offset);
        let overshoot = side * self.text_height * EXTENSION_OVERSHOOT;
        let arrow = self.text_height * ARROW_LENGTH;
        let spread = arrow * ARROW_SPREAD;
        let wing = |tip: Point, inward: f64, k: f64| {
            shift(shift(tip, along, inward * arrow), normal, k * spread)
        };
        let strokes = vec![
            (self.start, shift(a, normal, overshoot)),
            (self.end, shift(b, normal, overshoot)),
            (a, b),
            (a, wing(a, 1.0, 1.0)),
            (a, wing(a, 1.0, -1.0)),
            (b, wing(b, -1.0, 1.0)),
            (b, wing(b, -1.0, -1.0)),
        ];
        let middle = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        let label_at = shift(middle, normal, side * self.text_height * LABEL_STANDOFF);
        Some(DimensionDrawing {
            strokes,
            label_at,
            label_rotation_deg: upright(along.y.atan2(along.x).to_degrees()),
            value_mm: length,
        })
    }
}

/// `degrees` turned half a turn when it would read upside down, into
/// `-90 < degrees <= 90`: text running straight down reads bottom to top.
fn upright(degrees: f64) -> f64 {
    if degrees > 90.0 {
        degrees - 180.0
    } else if degrees <= -90.0 {
        degrees + 180.0
    } else {
        degrees
    }
}

#[cfg(test)]
#[path = "pcb_dimension_tests.rs"]
mod tests;
