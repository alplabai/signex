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

/// Commit a measurement parameter (value + unit) from its edit buffer,
/// ignoring a buffer that is not a finite number.
pub(super) fn commit_measurement(state: &mut ComponentPreviewState, name: String, unit: String) {
    if let Some(buf) = state.params_edit_buf.get(&name).cloned()
        && let Some(value) = crate::numeric_field::parse_field_f64(&buf)
    {
        state.row.parameters.insert(
            name,
            signex_library::ParamValue::Measurement { value, unit },
        );
        state.dirty = true;
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

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::{
        ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
        ParamMap, ParamValue, PlmReserved, PrimitiveRef,
    };
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
            more_datasheets: Vec::new(),
            state: LifecycleState::Draft,
            symbol_ref: PrimitiveRef::new(Uuid::new_v4(), Uuid::new_v4()),
            footprint_ref: None,
            alternate_footprints: Vec::new(),
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
}
