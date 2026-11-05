//! The one table of schematic primitive defaults (#2026): what a width or
//! size stored as `0` ("unset") draws and exports at.
//!
//! [`SchematicDefaults::BUILT_IN`] holds the values every sheet drew before
//! projects could override them; a project's `.snxprj` overrides any of them
//! (`crate::project::SchematicDefaultOverrides`). Precedence for a wire:
//! explicit item width > net class width > project default > built-in. The
//! canvas, the selection box and every export resolve through these methods.

use super::{
    FillType, SCHEMATIC_DEFAULT_WIRE_STROKE_MM, SCHEMATIC_RENDER_BUS_STROKE_MM,
    SCHEMATIC_RENDER_JUNCTION_MIN_RADIUS_MM, SCHEMATIC_RENDER_MIN_STROKE_MM,
};
use crate::net_class::NetLineStyle;

/// Font new text is placed in when the project names none.
pub const SCHEMATIC_DEFAULT_FONT: &str = "Iosevka";

/// Text size new text is placed at when the project names none (pt).
pub const SCHEMATIC_DEFAULT_TEXT_SIZE_PT: f64 = 10.0;

/// Resolved defaults for unset schematic primitives, all in mm except the
/// text size (pt). Plain data, so the canvas and export share one value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SchematicDefaults {
    pub wire_width_mm: f64,
    pub bus_width_mm: f64,
    /// Width of a symbol graphic stored with width `0`, and so of the pins of
    /// a symbol whose graphics are all unset.
    pub symbol_line_width_mm: f64,
    /// Width of a drawing (line, rectangle, ...) stored with width `0`.
    pub shape_stroke_mm: f64,
    /// Diameter of a junction stored with diameter `0`.
    pub junction_diameter_mm: f64,
    pub text_size_pt: f64,
    /// How a wire whose net class sets no line style is stroked. A wire
    /// stores no line style of its own.
    pub wire_line_style: NetLineStyle,
    /// Fill new shapes are placed with. A placed shape always stores its
    /// fill, so this never reinterprets a stored one.
    pub shape_fill: FillType,
}

/// A width counts as set when it is a finite number above zero.
#[must_use]
pub fn is_set_width(mm: f64) -> bool {
    mm.is_finite() && mm > 0.0
}

impl SchematicDefaults {
    /// Today's values: the defaults of a project that overrides nothing.
    pub const BUILT_IN: Self = Self {
        wire_width_mm: SCHEMATIC_DEFAULT_WIRE_STROKE_MM,
        bus_width_mm: SCHEMATIC_RENDER_BUS_STROKE_MM,
        symbol_line_width_mm: SCHEMATIC_DEFAULT_WIRE_STROKE_MM,
        shape_stroke_mm: SCHEMATIC_RENDER_MIN_STROKE_MM,
        junction_diameter_mm: 2.0 * SCHEMATIC_RENDER_JUNCTION_MIN_RADIUS_MM,
        text_size_pt: SCHEMATIC_DEFAULT_TEXT_SIZE_PT,
        wire_line_style: NetLineStyle::Solid,
        shape_fill: FillType::None,
    };

    /// The whole-point text size placement uses, the one conversion every
    /// placement path shares (the pre-placement form holds whole points);
    /// never below 1 pt.
    #[must_use]
    pub fn placed_text_pt(&self) -> u32 {
        self.text_size_pt.round().max(1.0) as u32
    }

    /// [`Self::placed_text_pt`] in mm, the size stored on placed text.
    #[must_use]
    pub fn placed_text_mm(&self) -> f64 {
        f64::from(self.placed_text_pt()) * super::SCHEMATIC_PT_TO_MM
    }

    /// The line style a wire is stroked with: its class's when the class
    /// sets one (an explicit Solid included), else the project default.
    #[must_use]
    pub fn wire_line(&self, class_line: Option<NetLineStyle>) -> NetLineStyle {
        class_line.unwrap_or(self.wire_line_style)
    }

    /// `own` when it is set, else `default`; never under the shared floor.
    fn resolve(own: f64, default: f64) -> f64 {
        let width = if is_set_width(own) { own } else { default };
        width.max(SCHEMATIC_RENDER_MIN_STROKE_MM)
    }

    /// The width a wire draws and exports at: its own width, else the net
    /// class width, else the project default, else the built-in.
    #[must_use]
    pub fn wire_mm(&self, own_mm: f64, class_mm: Option<f64>) -> f64 {
        let fallback = class_mm
            .filter(|w| is_set_width(*w))
            .unwrap_or(self.wire_width_mm);
        Self::resolve(own_mm, fallback)
    }

    /// The width a bus draws and exports at. A bus stores no width of its own
    /// and has no net class, so it is the project default, else the built-in.
    #[must_use]
    pub fn bus_mm(&self) -> f64 {
        Self::resolve(self.bus_width_mm, SCHEMATIC_RENDER_BUS_STROKE_MM)
    }

    /// The width a symbol graphic or pin stored as `width_mm` draws at.
    #[must_use]
    pub fn symbol_line_mm(&self, width_mm: f64) -> f64 {
        Self::resolve(width_mm, self.symbol_line_width_mm)
    }

    /// The width a drawing (shape) stored as `width_mm` draws at.
    #[must_use]
    pub fn shape_stroke_mm(&self, width_mm: f64) -> f64 {
        Self::resolve(width_mm, self.shape_stroke_mm)
    }

    /// The radius a junction stored with `diameter_mm` draws and exports at,
    /// never under [`SCHEMATIC_RENDER_JUNCTION_MIN_RADIUS_MM`].
    #[must_use]
    pub fn junction_radius_mm(&self, diameter_mm: f64) -> f64 {
        let diameter = if is_set_width(diameter_mm) {
            diameter_mm
        } else {
            self.junction_diameter_mm
        };
        (diameter * 0.5).max(SCHEMATIC_RENDER_JUNCTION_MIN_RADIUS_MM)
    }

    /// The symbol's library stroke width, which its pins draw at: the most
    /// common effective width among its stroked graphics (the wider one on a
    /// tie), or the symbol line default when it has none.
    #[must_use]
    pub fn symbol_stroke_mm(&self, graphic_widths_mm: impl IntoIterator<Item = f64>) -> f64 {
        let mut counts: Vec<(f64, usize)> = Vec::new();
        for width in graphic_widths_mm
            .into_iter()
            .map(|w| self.symbol_line_mm(w))
        {
            match counts.iter_mut().find(|(w, _)| *w == width) {
                Some((_, n)) => *n += 1,
                None => counts.push((width, 1)),
            }
        }
        counts
            .into_iter()
            .max_by(|a, b| a.1.cmp(&b.1).then(a.0.total_cmp(&b.0)))
            .map_or(self.symbol_line_mm(0.0), |(width, _)| width)
    }
}

impl Default for SchematicDefaults {
    fn default() -> Self {
        Self::BUILT_IN
    }
}

#[cfg(test)]
#[path = "defaults_tests.rs"]
mod tests;
