//! Measurement edits of a Component Preview row: a number field, an SI
//! prefix picker and a unit, stored as the canonical `{base, unit token}`.
//!
//! The unit comes from the class template's slot where one fixes it,
//! otherwise from the row (custom parameters pick theirs). A typed value
//! that breaks its slot (zero or a negative the slot does not allow) is
//! refused: it is not stored, the reason is kept in
//! `ComponentPreviewState::params_error` for the field to show, and Save
//! is blocked until it is fixed. A value that was already stored stays as
//! it is until it is edited.

use signex_library::{
    ParamKind, ParamSlot, ParamValue, commit_measurement as commit_typed, commit_slot_measurement,
    slot_field_seed, slot_unit,
};
use signex_types::quantity::{MantissaError, SiPrefix, Unit, parse_mantissa};

use crate::library::ComponentPreviewState;
use crate::library::prefix_picker::effective_prefix;
use crate::library::unit_picker::unit_change_note;

/// The measurement slot with a known unit behind `name`'s pending edit.
pub(super) fn measurement_slot(state: &ComponentPreviewState, name: &str) -> Option<ParamSlot> {
    state
        .params_slot
        .get(name)
        .filter(|s| s.kind == ParamKind::Measurement && slot_unit(s).is_some())
        .cloned()
}

/// The unit an edit commits under: the slot's, else the row's own
/// `unit_token` when that is a known unit. `None` keeps the raw text editor.
pub(super) fn editing_unit(slot: Option<&ParamSlot>, unit_token: &str) -> Option<Unit> {
    slot.and_then(slot_unit)
        .or_else(|| Unit::from_alias(unit_token.trim()))
}

/// Live-update the edit buffer for a measurement parameter row. A prefix
/// or unit typed into the number (`4k7`) is refused at once.
pub(super) fn set_measurement_buf(
    state: &mut ComponentPreviewState,
    name: String,
    buf: String,
    slot: Option<ParamSlot>,
) {
    if let Some(slot) = slot {
        state.params_slot.insert(name.clone(), slot);
    }
    match parse_mantissa(&buf) {
        Err(e @ (MantissaError::HasLetters(_) | MantissaError::Comma(_))) => {
            state.params_error.insert(name.clone(), e.to_string());
        }
        _ => {
            state.params_error.remove(&name);
        }
    }
    state.params_edit_buf.insert(name, buf);
    state.note_edit();
}

/// Store the outcome of a typed commit, or keep the reason it was refused.
/// An outcome equal to the stored value changes nothing (not even `dirty`).
fn apply(state: &mut ComponentPreviewState, name: String, result: Result<ParamValue, String>) {
    state.params_note.remove(&name);
    match result {
        Ok(value) => {
            if state.row.parameters.get(&name) != Some(&value) {
                state.row.parameters.insert(name.clone(), value);
                state.mark_dirty();
            }
            // The field reads from the stored value again.
            state.params_edit_buf.remove(&name);
            state.params_prefix.remove(&name);
            state.params_slot.remove(&name);
            state.params_error.remove(&name);
        }
        Err(why) => {
            crate::diagnostics::log_warning(format!("Parameter '{name}' refused: {why}"));
            state.params_error.insert(name, why);
        }
    }
}

/// Commit a measurement parameter from its edit buffer.
///
/// With a known unit the buffer is a plain number in the picked SI prefix
/// (by default the prefix the stored value displays with), stored
/// canonical (`{4700, "ohm"}` for 4.7 + k). A stored value the field
/// still shows as it was is left exactly as it is. A unit string that is
/// not a known unit keeps the raw number and its string.
pub(super) fn commit_measurement(state: &mut ComponentPreviewState, name: String, unit: String) {
    let Some(buf) = state.params_edit_buf.get(&name).cloned() else {
        return;
    };
    let slot = measurement_slot(state, &name);
    let Some(known) = editing_unit(slot.as_ref(), &unit) else {
        if let Some(value) = crate::numeric_field::parse_field_f64(&buf) {
            state.params_error.remove(&name);
            state
                .row
                .parameters
                .insert(name, ParamValue::Measurement { value, unit });
            state.mark_dirty();
        }
        return;
    };
    let prefix = effective_prefix(&known, state.measurement_prefix(&name));
    let existing = state.row.parameters.get(&name);
    let result = match &slot {
        Some(slot) => commit_slot_measurement(existing, &buf, prefix, slot),
        None => commit_typed(existing, &buf, prefix, &known).map_err(|e| e.to_string()),
    };
    apply(state, name, result);
}

/// The user picked an SI prefix for a measurement's number: remember it
/// and commit the buffered number (or the stored one) in that prefix.
pub(super) fn pick_prefix(
    state: &mut ComponentPreviewState,
    name: String,
    unit: String,
    prefix: SiPrefix,
    slot: Option<ParamSlot>,
) {
    if let Some(slot) = slot {
        state.params_slot.insert(name.clone(), slot);
    }
    let Some(known) = editing_unit(measurement_slot(state, &name).as_ref(), &unit) else {
        return;
    };
    // Seed the buffer with the number the field shows, so a pick with
    // nothing typed still commits it in the new prefix. A slot's field
    // shows `slot_field_seed` (a plain number or an unknown unit too);
    // a free parameter's shows the stored measurement's number.
    if !state.params_edit_buf.contains_key(&name) {
        let seed =
            state
                .row
                .parameters
                .get(&name)
                .and_then(|v| match measurement_slot(state, &name) {
                    Some(_) => Some(slot_field_seed(v, &known).0),
                    None => v.measurement_parts().map(|p| p.mantissa),
                });
        if let Some(seed) = seed {
            state.params_edit_buf.insert(name.clone(), seed);
            state.note_edit();
        }
    }
    state
        .params_prefix
        .insert(name.clone(), effective_prefix(&known, prefix));
    commit_measurement(state, name, unit);
}

/// The user picked a unit for a custom measurement row (a template slot
/// fixes its own). Like the Edit Component Details modal it keeps the
/// number as shown and its prefix, so 4.7 k stays 4.7 k in the new unit.
/// Picking the unit the row already has changes nothing.
pub(super) fn pick_unit(state: &mut ComponentPreviewState, name: String, unit: String) {
    let Some(new_unit) = Unit::from_alias(unit.trim()) else {
        return;
    };
    let Some(stored) = state.row.parameters.get(&name) else {
        return;
    };
    let ParamValue::Measurement { value, .. } = stored else {
        return;
    };
    let (mantissa, shown) = match (state.params_edit_buf.get(&name), stored.measurement_parts()) {
        (Some(buf), _) => (buf.clone(), state.measurement_prefix(&name)),
        (None, Some(parts)) => (parts.mantissa, parts.prefix),
        (None, None) => (value.to_string(), SiPrefix::One),
    };
    let prefix = effective_prefix(&new_unit, shown);
    let before = stored.as_quantity().map(|q| q.unit().clone());
    let result =
        commit_typed(Some(stored), &mantissa, prefix, &new_unit).map_err(|e| e.to_string());
    let ok = result.is_ok();
    apply(state, name.clone(), result);
    // The number is kept as shown, so a changed unit reinterprets it: say so.
    if ok
        && let Some(note) = state
            .row
            .parameters
            .get(&name)
            .and_then(|v| unit_change_note(&name, before.as_ref(), v))
    {
        state.params_note.insert(name, note);
    }
}
