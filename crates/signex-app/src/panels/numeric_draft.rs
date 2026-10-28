//! Typing drafts for plain numeric text inputs.
//!
//! A numeric box that re-formats its stored value on every keystroke
//! snaps `-` and `1.` away before the user can finish them. While a box
//! is being typed in, the raw text is kept as a draft and shown as-is;
//! every keystroke that already parses as a finite number is applied to
//! the model right away, and Enter drops the draft so the box shows the
//! stored value again (which reverts text that never parsed, and shows a
//! clamped or wrapped value, e.g. a pin length of `0` stored as `0.1`).
//!
//! A draft belongs to an owner: the tab, the symbol and the exact field
//! (including which pin or graphic is selected) it was typed into. It is
//! shown only while that owner is unchanged and the stored value is still
//! the one the model held right after the draft's own last keystroke. A
//! tab or selection switch, undo/redo, a canvas drag, or any other edit
//! that moves the stored value from outside the box ends the draft. The
//! setter's clamp does not: the stored value read back after the keystroke
//! is what the draft is compared to, so `0` -> `0.1` keeps showing `0`.
//!
//! iced 0.14's `text_input` has no blur event, so there is no focus-lost
//! hook. A draft ends on Enter, on typing in another numeric box (there is
//! one draft at a time), and on every owner or value change above; the
//! only thing left is clicking away to a control that changes nothing, in
//! which case the box keeps the typed text until the next of those.

use std::path::PathBuf;

use iced::Length;
use iced::widget::{TextInput, text_input};

use super::{
    GraphicFieldId, GraphicKindSummary, GraphicSummary, PanelMsg, PrePlacementData,
    SymbolEditorPanelContext, SymbolEditorSelection,
};

/// Which numeric box a draft belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericFieldKey {
    /// Pre-placement shape Width (mm).
    ShapeWidth,
    /// Symbol editor pin X (mm).
    PinX(usize),
    /// Symbol editor pin Y (mm).
    PinY(usize),
    /// Symbol editor pin length (mm).
    PinLength(usize),
    /// Symbol editor graphic scalar.
    Graphic { idx: usize, field: GraphicFieldId },
}

impl NumericFieldKey {
    /// The message that stores `value` into this field.
    pub fn committing(self, value: f64) -> PanelMsg {
        match self {
            Self::ShapeWidth => PanelMsg::SetPrePlacementShapeWidth(value),
            Self::PinX(pin_idx) => PanelMsg::SymEditorSetPinX { pin_idx, value },
            Self::PinY(pin_idx) => PanelMsg::SymEditorSetPinY { pin_idx, value },
            Self::PinLength(pin_idx) => PanelMsg::SymEditorSetPinLength { pin_idx, value },
            Self::Graphic { idx, field } => {
                PanelMsg::SymEditorSetGraphicField { idx, field, value }
            }
        }
    }
}

/// What a draft is attached to besides its field: the tab and, in the
/// symbol editor, the symbol shown in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericOwner {
    pub tab: Option<PathBuf>,
    pub symbol: Option<uuid::Uuid>,
}

/// The stored value of `key` and who owns the box, or `None` when the box
/// is not on screen (another selection, another tab, no placement).
pub fn field_state(
    key: NumericFieldKey,
    active_tab: Option<&PathBuf>,
    symbol: Option<&SymbolEditorPanelContext>,
    pre_placement: Option<&PrePlacementData>,
) -> Option<(NumericOwner, f64)> {
    if key == NumericFieldKey::ShapeWidth {
        let value = pre_placement?.shape_width_mm;
        let owner = NumericOwner {
            tab: active_tab.cloned(),
            symbol: None,
        };
        return Some((owner, value));
    }
    let sym = symbol?;
    let value = match (key, &sym.selected) {
        (NumericFieldKey::PinX(i), SymbolEditorSelection::Pin(p)) if p.idx == i => p.position[0],
        (NumericFieldKey::PinY(i), SymbolEditorSelection::Pin(p)) if p.idx == i => p.position[1],
        (NumericFieldKey::PinLength(i), SymbolEditorSelection::Pin(p)) if p.idx == i => p.length,
        (NumericFieldKey::Graphic { idx, field }, SymbolEditorSelection::Graphic(g))
            if g.idx == idx =>
        {
            graphic_scalar(g, field)?
        }
        _ => return None,
    };
    let owner = NumericOwner {
        tab: Some(sym.path.clone()),
        symbol: Some(sym.symbol_uuid),
    };
    Some((owner, value))
}

/// The scalar a graphic's numeric box shows, `None` when the field does
/// not apply to that kind of graphic.
fn graphic_scalar(g: &GraphicSummary, field: GraphicFieldId) -> Option<f64> {
    use GraphicFieldId as F;
    use GraphicKindSummary as K;
    match (&g.kind, field) {
        (_, F::StrokeWidth) => Some(g.stroke_width),
        (K::Rectangle { from, .. } | K::Line { from, .. }, F::FromX) => Some(from[0]),
        (K::Rectangle { from, .. } | K::Line { from, .. }, F::FromY) => Some(from[1]),
        (K::Rectangle { to, .. } | K::Line { to, .. }, F::ToX) => Some(to[0]),
        (K::Rectangle { to, .. } | K::Line { to, .. }, F::ToY) => Some(to[1]),
        (K::Circle { center, .. } | K::Arc { center, .. }, F::CenterX) => Some(center[0]),
        (K::Circle { center, .. } | K::Arc { center, .. }, F::CenterY) => Some(center[1]),
        (K::Circle { radius, .. } | K::Arc { radius, .. }, F::Radius) => Some(*radius),
        (K::Arc { start_deg, .. }, F::StartDeg) => Some(*start_deg),
        (K::Arc { end_deg, .. }, F::EndDeg) => Some(*end_deg),
        (K::Text { position, .. }, F::PositionX) => Some(position[0]),
        (K::Text { position, .. }, F::PositionY) => Some(position[1]),
        (K::Text { size, .. }, F::TextSize) => Some(*size),
        _ => None,
    }
}

/// The raw text of the numeric box being typed in. Only one box has
/// focus at a time, so one draft is enough.
#[derive(Debug, Clone, PartialEq)]
pub struct NumericDraft {
    pub key: NumericFieldKey,
    pub owner: NumericOwner,
    pub text: String,
    /// The stored value read back right after this keystroke was applied
    /// (post-clamp). A different stored value later means something other
    /// than the box moved it.
    pub applied: f64,
}

impl NumericDraft {
    /// Whether the box is still the one the draft was typed into and
    /// nothing else has changed its value since.
    pub fn is_live(&self, now: Option<(NumericOwner, f64)>) -> bool {
        matches!(now, Some((owner, stored)) if owner == self.owner && stored == self.applied)
    }
}

/// What the box shows: the draft when it belongs to `key`, else `value`
/// to three decimals. Drafts that are no longer live never reach a view.
pub fn shown_text(draft: Option<&NumericDraft>, key: NumericFieldKey, value: f64) -> String {
    match draft {
        Some(d) if d.key == key => d.text.clone(),
        _ => format!("{value:.3}"),
    }
}

/// A numeric text input wired to the draft messages.
pub fn numeric_input<'a>(
    placeholder: &str,
    key: NumericFieldKey,
    value: f64,
    draft: Option<&NumericDraft>,
) -> TextInput<'a, PanelMsg> {
    text_input(placeholder, &shown_text(draft, key, value))
        .on_input(move |s| PanelMsg::NumericFieldTyped(key, s))
        .on_submit(PanelMsg::NumericFieldSubmitted(key))
        .width(Length::FillPortion(3))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(tab: &str) -> NumericOwner {
        NumericOwner {
            tab: Some(PathBuf::from(tab)),
            symbol: None,
        }
    }

    fn draft(key: NumericFieldKey, text: &str, applied: f64) -> NumericDraft {
        NumericDraft {
            key,
            owner: owner("a"),
            text: text.to_string(),
            applied,
        }
    }

    #[test]
    fn draft_text_is_shown_for_its_own_field_only() {
        let d = draft(NumericFieldKey::PinX(0), "-", 2.54);
        assert_eq!(shown_text(Some(&d), NumericFieldKey::PinX(0), 2.54), "-");
        assert_eq!(
            shown_text(Some(&d), NumericFieldKey::PinX(1), 2.54),
            "2.540"
        );
        assert_eq!(shown_text(None, NumericFieldKey::PinX(0), 2.54), "2.540");
    }

    #[test]
    fn a_draft_is_live_only_for_its_owner_and_applied_value() {
        let d = draft(NumericFieldKey::PinLength(0), "0", 0.1);
        assert!(
            d.is_live(Some((owner("a"), 0.1))),
            "a clamp keeps the draft"
        );
        assert!(!d.is_live(Some((owner("b"), 0.1))), "another tab");
        assert!(!d.is_live(Some((owner("a"), 3.0))), "moved from outside");
        assert!(!d.is_live(None), "box no longer on screen");
    }
}
