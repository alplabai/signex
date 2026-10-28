//! Parameter edits for a Component Preview row.
//!
//! Text and boolean parameters commit immediately; numeric and
//! measurement parameters are edited through a per-row buffer
//! (`params_edit_buf`) and committed on demand so a half-typed value is
//! never parsed.

use signex_types::quantity::{MantissaError, SiPrefix, Unit};

use crate::library::ComponentPreviewState;
use crate::library::messages::ParamKindMsg;
use crate::library::prefix_picker::effective_prefix;

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
    if let Some(buf) = state.params_edit_buf.get(&name).cloned()
        && let Some(value) = crate::numeric_field::parse_field_f64(&buf)
    {
        state
            .row
            .parameters
            .insert(name, signex_library::ParamValue::Number(value));
        state.dirty = true;
    }
}

/// Live-update the edit buffer for a measurement parameter row.
pub(super) fn set_measurement_buf(state: &mut ComponentPreviewState, name: String, buf: String) {
    state.params_edit_buf.insert(name, buf);
}

/// Commit a measurement parameter from its edit buffer, ignoring a
/// buffer that is not a finite number.
///
/// A known `unit` makes the buffer a plain number in the picked SI prefix
/// ([`pick_prefix`]; by default the prefix the stored value displays
/// with). It is stored canonical (`{4700, "ohm"}` for 4.7 + k) and an
/// unchanged number leaves the stored value, legacy `{4.7, "kΩ"}`
/// included, exactly as it was. A unit string that is not a known unit
/// keeps the raw number and its string.
pub(super) fn commit_measurement(state: &mut ComponentPreviewState, name: String, unit: String) {
    let Some(buf) = state.params_edit_buf.get(&name).cloned() else {
        return;
    };
    if let Some(known) = Unit::from_alias(unit.trim()) {
        commit_typed_measurement(state, name, &buf, &known);
    } else if let Some(value) = crate::numeric_field::parse_field_f64(&buf) {
        state.row.parameters.insert(
            name,
            signex_library::ParamValue::Measurement { value, unit },
        );
        state.dirty = true;
    }
}

fn commit_typed_measurement(
    state: &mut ComponentPreviewState,
    name: String,
    buf: &str,
    unit: &Unit,
) {
    let prefix = effective_prefix(unit, state.measurement_prefix(&name));
    match signex_library::commit_measurement(state.row.parameters.get(&name), buf, prefix, unit) {
        Ok(value) => {
            if state.row.parameters.get(&name) != Some(&value) {
                state.row.parameters.insert(name.clone(), value);
                state.dirty = true;
            }
            // The field reads from the stored value again.
            state.params_edit_buf.remove(&name);
            state.params_prefix.remove(&name);
        }
        Err(err @ MantissaError::NonFinite(_)) => {
            crate::diagnostics::log_warning(format!("Parameter '{name}' refused: {err}"));
        }
        // Mid-edit or not a plain number: the typed text stays for the
        // user to fix, and Save names it.
        Err(_) => {}
    }
}

/// The user picked an SI prefix for a measurement's number: remember it
/// and commit the buffered number (or the stored one) in that prefix.
pub(super) fn pick_prefix(
    state: &mut ComponentPreviewState,
    name: String,
    unit: String,
    prefix: SiPrefix,
) {
    let Some(known) = Unit::from_alias(unit.trim()) else {
        return;
    };
    if !state.params_edit_buf.contains_key(&name)
        && let Some(parts) = state
            .row
            .parameters
            .get(&name)
            .and_then(|v| v.measurement_parts())
    {
        state.params_edit_buf.insert(name.clone(), parts.mantissa);
    }
    state
        .params_prefix
        .insert(name.clone(), effective_prefix(&known, prefix));
    commit_measurement(state, name, unit);
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
    state.params_edit_buf.remove(&name);
    state.params_prefix.remove(&name);
    state.dirty = true;
}

/// Commit every parseable numeric / measurement edit buffer into the row
/// so a save does not drop a typed value. A buffer that does not parse
/// stays put and the first such parameter is named in the `Err`, so the
/// caller refuses the save instead of silently dropping the typed text.
/// A buffer whose parameter is no longer a number / measurement is dead
/// and is discarded.
pub(crate) fn flush_edit_buffers(state: &mut ComponentPreviewState) -> Result<(), String> {
    use signex_library::ParamValue;
    let mut unparsed: Option<String> = None;
    let names: Vec<String> = state.params_edit_buf.keys().cloned().collect();
    for name in names {
        let unit = match state.row.parameters.get(&name) {
            Some(ParamValue::Number(_)) => None,
            Some(ParamValue::Measurement { unit, .. }) => Some(unit.clone()),
            _ => {
                state.params_edit_buf.remove(&name);
                continue;
            }
        };
        match unit {
            None => commit_number(state, name.clone()),
            Some(unit) => commit_measurement(state, name.clone(), unit),
        }
        // A typed measurement commit clears its own buffer; what is left
        // is committed text (numbers) or text that is not a number.
        let parses = state
            .params_edit_buf
            .get(&name)
            .is_none_or(|b| crate::numeric_field::parse_field_f64(b).is_some());
        if parses {
            state.params_edit_buf.remove(&name);
            state.params_prefix.remove(&name);
        } else if unparsed.is_none() {
            unparsed = Some(name);
        }
    }
    match unparsed {
        Some(name) => Err(format!("parameter '{name}' is not a valid number")),
        None => Ok(()),
    }
}

/// Add a custom parameter row with an empty value of the chosen kind,
/// named by `new_param_name`. Ignores a blank name and a name that
/// already exists, so an existing value is never overwritten.
pub(super) fn add_custom(state: &mut ComponentPreviewState, kind: ParamKindMsg) {
    let name = state.new_param_name.clone();
    let trimmed = name.trim();
    if trimmed.is_empty() || state.row.parameters.contains_key(trimmed) {
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
    state.new_param_name.clear();
    state.dirty = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::{
        ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
        ParamMap, ParamValue, PlmReserved, PrimitiveRef,
    };
    use signex_types::quantity::SiPrefix;
    use std::path::PathBuf;
    use uuid::Uuid;

    const NON_FINITE: [&str; 5] = ["nan", "inf", "-inf", "1e999", "-1e999"];

    fn fixture() -> ComponentPreviewState {
        let row = ComponentRow {
            alternate_symbols: Vec::new(),
            row_id: Uuid::new_v4(),
            internal_pn: InternalPn::new("TEST_PN"),
            class: ComponentClass::generic(),
            datasheet: DatasheetRef::default(),
            datasheet_label: String::new(),
            description: String::new(),
            more_datasheets: Vec::new(),
            state: LifecycleState::Draft,
            symbol_ref: PrimitiveRef::new(Uuid::new_v4(), Uuid::new_v4()),
            footprint_ref: None,
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("Acme", "ACM-001"),
            alternates: Vec::new(),
            supply: Vec::new(),
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            version: "0.0.1".into(),
            released: false,
            symbol_version: String::new(),
            footprint_version: String::new(),
            sim_version: String::new(),
            created: chrono::Utc::now(),
            updated: chrono::Utc::now(),
            content_hash: [0u8; 32],
            server_revision: None,
        };
        ComponentPreviewState::from_row(PathBuf::from("/tmp/test.snxlib"), "generics".into(), row)
    }

    /// #828 -- a number cell keeps its stored value when the buffer is
    /// `nan` / `inf` / an overflow, and a good buffer still commits.
    #[test]
    fn commit_number_refuses_non_finite() {
        for bad in NON_FINITE {
            let mut state = fixture();
            state
                .row
                .parameters
                .insert("tcr".into(), ParamValue::Number(5.0));
            set_number_buf(&mut state, "tcr".into(), bad.into());
            commit_number(&mut state, "tcr".into());
            assert_eq!(
                state.row.parameters.get("tcr"),
                Some(&ParamValue::Number(5.0)),
                "{bad:?}"
            );
            assert!(!state.dirty, "{bad:?} must not dirty the row");
        }
        let mut state = fixture();
        set_number_buf(&mut state, "tcr".into(), "7.5".into());
        commit_number(&mut state, "tcr".into());
        assert_eq!(
            state.row.parameters.get("tcr"),
            Some(&ParamValue::Number(7.5))
        );
    }

    /// A buffer that is mid-edit (`12.5e`) is not a number yet: nothing
    /// commits and the typed text stays in the buffer for the user to fix.
    #[test]
    fn commit_number_skips_a_mid_edit_buffer() {
        let mut state = fixture();
        set_number_buf(&mut state, "tcr".into(), "12.5e".into());
        commit_number(&mut state, "tcr".into());
        assert!(!state.row.parameters.contains_key("tcr"));
        assert!(!state.dirty);
        assert_eq!(
            state.params_edit_buf.get("tcr").map(String::as_str),
            Some("12.5e")
        );
    }

    /// #828 -- same contract for a measurement cell; the unit is untouched.
    #[test]
    fn commit_measurement_refuses_non_finite() {
        let stored = ParamValue::Measurement {
            value: 12.0,
            unit: "V".into(),
        };
        for bad in NON_FINITE {
            let mut state = fixture();
            state.row.parameters.insert("vmax".into(), stored.clone());
            set_measurement_buf(&mut state, "vmax".into(), bad.into());
            commit_measurement(&mut state, "vmax".into(), "V".into());
            assert_eq!(state.row.parameters.get("vmax"), Some(&stored), "{bad:?}");
            assert!(!state.dirty, "{bad:?} must not dirty the row");
        }
        let mut state = fixture();
        set_measurement_buf(&mut state, "vmax".into(), "12.5".into());
        commit_measurement(&mut state, "vmax".into(), "V".into());
        assert_eq!(
            state.row.parameters.get("vmax"),
            Some(&ParamValue::Measurement {
                value: 12.5,
                unit: "V".into(),
            })
        );
    }

    /// #1356 -- Save flushes a typed number / measurement still in its
    /// edit buffer into the row, and empties the buffer.
    #[test]
    fn flush_commits_buffered_values_before_save() {
        let mut state = fixture();
        state
            .row
            .parameters
            .insert("tcr".into(), ParamValue::Number(1.0));
        state.row.parameters.insert(
            "vmax".into(),
            ParamValue::Measurement {
                value: 1.0,
                unit: "V".into(),
            },
        );
        set_number_buf(&mut state, "tcr".into(), "7.5".into());
        set_measurement_buf(&mut state, "vmax".into(), "12".into());
        assert_eq!(flush_edit_buffers(&mut state), Ok(()));
        assert_eq!(
            state.row.parameters.get("tcr"),
            Some(&ParamValue::Number(7.5))
        );
        assert_eq!(
            state.row.parameters.get("vmax"),
            Some(&ParamValue::Measurement {
                value: 12.0,
                unit: "V".into()
            })
        );
        assert!(state.params_edit_buf.is_empty());
    }

    /// #1356 -- removing a parameter drops its buffer too, so the dead
    /// text cannot keep the tab dirty or resurface.
    #[test]
    fn remove_clears_the_edit_buffer() {
        let mut state = fixture();
        state
            .row
            .parameters
            .insert("tcr".into(), ParamValue::Number(1.0));
        set_number_buf(&mut state, "tcr".into(), "7".into());
        remove(&mut state, "tcr".into());
        assert!(state.params_edit_buf.is_empty());
    }

    /// #1356 -- the kind pill adds the parameter named in the input.
    #[test]
    fn add_custom_uses_the_typed_name() {
        let mut state = fixture();
        add_custom(&mut state, ParamKindMsg::Number);
        assert!(state.row.parameters.is_empty(), "blank name adds nothing");
        state.new_param_name = "  gain ".into();
        add_custom(&mut state, ParamKindMsg::Number);
        assert_eq!(
            state.row.parameters.get("gain"),
            Some(&ParamValue::Number(0.0))
        );
        assert!(state.new_param_name.is_empty());
    }

    /// An existing parameter is never replaced by adding the same name.
    #[test]
    fn add_custom_keeps_an_existing_value() {
        let mut state = fixture();
        let stored = ParamValue::Measurement {
            value: 10000.0,
            unit: "ohm".into(),
        };
        state
            .row
            .parameters
            .insert("resistance".into(), stored.clone());
        state.new_param_name = "resistance".into();
        add_custom(&mut state, ParamKindMsg::Text);
        assert_eq!(state.row.parameters.get("resistance"), Some(&stored));
        assert!(!state.dirty);
        assert_eq!(state.new_param_name, "resistance");
    }

    /// A save is refused for an unparseable buffer (named in the error);
    /// a buffer for a now-Text parameter is discarded.
    #[test]
    fn flush_reports_unparseable_and_drops_dead_buffers() {
        let mut state = fixture();
        state
            .row
            .parameters
            .insert("tcr".into(), ParamValue::Number(1.0));
        state
            .row
            .parameters
            .insert("note".into(), ParamValue::Text("x".into()));
        set_number_buf(&mut state, "tcr".into(), "12x".into());
        set_number_buf(&mut state, "note".into(), "5".into());
        let err = flush_edit_buffers(&mut state).unwrap_err();
        assert!(err.contains("tcr"), "{err}");
        assert_eq!(
            state.row.parameters.get("tcr"),
            Some(&ParamValue::Number(1.0))
        );
        assert!(state.params_edit_buf.contains_key("tcr"));
        assert!(!state.params_edit_buf.contains_key("note"));
    }

    fn meas(value: f64, unit: &str) -> ParamValue {
        ParamValue::Measurement {
            value,
            unit: unit.into(),
        }
    }

    fn state_with(name: &str, stored: ParamValue) -> ComponentPreviewState {
        let mut state = fixture();
        state.row.parameters.insert(name.into(), stored);
        state
    }

    /// The prefix the picker shows next to a stored value, and the number
    /// the field seeds with, for the unit's editor.
    fn shown(state: &ComponentPreviewState, name: &str) -> (String, SiPrefix) {
        let parts = state.row.parameters[name].measurement_parts().unwrap();
        (parts.mantissa, state.measurement_prefix(name))
    }

    /// #1606: a legacy `{4.7, "kΩ"}` edits as 4.7 with the k picker, not
    /// as 4.7 ohm. Retyping the seed is a no-op; a real edit stores the
    /// base value.
    #[test]
    fn a_legacy_kilo_ohm_edits_as_4_7_with_the_k_picker() {
        let mut state = state_with("value", meas(4.7, "kΩ"));
        assert_eq!(shown(&state, "value"), ("4.7".into(), SiPrefix::Kilo));

        set_measurement_buf(&mut state, "value".into(), "4.7".into());
        commit_measurement(&mut state, "value".into(), "ohm".into());
        assert_eq!(state.row.parameters["value"], meas(4.7, "kΩ"));
        assert!(!state.dirty, "an unchanged commit must not dirty the row");

        set_measurement_buf(&mut state, "value".into(), "10".into());
        commit_measurement(&mut state, "value".into(), "ohm".into());
        assert_eq!(state.row.parameters["value"], meas(10_000.0, "ohm"));
        assert!(state.dirty);
    }

    #[test]
    fn a_canonical_value_edits_in_its_shown_prefix() {
        let mut state = state_with("value", meas(4700.0, "ohm"));
        assert_eq!(shown(&state, "value"), ("4.7".into(), SiPrefix::Kilo));
        set_measurement_buf(&mut state, "value".into(), "4.7".into());
        commit_measurement(&mut state, "value".into(), "ohm".into());
        assert_eq!(state.row.parameters["value"], meas(4700.0, "ohm"));
        assert!(!state.dirty);
        set_measurement_buf(&mut state, "value".into(), "5.6".into());
        commit_measurement(&mut state, "value".into(), "ohm".into());
        assert_eq!(state.row.parameters["value"], meas(5600.0, "ohm"));
    }

    #[test]
    fn picking_a_prefix_commits_the_number_in_it() {
        let mut state = state_with("value", meas(4700.0, "ohm"));
        pick_prefix(&mut state, "value".into(), "ohm".into(), SiPrefix::Mega);
        assert_eq!(state.row.parameters["value"], meas(4_700_000.0, "ohm"));
        // A typed number picks up the prefix chosen afterwards.
        set_measurement_buf(&mut state, "value".into(), "33".into());
        pick_prefix(&mut state, "value".into(), "ohm".into(), SiPrefix::Kilo);
        assert_eq!(state.row.parameters["value"], meas(33_000.0, "ohm"));
    }

    #[test]
    fn a_prefix_typed_into_the_number_field_is_refused() {
        for typed in ["4k7", "10k", "4.7uF"] {
            let mut state = state_with("value", meas(4700.0, "ohm"));
            set_measurement_buf(&mut state, "value".into(), typed.into());
            commit_measurement(&mut state, "value".into(), "ohm".into());
            assert_eq!(
                state.row.parameters["value"],
                meas(4700.0, "ohm"),
                "{typed}"
            );
            assert!(!state.dirty, "{typed}");
            assert_eq!(
                state.params_edit_buf.get("value").map(String::as_str),
                Some(typed)
            );
        }
    }

    #[test]
    fn a_unit_without_prefixes_ignores_a_stale_pick() {
        let mut state = state_with("tolerance", meas(1.0, "%"));
        set_measurement_buf(&mut state, "tolerance".into(), "5".into());
        pick_prefix(&mut state, "tolerance".into(), "%".into(), SiPrefix::Kilo);
        assert_eq!(state.row.parameters["tolerance"], meas(5.0, "%"));
    }

    #[test]
    fn an_unknown_unit_keeps_the_text_editor_and_its_string() {
        let mut state = state_with("len", meas(1.5, "furlong"));
        set_measurement_buf(&mut state, "len".into(), "2".into());
        commit_measurement(&mut state, "len".into(), "furlong".into());
        assert_eq!(state.row.parameters["len"], meas(2.0, "furlong"));
    }
}
