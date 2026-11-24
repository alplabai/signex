//! The look of a linear board dimension (#2868): its arrows, text,
//! extension lines and how its value is written, and the defaults a new
//! dimension starts from. See docs/PCB_TOOLS.md, "Dimension settings".

use serde::{Deserialize, Serialize};

use crate::pcb::{PCB_DEFAULT_GRAPHIC_WIDTH_MM, PCB_DEFAULT_TEXT_SIZE_MM};
use crate::pcb_dimension::DIMENSION_DEFAULT_PRECISION;
pub use crate::schematic::DimensionUnits;

/// The arrowheads at the two ends of the dimension line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArrowStyle {
    /// A filled triangle.
    Filled,
    /// Two strokes; the look of a dimension from before these settings.
    #[default]
    Open,
    /// No arrowheads.
    None,
}

impl ArrowStyle {
    pub const ALL: [Self; 3] = [Self::Filled, Self::Open, Self::None];
}

/// Where the value sits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DimensionTextPosition {
    /// Beside the dimension line, on the side it stands off, `text_gap`
    /// from it.
    #[default]
    Above,
    /// On the dimension line, which breaks `text_gap` before and after it.
    Inline,
    /// Where `LinearDimension::text_at` puts it.
    Manual,
}

impl DimensionTextPosition {
    pub const ALL: [Self; 3] = [Self::Above, Self::Inline, Self::Manual];
}

/// The typeface of a board text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardFont {
    /// The monospace face board text is drawn in.
    #[default]
    Mono,
    /// The proportional interface face.
    Sans,
}

impl BoardFont {
    pub const ALL: [Self; 2] = [Self::Mono, Self::Sans];
}

/// How far an extension line ran past the dimension line, in text heights,
/// before these settings.
const LEGACY_OVERSHOOT: f64 = 0.5;
/// How wide an arrow was, in text heights.
const LEGACY_ARROW_WIDTH: f64 = 0.6;
/// The gap between the dimension line and the value, in text heights.
const LEGACY_TEXT_GAP: f64 = 0.5;
/// The advance of one glyph of board text, in text heights: the canvas
/// draws it in a monospace face whose glyphs are about this wide.
pub const GLYPH_ADVANCE: f64 = 0.6;

/// The arrows, text, extension lines and number format of a dimension.
/// Lengths are mm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DimensionStyle {
    pub arrow_style: ArrowStyle,
    /// From the tip back along the dimension line.
    pub arrow_length: f64,
    /// Across the dimension line, at the arrow's open end.
    pub arrow_width: f64,
    pub font: BoardFont,
    pub bold: bool,
    pub italic: bool,
    /// The width of one character.
    pub text_width: f64,
    pub text_position: DimensionTextPosition,
    /// Between the value and the dimension line.
    pub text_gap: f64,
    /// Between a measured point and the start of its extension line.
    pub extension_offset: f64,
    /// How far an extension line runs past the dimension line.
    pub extension_overshoot: f64,
    pub extension_width: f64,
    pub unit: DimensionUnits,
    /// The unit's symbol is written after the number.
    pub show_unit: bool,
    pub prefix: String,
    pub suffix: String,
    /// The number keeps zeros after its last significant decimal.
    pub trailing_zeros: bool,
}

impl DimensionStyle {
    /// The look every dimension had before these settings, which scaled
    /// with its text height: open arrows one text height long, the value
    /// half a height off the line, extension lines from the measured points
    /// half a height past the line at the line width.
    pub fn for_text_height(text_height: f64, line_width: f64) -> Self {
        Self {
            arrow_style: ArrowStyle::Open,
            arrow_length: text_height,
            arrow_width: text_height * LEGACY_ARROW_WIDTH,
            font: BoardFont::Mono,
            bold: false,
            italic: false,
            text_width: text_height * GLYPH_ADVANCE,
            text_position: DimensionTextPosition::Above,
            text_gap: text_height * LEGACY_TEXT_GAP,
            extension_offset: 0.0,
            extension_overshoot: text_height * LEGACY_OVERSHOOT,
            extension_width: line_width,
            unit: DimensionUnits::Mm,
            show_unit: true,
            prefix: String::new(),
            suffix: String::new(),
            trailing_zeros: true,
        }
    }

    /// The lengths, for the finite check of a drawing.
    pub(crate) fn lengths(&self) -> [f64; 7] {
        [
            self.arrow_length,
            self.arrow_width,
            self.text_width,
            self.text_gap,
            self.extension_offset,
            self.extension_overshoot,
            self.extension_width,
        ]
    }

    /// `mm` written in this style: prefix, the number to `precision`
    /// decimals in the unit, the unit's symbol and the suffix.
    pub fn value_text(&self, mm: f64, precision: u8) -> String {
        let mut number = format!("{:.*}", usize::from(precision), self.unit.from_mm(mm));
        if !self.trailing_zeros && number.contains('.') {
            number.truncate(number.trim_end_matches('0').trim_end_matches('.').len());
        }
        let unit = if self.show_unit {
            self.unit.suffix()
        } else {
            ""
        };
        format!("{}{number}{unit}{}", self.prefix, self.suffix)
    }
}

impl Default for DimensionStyle {
    fn default() -> Self {
        Self::for_text_height(PCB_DEFAULT_TEXT_SIZE_MM, PCB_DEFAULT_GRAPHIC_WIDTH_MM)
    }
}

/// What a new dimension starts from (Preferences ▸ PCB Defaults); its own
/// Properties override each value once it is placed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DimensionDefaults {
    pub line_width: f64,
    pub text_height: f64,
    pub precision: u8,
    pub style: DimensionStyle,
}

impl DimensionDefaults {
    /// Whether these are values a dimension's Properties could have set:
    /// every number finite, the line width, text height and text width
    /// above zero, no other length below zero, at most
    /// `DIMENSION_MAX_PRECISION` decimals.
    pub fn is_sound(&self) -> bool {
        let s = &self.style;
        let lengths = [
            s.arrow_length,
            s.arrow_width,
            s.text_gap,
            s.extension_offset,
            s.extension_overshoot,
            s.extension_width,
        ];
        let positive = [self.line_width, self.text_height, s.text_width];
        self.precision <= crate::pcb_dimension::DIMENSION_MAX_PRECISION
            && positive.iter().all(|v| v.is_finite() && *v > 0.0)
            && lengths.iter().all(|v| v.is_finite() && *v >= 0.0)
    }
}

impl Default for DimensionDefaults {
    fn default() -> Self {
        Self {
            line_width: PCB_DEFAULT_GRAPHIC_WIDTH_MM,
            text_height: PCB_DEFAULT_TEXT_SIZE_MM,
            precision: DIMENSION_DEFAULT_PRECISION,
            style: DimensionStyle::default(),
        }
    }
}

#[cfg(test)]
#[path = "pcb_dimension_style_tests.rs"]
mod tests;
