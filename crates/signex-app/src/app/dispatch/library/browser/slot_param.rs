//! Adding a class-template slot as a parameter row in the Edit Component
//! Details modal (#1680).

use signex_library::{ParamKind, ParameterTemplate, blank_slot_value};

use crate::library::state::EditRowModalState;

/// Add a row for the template slot `name`, in the slot's own kind (a
/// measurement in the slot's unit). A name the template lacks, or a row
/// the draft already holds, is left alone.
pub(super) fn add_slot_param(
    modal: &mut EditRowModalState,
    name: &str,
    template: Option<&ParameterTemplate>,
) {
    let Some(slot) = template.and_then(|t| t.slot(name)) else {
        return;
    };
    if modal.draft.parameters.contains_key(name) {
        return;
    }
    let value = blank_slot_value(slot);
    let mut entry = EditRowModalState::param_buf_entry(&value);
    // A number field opens empty, so saving it unfilled is refused
    // instead of storing the placeholder zero.
    if matches!(slot.kind, ParamKind::Measurement | ParamKind::Number) {
        entry.0.clear();
    }
    modal.param_buf.insert(name.to_string(), entry);
    modal.draft.parameters.insert(name.to_string(), value);
    modal.error = None;
}
