//! Pin primitive — base stroke + IEEE-Std-91 decorator.
//!
//! Pins are drawn as part of a parent symbol's render pass. The parent
//! supplies its [`SymbolTransform`](super::SymbolTransform) so this
//! function can fold the library-space pin geometry into world space.
//!
//! Decorator catalog comes from
//! `docs/RENDERING_RULES.md::pin-shape-decorators`, which paraphrases
//! IEEE-Std-91 (Graphic Symbols for Logic Functions, IEEE 1984/2004).
//! Decorator dimensions scale with `pin.length`.
//!
//! Filled in by Wave 2 sub-agent 8.

use iced::widget::canvas::Frame;
use signex_types::schematic::Pin;

use super::{RenderContext, SymbolTransform};

/// Render a pin (base stroke + decorator + name/number text) using the
/// parent symbol's `transform` to project library-space coordinates
/// into world space.
pub fn draw_pin(
    _frame: &mut Frame,
    _pin: &Pin,
    _transform: &SymbolTransform,
    _ctx: &RenderContext<'_>,
) {
    todo!("Wave 2 sub-agent 8 fills this in against signex-types::Pin and the IEEE-Std-91 catalog")
}

#[cfg(test)]
mod tests {
    // Sub-agent populates: render_smoke for the seven PinShapeStyle
    // variants plus rotation / mirror edge cases.
}
