//! Symbol render — graphics under the symbol's transform, then pins,
//! then visible fields (reference / value / user fields).
//!
//! Symbol body comes from `LibSymbol::graphics`; pins are delegated to
//! [`super::pin::draw_pin`]; field text uses the rotation + justify
//! produced by [`super::field_style::field_effective_style`].
//!
//! Filled in by Wave 3 sub-agent 11.

use iced::widget::canvas::Frame;
use signex_types::schematic::{LibSymbol, Symbol};

use super::RenderContext;

/// Render a single placed symbol into the content layer's frame.
pub fn draw_symbol(
    _frame: &mut Frame,
    _symbol: &Symbol,
    _lib: &LibSymbol,
    _ctx: &RenderContext<'_>,
) {
    todo!("Wave 3 sub-agent 11 fills this in against signex-types::Symbol + LibSymbol")
}

#[cfg(test)]
mod tests {
    // Sub-agent populates: render_smoke (one symbol with a few graphics
    // and pins) + edge cases (rotated symbol, mirrored symbol, hidden
    // field).
}
