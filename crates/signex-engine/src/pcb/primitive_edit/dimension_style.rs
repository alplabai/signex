//! The look fields of a dimension (#2868): arrows, text, extension lines
//! and number format, each set as one field of a `DimensionEdit`.

use signex_types::pcb::Point;
use signex_types::pcb_dimension::LinearDimension;
use signex_types::pcb_dimension_style::{
    ArrowStyle, BoardFont, DimensionTextPosition, DimensionUnits,
};

use super::{PrimitiveEditRefusal, finite};

/// One look field of the selected dimensions, set to an absolute value.
/// Lengths are mm.
#[derive(Debug, Clone, PartialEq)]
pub enum DimensionStyleEdit {
    ArrowStyle(ArrowStyle),
    ArrowLength(f64),
    ArrowWidth(f64),
    Font(BoardFont),
    Bold(bool),
    Italic(bool),
    TextWidth(f64),
    TextPosition(DimensionTextPosition),
    TextGap(f64),
    /// A Manual value's point from the middle of the line, along it.
    TextAlong(f64),
    /// A Manual value's point from the line, across it.
    TextAcross(f64),
    ExtensionOffset(f64),
    ExtensionOvershoot(f64),
    ExtensionWidth(f64),
    Unit(DimensionUnits),
    ShowUnit(bool),
    Prefix(String),
    Suffix(String),
    TrailingZeros(bool),
}

/// A length that must be finite and not below zero.
fn length(value: f64) -> Result<f64, PrimitiveEditRefusal> {
    match finite(value)? {
        v if v < 0.0 => Err(PrimitiveEditRefusal::Negative),
        v => Ok(v),
    }
}

/// A length that must be finite and above zero.
fn positive(value: f64) -> Result<f64, PrimitiveEditRefusal> {
    match finite(value)? {
        v if v <= 0.0 => Err(PrimitiveEditRefusal::NotPositive),
        v => Ok(v),
    }
}

/// `edit` applied to `dimension`. Picking Manual starts the value where it
/// is drawn, so it does not jump.
pub fn apply_style(
    dimension: &mut LinearDimension,
    edit: &DimensionStyleEdit,
) -> Result<(), PrimitiveEditRefusal> {
    use DimensionStyleEdit as E;
    let style = &mut dimension.style;
    match edit {
        E::ArrowStyle(v) => style.arrow_style = *v,
        E::ArrowLength(v) => style.arrow_length = length(*v)?,
        E::ArrowWidth(v) => style.arrow_width = length(*v)?,
        E::Font(v) => style.font = *v,
        E::Bold(v) => style.bold = *v,
        E::Italic(v) => style.italic = *v,
        E::TextWidth(v) => style.text_width = positive(*v)?,
        E::TextPosition(DimensionTextPosition::Manual)
            if style.text_position != DimensionTextPosition::Manual =>
        {
            let shown = match style.text_position {
                DimensionTextPosition::Inline => Point::new(0.0, 0.0),
                _ => dimension.above_text_at(),
            };
            dimension.text_at = shown;
            dimension.style.text_position = DimensionTextPosition::Manual;
        }
        E::TextPosition(v) => style.text_position = *v,
        E::TextGap(v) => style.text_gap = length(*v)?,
        E::TextAlong(v) => dimension.text_at.x = finite(*v)?,
        E::TextAcross(v) => dimension.text_at.y = finite(*v)?,
        E::ExtensionOffset(v) => style.extension_offset = length(*v)?,
        E::ExtensionOvershoot(v) => style.extension_overshoot = length(*v)?,
        E::ExtensionWidth(v) => style.extension_width = length(*v)?,
        E::Unit(v) => style.unit = *v,
        E::ShowUnit(v) => style.show_unit = *v,
        E::Prefix(v) => style.prefix = v.clone(),
        E::Suffix(v) => style.suffix = v.clone(),
        E::TrailingZeros(v) => style.trailing_zeros = *v,
    }
    Ok(())
}
