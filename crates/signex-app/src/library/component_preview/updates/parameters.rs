//! Parameter edits for a Component Preview row.
//!
//! Text and boolean parameters commit immediately; numeric and
//! measurement parameters are edited through a per-row buffer
//! (`params_edit_buf`) and committed on demand so a half-typed value is
//! never parsed.

use crate::library::ComponentPreviewState;
use crate::library::messages::ParamKindMsg;

/// Set a text parameter's value directly. Ignores an empty name.
pub(super) fn set_text(state: &mut ComponentPreviewState, name: String, value: String) {
    if !name.is_empty() {
        state
            .row
            .parameters
            .insert(name, signex_library::ParamValue::Text(value));
        state.dirty = true;
    }
}

/// Live-update the edit buffer for a numeric parameter row.
pub(super) fn set_number_buf(state: &mut ComponentPreviewState, name: String, buf: String) {
    state.params_edit_buf.insert(name, buf);
}

/// Commit a numeric parameter from its edit buffer, ignoring a buffer
/// that does not parse as `f64`.
pub(super) fn commit_number(state: &mut ComponentPreviewState, name: String) {
    if let Some(buf) = state.params_edit_buf.get(&name).cloned() {
        if let Ok(value) = buf.trim().parse::<f64>() {
            state
                .row
                .parameters
                .insert(name, signex_library::ParamValue::Number(value));
            state.dirty = true;
        }
    }
}

/// Live-update the edit buffer for a measurement parameter row.
pub(super) fn set_measurement_buf(state: &mut ComponentPreviewState, name: String, buf: String) {
    state.params_edit_buf.insert(name, buf);
}

/// Commit a measurement parameter (value + unit) from its edit buffer,
/// ignoring a buffer that does not parse as `f64`.
pub(super) fn commit_measurement(state: &mut ComponentPreviewState, name: String, unit: String) {
    if let Some(buf) = state.params_edit_buf.get(&name).cloned() {
        if let Ok(value) = buf.trim().parse::<f64>() {
            state.row.parameters.insert(
                name,
                signex_library::ParamValue::Measurement { value, unit },
            );
            state.dirty = true;
        }
    }
}

/// Toggle a boolean parameter.
pub(super) fn set_bool(state: &mut ComponentPreviewState, name: String, value: bool) {
    state
        .row
        .parameters
        .insert(name, signex_library::ParamValue::Bool(value));
    state.dirty = true;
}

/// Drop a parameter from the row.
pub(super) fn remove(state: &mut ComponentPreviewState, name: String) {
    state.row.parameters.remove(&name);
    state.dirty = true;
}

/// Add a custom parameter row with an empty value of the chosen kind.
/// Ignores a blank name.
pub(super) fn add_custom(state: &mut ComponentPreviewState, name: String, kind: ParamKindMsg) {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return;
    }
    let value = match kind {
        ParamKindMsg::Text => signex_library::ParamValue::Text(String::new()),
        ParamKindMsg::Number => signex_library::ParamValue::Number(0.0),
        ParamKindMsg::Bool => signex_library::ParamValue::Bool(false),
        ParamKindMsg::Measurement(unit) => {
            signex_library::ParamValue::Measurement { value: 0.0, unit }
        }
    };
    state.row.parameters.insert(trimmed.to_string(), value);
    state.dirty = true;
}
