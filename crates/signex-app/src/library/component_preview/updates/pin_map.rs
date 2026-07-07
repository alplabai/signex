//! Pin-map override edits for a Component Preview row.
//!
//! The pin map defaults to a 1:1 symbol-pin → footprint-pad mapping by
//! number; this module owns the inline override editor that lets a user
//! pin specific pads, plus the toolbar actions that clear or auto-match
//! the overrides.

use crate::library::ComponentPreviewState;

/// Clear every override, reverting to the default number-based match,
/// and collapse the inline override editor.
pub(super) fn clear_overrides(state: &mut ComponentPreviewState) {
    state.row.pin_map_overrides.clear();
    state.pin_map_state.expanded_row = None;
    state.pin_map_state.override_buf.clear();
    state.dirty = true;
}

/// Auto-match by pin name. Stubbed until the name-based heuristic ships;
/// emits a tracing warning and leaves the overrides untouched.
pub(super) fn warn_auto_match_by_name() {
    tracing::warn!(
        target: "signex::library",
        "Pin Map: Auto-Match by Name is stubbed; awaiting heuristic implementation"
    );
}

/// Expand the inline override editor for `pin`, seeding the edit buffer
/// with that pin's current pad number (empty when unset).
pub(super) fn open_override_edit(state: &mut ComponentPreviewState, pin: String) {
    let seed = state
        .row
        .pin_map_overrides
        .iter()
        .find(|o| o.symbol_pin_number == pin)
        .map(|o| o.footprint_pad_number.clone())
        .unwrap_or_default();
    state.pin_map_state.expanded_row = Some(pin);
    state.pin_map_state.override_buf = seed;
}

/// Live-update the override edit buffer, but only while `pin`'s row is
/// the one currently expanded.
pub(super) fn set_override_buf(state: &mut ComponentPreviewState, pin: String, value: String) {
    if state.pin_map_state.expanded_row.as_deref() == Some(pin.as_str()) {
        state.pin_map_state.override_buf = value;
    }
}

/// Commit an override for `pin`: an empty pad clears it, otherwise the
/// existing entry is updated in place or a new one is pushed. Collapses
/// the inline editor afterwards.
pub(super) fn add_override(state: &mut ComponentPreviewState, pin: String, pad: String) {
    let trimmed = pad.trim();
    if trimmed.is_empty() {
        state
            .row
            .pin_map_overrides
            .retain(|o| o.symbol_pin_number != pin);
    } else if let Some(existing) = state
        .row
        .pin_map_overrides
        .iter_mut()
        .find(|o| o.symbol_pin_number == pin)
    {
        existing.footprint_pad_number = trimmed.to_string();
    } else {
        state
            .row
            .pin_map_overrides
            .push(signex_library::PinPadOverride::new(pin, trimmed));
    }
    state.pin_map_state.expanded_row = None;
    state.pin_map_state.override_buf.clear();
    state.dirty = true;
}

/// Discard the edit buffer and collapse the inline override editor
/// without touching the stored overrides.
pub(super) fn cancel_override_edit(state: &mut ComponentPreviewState) {
    state.pin_map_state.expanded_row = None;
    state.pin_map_state.override_buf.clear();
}

/// Remove `pin`'s override entry and collapse the inline editor.
pub(super) fn remove_override(state: &mut ComponentPreviewState, pin: String) {
    state
        .row
        .pin_map_overrides
        .retain(|o| o.symbol_pin_number != pin);
    state.pin_map_state.expanded_row = None;
    state.pin_map_state.override_buf.clear();
    state.dirty = true;
}
