//! Parameter edits for a Component Preview row.
//!
//! Text and boolean parameters commit immediately; numeric and
//! measurement parameters are edited through a per-row buffer
//! (`params_edit_buf`) and committed on demand so a half-typed value is
//! never parsed.

use signex_library::ParamValue;

use super::measurement::{commit_measurement, editing_unit, measurement_slot};
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
    state.params_slot.remove(&name);
    state.params_error.remove(&name);
    state.dirty = true;
}

/// Commit every parseable numeric / measurement edit buffer into the row
/// so a save does not drop a typed value. A buffer that does not parse,
/// or a measurement the editor refuses (a prefix typed into the number,
/// a value that overflows, zero or a negative the class template does not
/// allow), stays put and the first such parameter is named in the `Err`
/// with the reason, so the caller refuses the save instead of silently
/// dropping the typed text. A buffer whose parameter is no longer a
/// number / measurement is dead and is discarded.
pub(crate) fn flush_edit_buffers(state: &mut ComponentPreviewState) -> Result<(), String> {
    let mut refused: Option<String> = None;
    let names: Vec<String> = state.params_edit_buf.keys().cloned().collect();
    for name in names {
        let slot = measurement_slot(state, &name);
        // A class-template measurement slot decides how its buffer is
        // committed, whatever kind the stored value has (a legacy plain
        // number in a slot is edited as a measurement, like on Enter).
        let unit = match (&slot, state.row.parameters.get(&name)) {
            (Some(slot), _) => Some(slot.unit.clone().unwrap_or_default()),
            (None, Some(ParamValue::Number(_))) => None,
            (None, Some(ParamValue::Measurement { unit, .. })) => Some(unit.clone()),
            _ => {
                state.params_edit_buf.remove(&name);
                continue;
            }
        };
        let typed = unit
            .as_deref()
            .is_some_and(|u| editing_unit(slot.as_ref(), u).is_some());
        match unit {
            None => commit_number(state, name.clone()),
            Some(unit) => commit_measurement(state, name.clone(), unit),
        }
        let Some(buf) = state.params_edit_buf.get(&name) else {
            continue;
        };
        // A raw number commit leaves its buffer; a typed one that is still
        // here was refused.
        if !typed && crate::numeric_field::parse_field_f64(buf).is_some() {
            state.params_edit_buf.remove(&name);
            state.params_prefix.remove(&name);
        } else if refused.is_none() {
            let why = state
                .params_error
                .get(&name)
                .cloned()
                .unwrap_or_else(|| "is not a valid number".to_string());
            refused = Some(format!("parameter '{name}': {why}"));
        }
    }
    match refused {
        Some(msg) => Err(msg),
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
    use super::super::measurement::{pick_prefix, pick_unit, set_measurement_buf};
    use super::*;
    use signex_library::{
        ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
        ParamMap, PlmReserved, PrimitiveRef,
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
            set_measurement_buf(&mut state, "vmax".into(), bad.into(), None);
            commit_measurement(&mut state, "vmax".into(), "V".into());
            assert_eq!(state.row.parameters.get("vmax"), Some(&stored), "{bad:?}");
            assert!(!state.dirty, "{bad:?} must not dirty the row");
        }
        let mut state = fixture();
        set_measurement_buf(&mut state, "vmax".into(), "12.5".into(), None);
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
        set_measurement_buf(&mut state, "vmax".into(), "12".into(), None);
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

        set_measurement_buf(&mut state, "value".into(), "4.7".into(), None);
        commit_measurement(&mut state, "value".into(), "ohm".into());
        assert_eq!(state.row.parameters["value"], meas(4.7, "kΩ"));
        assert!(!state.dirty, "an unchanged commit must not dirty the row");

        set_measurement_buf(&mut state, "value".into(), "10".into(), None);
        commit_measurement(&mut state, "value".into(), "ohm".into());
        assert_eq!(state.row.parameters["value"], meas(10_000.0, "ohm"));
        assert!(state.dirty);
    }

    #[test]
    fn a_canonical_value_edits_in_its_shown_prefix() {
        let mut state = state_with("value", meas(4700.0, "ohm"));
        assert_eq!(shown(&state, "value"), ("4.7".into(), SiPrefix::Kilo));
        set_measurement_buf(&mut state, "value".into(), "4.7".into(), None);
        commit_measurement(&mut state, "value".into(), "ohm".into());
        assert_eq!(state.row.parameters["value"], meas(4700.0, "ohm"));
        assert!(!state.dirty);
        set_measurement_buf(&mut state, "value".into(), "5.6".into(), None);
        commit_measurement(&mut state, "value".into(), "ohm".into());
        assert_eq!(state.row.parameters["value"], meas(5600.0, "ohm"));
    }

    #[test]
    fn picking_a_prefix_commits_the_number_in_it() {
        let mut state = state_with("value", meas(4700.0, "ohm"));
        pick_prefix(
            &mut state,
            "value".into(),
            "ohm".into(),
            SiPrefix::Mega,
            None,
        );
        assert_eq!(state.row.parameters["value"], meas(4_700_000.0, "ohm"));
        // A typed number picks up the prefix chosen afterwards.
        set_measurement_buf(&mut state, "value".into(), "33".into(), None);
        pick_prefix(
            &mut state,
            "value".into(),
            "ohm".into(),
            SiPrefix::Kilo,
            None,
        );
        assert_eq!(state.row.parameters["value"], meas(33_000.0, "ohm"));
    }

    #[test]
    fn a_prefix_typed_into_the_number_field_is_refused() {
        for typed in ["4k7", "10k", "4.7uF"] {
            let mut state = state_with("value", meas(4700.0, "ohm"));
            set_measurement_buf(&mut state, "value".into(), typed.into(), None);
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
        set_measurement_buf(&mut state, "tolerance".into(), "5".into(), None);
        pick_prefix(
            &mut state,
            "tolerance".into(),
            "%".into(),
            SiPrefix::Kilo,
            None,
        );
        assert_eq!(state.row.parameters["tolerance"], meas(5.0, "%"));
    }

    #[test]
    fn picking_a_unit_stores_that_unit_token() {
        let mut state = state_with("gain", meas(0.0, ""));
        pick_unit(&mut state, "gain".into(), "F".into());
        assert_eq!(state.row.parameters["gain"], meas(0.0, "F"));
        // A number typed before the pick is committed in the new unit.
        set_measurement_buf(&mut state, "gain".into(), "100".into(), None);
        pick_prefix(&mut state, "gain".into(), "F".into(), SiPrefix::Nano, None);
        pick_unit(&mut state, "gain".into(), "H".into());
        assert_eq!(state.row.parameters["gain"], meas(1e-7, "H"));
    }

    #[test]
    fn an_unknown_unit_keeps_the_text_editor_and_its_string() {
        let mut state = state_with("len", meas(1.5, "furlong"));
        set_measurement_buf(&mut state, "len".into(), "2".into(), None);
        commit_measurement(&mut state, "len".into(), "furlong".into());
        assert_eq!(state.row.parameters["len"], meas(2.0, "furlong"));
    }

    fn slot(name: &str, unit: &str) -> signex_library::ParamSlot {
        signex_library::ParamSlot {
            name: name.into(),
            kind: signex_library::ParamKind::Measurement,
            unit: Some(unit.into()),
            allow_zero: false,
            allow_negative: false,
            display_plus_minus: false,
        }
    }

    /// Type `text` into a slot's field and press Enter.
    fn enter(state: &mut ComponentPreviewState, name: &str, unit: &str, text: &str) {
        set_measurement_buf(state, name.into(), text.into(), Some(slot(name, unit)));
        commit_measurement(state, name.into(), unit.into());
    }

    /// Zero on a capacitor slot is refused: nothing is stored, the field
    /// has the reason, and Save is blocked until it is fixed.
    #[test]
    fn a_zero_capacitance_is_refused_and_blocks_save() {
        let mut state = fixture();
        enter(&mut state, "value", "F", "0");
        assert!(!state.row.parameters.contains_key("value"));
        assert!(!state.dirty);
        let why = state
            .params_error
            .get("value")
            .expect("the reason is shown");
        assert!(why.contains("zero not allowed"), "{why}");
        let err = flush_edit_buffers(&mut state).unwrap_err();
        assert!(
            err.contains("value") && err.contains("zero not allowed"),
            "{err}"
        );
        // Fixing it clears the reason and Save goes through.
        enter(&mut state, "value", "F", "100");
        assert!(state.params_error.is_empty());
        assert_eq!(flush_edit_buffers(&mut state), Ok(()));
    }

    #[test]
    fn a_negative_resistance_is_refused_and_blocks_save() {
        let mut state = state_with("value", meas(4700.0, "ohm"));
        enter(&mut state, "value", "ohm", "-4.7");
        assert_eq!(state.row.parameters["value"], meas(4700.0, "ohm"));
        assert!(!state.dirty);
        let why = state
            .params_error
            .get("value")
            .expect("the reason is shown");
        assert!(why.contains("negative not allowed"), "{why}");
        assert!(flush_edit_buffers(&mut state).is_err());
    }

    /// A slot that allows a negative (an offset) takes it.
    #[test]
    fn a_slot_that_allows_a_negative_takes_it() {
        let mut state = fixture();
        let mut offset = slot("offset", "V");
        offset.allow_negative = true;
        set_measurement_buf(&mut state, "offset".into(), "-5".into(), Some(offset));
        commit_measurement(&mut state, "offset".into(), "V".into());
        assert_eq!(state.row.parameters["offset"], meas(-5.0, "V"));
    }

    /// Enter on a legacy wrong-unit value, number unchanged, rewrites
    /// nothing: the unit stays V, no refusal, not dirty. Editing it stores
    /// the number in the slot's unit.
    #[test]
    fn enter_on_a_legacy_wrong_unit_value_does_not_rewrite_it() {
        let mut state = state_with("value", meas(4.7, "V"));
        enter(&mut state, "value", "ohm", "4.7");
        assert_eq!(state.row.parameters["value"], meas(4.7, "V"));
        assert!(!state.dirty);
        assert!(state.params_error.is_empty());

        enter(&mut state, "value", "ohm", "5.6");
        assert_eq!(state.row.parameters["value"], meas(5.6, "ohm"));
        assert!(state.dirty);
    }

    /// `1e308` with the M prefix overflows: refused with a message and
    /// Save is blocked, not silently dropped.
    #[test]
    fn an_overflowing_edit_is_refused_and_blocks_save() {
        let mut state = state_with("value", meas(4700.0, "ohm"));
        set_measurement_buf(
            &mut state,
            "value".into(),
            "1e308".into(),
            Some(slot("value", "ohm")),
        );
        pick_prefix(
            &mut state,
            "value".into(),
            "ohm".into(),
            SiPrefix::Mega,
            Some(slot("value", "ohm")),
        );
        assert_eq!(state.row.parameters["value"], meas(4700.0, "ohm"));
        assert!(state.params_error.contains_key("value"));
        let err = flush_edit_buffers(&mut state).unwrap_err();
        assert!(err.contains("value"), "{err}");
        assert_eq!(state.row.parameters["value"], meas(4700.0, "ohm"));
    }

    /// `4k7` shows its refusal as it is typed, before Enter or Save.
    #[test]
    fn a_prefix_typed_into_the_number_shows_the_refusal_at_once() {
        let mut state = state_with("value", meas(4700.0, "ohm"));
        set_measurement_buf(&mut state, "value".into(), "4k7".into(), None);
        assert!(state.params_error.contains_key("value"));
        set_measurement_buf(&mut state, "value".into(), "4.7".into(), None);
        assert!(state.params_error.is_empty());
    }

    /// Picking the unit a row already has changes nothing, not even dirty.
    #[test]
    fn re_picking_the_same_unit_is_not_an_edit() {
        let mut state = state_with("gain", meas(4700.0, "ohm"));
        pick_unit(&mut state, "gain".into(), "ohm".into());
        assert_eq!(state.row.parameters["gain"], meas(4700.0, "ohm"));
        assert!(!state.dirty);
    }

    /// The component editor keeps the number and prefix on a unit pick,
    /// like the Edit Component Details modal: 4.7 k stays 4.7 k where the
    /// new unit has the prefix, and the number alone stays where it has not.
    #[test]
    fn a_unit_pick_keeps_the_number_and_prefix() {
        let mut state = state_with("gain", meas(4.7, "kΩ"));
        pick_unit(&mut state, "gain".into(), "V".into());
        assert_eq!(state.row.parameters["gain"], meas(4700.0, "V"));
        assert!(state.dirty);

        // Farad has no kilo: the shown 4.7 stays 4.7 F, not 4700 F.
        let mut state = state_with("gain", meas(4.7, "kΩ"));
        pick_unit(&mut state, "gain".into(), "F".into());
        assert_eq!(state.row.parameters["gain"], meas(4.7, "F"));
    }

    /// Save on a slot row stored as a plain number commits it as a
    /// measurement, like Enter: `10` with k stores 10000 ohm, and the slot's
    /// rules apply (a negative resistance, a zero capacitance are refused).
    #[test]
    fn save_commits_a_plain_number_in_a_slot_like_enter() {
        let res = slot("value", "ohm");
        let mut state = state_with("value", ParamValue::Number(4.7));
        set_measurement_buf(&mut state, "value".into(), "10".into(), Some(res.clone()));
        state.params_prefix.insert("value".into(), SiPrefix::Kilo);
        assert_eq!(flush_edit_buffers(&mut state), Ok(()));
        assert_eq!(state.row.parameters["value"], meas(10_000.0, "ohm"));

        let mut enter_state = state_with("value", ParamValue::Number(4.7));
        enter_state
            .params_prefix
            .insert("value".into(), SiPrefix::Kilo);
        set_measurement_buf(
            &mut enter_state,
            "value".into(),
            "10".into(),
            Some(res.clone()),
        );
        commit_measurement(&mut enter_state, "value".into(), "ohm".into());
        assert_eq!(
            enter_state.row.parameters["value"],
            state.row.parameters["value"]
        );

        let mut negative = state_with("value", ParamValue::Number(4.7));
        set_measurement_buf(&mut negative, "value".into(), "-5".into(), Some(res));
        let err = flush_edit_buffers(&mut negative).unwrap_err();
        assert!(err.contains("negative not allowed"), "{err}");
        assert_eq!(negative.row.parameters["value"], ParamValue::Number(4.7));

        let mut zero = state_with("value", ParamValue::Number(1e-7));
        set_measurement_buf(
            &mut zero,
            "value".into(),
            "0".into(),
            Some(slot("value", "F")),
        );
        let err = flush_edit_buffers(&mut zero).unwrap_err();
        assert!(err.contains("zero not allowed"), "{err}");
        assert_eq!(zero.row.parameters["value"], ParamValue::Number(1e-7));
    }

    /// Enter on a slot value the field cannot express rewrites nothing: an
    /// unknown unit, a prefix the slot unit lacks, a plain number.
    #[test]
    fn enter_on_a_slot_value_the_field_cannot_express_rewrites_nothing() {
        let cases = [
            (meas(4.7, "meg"), slot("value", "ohm"), "ohm"),
            (meas(4.7, "kV"), slot("value", "F"), "F"),
            (ParamValue::Number(4.7), slot("value", "ohm"), "ohm"),
        ];
        for (stored, slot, unit) in cases {
            let mut state = state_with("value", stored.clone());
            let seed = signex_library::slot_field_seed(
                &stored,
                &signex_library::slot_unit(&slot).unwrap(),
            )
            .0;
            set_measurement_buf(&mut state, "value".into(), seed, Some(slot));
            commit_measurement(&mut state, "value".into(), unit.into());
            assert_eq!(state.row.parameters["value"], stored);
            assert!(!state.dirty, "{stored:?}");
            assert!(state.params_error.is_empty(), "{stored:?}");
        }
    }

    /// A unit change on a free parameter keeps the number as shown and
    /// says the value is reinterpreted; re-picking the same unit says nothing.
    #[test]
    fn a_unit_change_says_the_value_is_reinterpreted() {
        let mut state = state_with("gain", meas(4.7, "kΩ"));
        pick_unit(&mut state, "gain".into(), "%".into());
        let note = state.params_error.get("gain").expect("a notice");
        assert!(note.contains("reinterpreted"), "{note}");
        let mut same = state_with("gain", meas(4.7, "kΩ"));
        pick_unit(&mut same, "gain".into(), "ohm".into());
        assert!(same.params_error.is_empty());
    }
}
