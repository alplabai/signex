//! A linear board dimension (#2373): the distance between two points, drawn
//! as a dimension line standing off them, two extension lines, an arrow at
//! each end and the measured value. Its look is a `DimensionStyle` (#2868).
//! The board keeps them in `PcbBoard::free` (see docs/PCB_TOOLS.md,
//! "Dimension").

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::format::{parse_pcb_layer, pcb_layer_token};
use crate::pcb::{PCB_DEFAULT_GRAPHIC_WIDTH_MM, PCB_DEFAULT_TEXT_SIZE_MM};
use crate::pcb_dimension_style::{
    ArrowStyle, DimensionDefaults, DimensionStyle, DimensionTextPosition,
};
use crate::pcb_layer::{PcbLayer, TechnicalLayer};
use crate::schematic::Point;

/// Decimals of the value a new dimension shows.
pub const DIMENSION_DEFAULT_PRECISION: u8 = 2;

/// How many of `DimensionDrawing::strokes` are extension lines, first.
pub const EXTENSION_STROKES: usize = 2;

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
    #[serde(default)]
    pub style: DimensionStyle,
    /// The value's centre when the text position is Manual, in mm from the
    /// middle of the dimension line: `x` along `start` to `end`, `y` along
    /// the quarter turn `offset` is measured on. It moves and turns with
    /// the dimension.
    #[serde(default = "middle_of_the_line")]
    pub text_at: Point,
}

/// The default `LinearDimension::text_at`.
fn middle_of_the_line() -> Point {
    Point::new(0.0, 0.0)
}

/// The strokes and the value of a dimension, in board mm.
#[derive(Debug, Clone, PartialEq)]
pub struct DimensionDrawing {
    /// The two extension lines (drawn at the extension width), then the
    /// dimension line, in two pieces when the value is inline, then the
    /// sides of open arrows (at the line width).
    pub strokes: Vec<(Point, Point)>,
    /// Filled arrowheads: the tip, then the two corners of the open end.
    pub filled_arrows: Vec<[Point; 3]>,
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
    /// the Dimensions layer with the PCB default line width and text height
    /// and the look dimensions had before their settings.
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
            style: DimensionStyle::default(),
            text_at: Point::new(0.0, 0.0),
        }
    }

    /// This dimension with the line width, text height, decimals and look
    /// of `defaults`. A Manual value starts where Above puts it.
    pub fn with_defaults(mut self, defaults: &DimensionDefaults) -> Self {
        self.line_width = defaults.line_width;
        self.text_height = defaults.text_height;
        self.precision = defaults.precision;
        self.style = defaults.style.clone();
        self.text_at = self.above_text_at();
        self
    }

    /// The typed layer, or `None` for a name this build does not know.
    pub fn pcb_layer(&self) -> Option<PcbLayer> {
        parse_pcb_layer(&self.layer)
    }

    /// The measured distance in mm.
    pub fn value_mm(&self) -> f64 {
        (self.end.x - self.start.x).hypot(self.end.y - self.start.y)
    }

    /// The value as drawn: prefix, the distance to `precision` decimals in
    /// the style's unit, the unit and the suffix.
    pub fn value_text(&self) -> String {
        self.style.value_text(self.value_mm(), self.precision)
    }

    /// How wide the stroke `index` of `DimensionDrawing::strokes` is drawn.
    pub fn stroke_width(&self, index: usize) -> f64 {
        if index < EXTENSION_STROKES {
            self.style.extension_width
        } else {
            self.line_width
        }
    }

    /// The side of `start`–`end` the dimension line stands on: 1 or -1.
    fn side(&self) -> f64 {
        if self.offset < 0.0 { -1.0 } else { 1.0 }
    }

    /// `text_at` for the value where Above puts it.
    pub fn above_text_at(&self) -> Point {
        let across = self.style.text_gap + self.text_height / 2.0;
        Point::new(0.0, self.side() * across)
    }

    /// The length of the value's text along its line.
    pub fn label_length(&self) -> f64 {
        self.style.text_width * self.value_text().chars().count().max(1) as f64
    }

    /// What the dimension draws; `None` when its points are the same, a
    /// number in it is not finite or its text height is not above zero.
    pub fn drawing(&self) -> Option<DimensionDrawing> {
        let length = self.value_mm();
        let style = &self.style;
        let numbers = [
            length,
            self.offset,
            self.text_height,
            self.text_at.x,
            self.text_at.y,
        ];
        let finite = numbers
            .iter()
            .chain(style.lengths().iter())
            .all(|v| v.is_finite());
        if !finite || length == 0.0 || self.text_height <= 0.0 {
            return None;
        }
        let along = Point::new(
            (self.end.x - self.start.x) / length,
            (self.end.y - self.start.y) / length,
        );
        let normal = Point::new(-along.y, along.x);
        let side = self.side();
        let shift = |p: Point, by: Point, k: f64| Point::new(p.x + by.x * k, p.y + by.y * k);

        let a = shift(self.start, normal, self.offset);
        let b = shift(self.end, normal, self.offset);
        let overshoot = side * style.extension_overshoot;
        let from = side * style.extension_offset;
        let mut strokes = vec![
            (shift(self.start, normal, from), shift(a, normal, overshoot)),
            (shift(self.end, normal, from), shift(b, normal, overshoot)),
        ];
        let middle = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        if style.text_position == DimensionTextPosition::Inline {
            // The line breaks around the value; none is left when the value
            // does not fit between the arrows.
            let reach = length / 2.0 - (self.label_length() / 2.0 + style.text_gap);
            if reach > 0.0 {
                strokes.push((a, shift(a, along, reach)));
                strokes.push((shift(b, along, -reach), b));
            }
        } else {
            strokes.push((a, b));
        }
        let half_width = style.arrow_width / 2.0;
        let wing = |tip: Point, inward: f64, k: f64| {
            shift(
                shift(tip, along, inward * style.arrow_length),
                normal,
                k * half_width,
            )
        };
        let mut filled_arrows = Vec::new();
        for (tip, inward) in [(a, 1.0), (b, -1.0)] {
            match style.arrow_style {
                ArrowStyle::Open => {
                    strokes.push((tip, wing(tip, inward, 1.0)));
                    strokes.push((tip, wing(tip, inward, -1.0)));
                }
                ArrowStyle::Filled => {
                    filled_arrows.push([tip, wing(tip, inward, 1.0), wing(tip, inward, -1.0)]);
                }
                ArrowStyle::None => {}
            }
        }
        let text_at = match style.text_position {
            DimensionTextPosition::Above => self.above_text_at(),
            DimensionTextPosition::Inline => Point::new(0.0, 0.0),
            DimensionTextPosition::Manual => self.text_at,
        };
        let label_at = shift(shift(middle, along, text_at.x), normal, text_at.y);
        Some(DimensionDrawing {
            strokes,
            filled_arrows,
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
