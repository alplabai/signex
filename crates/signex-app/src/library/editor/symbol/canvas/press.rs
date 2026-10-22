//! What a Select-tool press would grab, in one place.
//!
//! The press handler and the hover cursor both ask this, so the cursor a
//! user sees over a spot always matches what a click there would do.

use super::*;

/// The thing a Select-tool press at a point would grab.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::library::editor::symbol::canvas) enum PressTarget {
    /// A pin, by its tip or line, or (lowest priority) its name/number label.
    Pin(usize),
    /// A resize handle of an already-selected graphic.
    Handle(usize, state::GraphicHandle),
    /// A graphic body.
    Graphic(usize),
}

impl SymbolCanvas<'_> {
    /// Priority: a pin by its tip or line, then a resize handle of a
    /// selected graphic (the only handles drawn), then a graphic body, and
    /// last a pin by its name/number label. The label comes last because
    /// labels sit inside the body: ranking them first would let a pin's
    /// text box swallow every press in the body next to it. Every step
    /// honours the selection filter. The pin's line zone is narrow (see
    /// [`state::PIN_LINE_GRAB_MM`]), so a graphic that touches a pin's
    /// base stays clickable.
    pub(in crate::library::editor::symbol::canvas) fn press_target(
        &self,
        x: f64,
        y: f64,
    ) -> Option<PressTarget> {
        let tol_mm = self.pick_tolerance_mm();
        if let Some(pin) = state::hit_test_pin(
            self.symbol,
            x,
            y,
            tol_mm,
            self.active_part,
            &self.selection_filter,
        ) {
            return Some(PressTarget::Pin(pin));
        }
        if let Some((idx, handle)) = state::hit_test_graphic_handle(
            self.symbol,
            x,
            y,
            tol_mm,
            self.active_part,
            &self.selected,
            &self.selection_filter,
        ) {
            return Some(PressTarget::Handle(idx, handle));
        }
        state::hit_test_graphic(self.symbol, x, y, self.active_part, &self.selection_filter)
            .map(PressTarget::Graphic)
            .or_else(|| self.pin_hit_by_label(x, y).map(PressTarget::Pin))
    }
}
