//! What a Library Browser parameter cell commits, given what it holds.
//!
//! A cell that already holds a typed value keeps that type. Before #599
//! the `Err` arm of the numeric parse substituted
//! `ParamValue::Text(buffer)`, which silently retyped the parameter and
//! threw the stored unit away: `Measurement { 4.7, "kΩ" }` edited to
//! `4k7` became `Text("4k7")`, and the cell still rendered the typed
//! text so it looked committed. What was gone was the numeric type and
//! unit that every numeric facet filter, range query, sort and BOM
//! column keys on — the row dropped out of "resistance between 1k and
//! 10k" for good.
//!
//! An unparseable buffer is now a refusal: the stored value is left
//! alone and the caller reports it and keeps the buffer in the cell so
//! the user can correct it.
//!
//! #612 extended that to the boolean arm, which the register had not
//! recorded and which was retyping the same way — a buffer that is
//! neither `true` nor `false` used to become `Text` and take the
//! boolean type with it.

use signex_library::{
    ParamKind, ParamSlot, ParamValue, ParameterTemplate, commit_measurement,
    commit_slot_measurement, slot_unit,
};
use signex_types::numeric::parse_finite_f64;
use signex_types::quantity::{MantissaError, SiPrefix, Unit, parse_mantissa};

use crate::library::prefix_picker::effective_prefix;
use crate::library::unit_picker::unit_change_note;

/// Why a measurement cell refuses an inline commit, for the report.
const MEASUREMENT_IN_MODAL: &str =
    "an edit in the Edit Component Details dialog (number, SI prefix and unit)";

/// A refused commit — the buffer could not be read as the cell's type.
pub(super) struct ParamRefusal {
    /// What the cell would have accepted. Static text for the report.
    pub expected: &'static str,
    /// The stored value that was kept, rendered for the report.
    pub kept: String,
}

/// Decide the value a parameter cell commits.
///
/// `existing` is the value currently stored under the cell's key
/// (`None` for a key the row does not have yet). Returns `Err` when the
/// cell is typed and `buf` cannot be read as that type — the caller must
/// then leave the stored value untouched.
pub(super) fn param_value_for_commit(
    existing: Option<&ParamValue>,
    buf: &str,
    in_measurement_slot: bool,
) -> Result<ParamValue, ParamRefusal> {
    // A class-template measurement slot is edited in the dialog whatever
    // the stored value is (missing, text or a plain number): an inline
    // buffer would be stored as text and skip the slot's unit and rules.
    if in_measurement_slot {
        return Err(refusal(MEASUREMENT_IN_MODAL, existing));
    }
    match existing {
        // A measurement is not edited in a grid cell: its cell opens the
        // Edit Component Details modal, which has the SI-prefix picker and
        // the unit. A raw buffer cannot say which prefix it is in, so an
        // inline commit of one is refused and the stored value is kept
        // (reading it in the shown prefix stored 1000x off, #1572).
        Some(ParamValue::Measurement { .. }) => Err(refusal(MEASUREMENT_IN_MODAL, existing)),
        Some(ParamValue::Number(_)) => parse_finite_f64(buf)
            .map(ParamValue::Number)
            .map_err(|_| refusal("a finite number", existing)),
        // #612. The #599 register recorded the two numeric arms only, so
        // this one kept retyping to text — the same defect one arm up,
        // with the same consequence for anything keying on the boolean
        // type.
        //
        // The accepted set is deliberately unchanged: `true` / `false`,
        // case-insensitive, and nothing else. Widening it to `1`, `0`,
        // `yes`, `no` is a product decision about what a boolean cell
        // takes, and making it here would smuggle a behaviour change in
        // behind a bug fix. Refusing the rest breaks no input that
        // worked before — it only closes the path that destroyed the
        // type. The numeric arms above draw the line in the same place:
        // exactly what `parse_finite_f64` accepts.
        Some(ParamValue::Bool(_)) => {
            if buf.eq_ignore_ascii_case("true") {
                Ok(ParamValue::Bool(true))
            } else if buf.eq_ignore_ascii_case("false") {
                Ok(ParamValue::Bool(false))
            } else {
                Err(refusal("true or false", existing))
            }
        }
        // An untyped cell (text today, or a key the row does not carry
        // yet) takes the buffer verbatim — there is no type to lose.
        _ => Ok(ParamValue::Text(buf.to_string())),
    }
}

fn refusal(expected: &'static str, existing: Option<&ParamValue>) -> ParamRefusal {
    ParamRefusal {
        expected,
        kept: existing.map(ParamValue::display).unwrap_or_default(),
    }
}

/// The slot behind a parameter when it is a measurement slot with a known
/// unit: that slot fixes the unit and carries the sign rules.
fn measurement_slot<'t>(
    template: Option<&'t ParameterTemplate>,
    key: &str,
) -> Option<&'t ParamSlot> {
    template
        .and_then(|t| t.slot(key))
        .filter(|s| s.kind == ParamKind::Measurement && slot_unit(s).is_some())
}

/// Whether `key` is a measurement slot with a known unit in `template`.
pub(super) fn is_measurement_slot(template: Option<&ParameterTemplate>, key: &str) -> bool {
    measurement_slot(template, key).is_some()
}

/// What the edit modal's "commit parameter" stores for a typed `value`,
/// the picked `prefix` and `unit`. `Err` carries the reason the entry was
/// refused, and the draft must stay as it is.
///
/// A known unit makes the value a measurement: `value` is a plain number
/// (no prefix or unit typed into it) in `prefix`, stored canonical as
/// `{base, unit token}`, and an unchanged number keeps `existing` as it
/// is. A class-template `slot` fixes the unit (`unit` is ignored) and
/// refuses a value that breaks it. A unit string that is not a known unit
/// keeps the raw number and its string. A `nan` / `inf` / overflowing
/// entry is refused either way.
pub(super) fn modal_param_value(
    value: String,
    unit: &str,
    prefix: SiPrefix,
    existing: Option<&ParamValue>,
    slot: Option<&ParamSlot>,
) -> Result<ParamValue, String> {
    if let Some(slot) = slot {
        return commit_slot_measurement(existing, &value, prefix, slot);
    }
    use crate::numeric_field::{FieldNumber, parse_field_number};
    let unit_text = unit.trim();
    if let Some(known) = Unit::from_alias(unit_text) {
        let prefix = effective_prefix(&known, prefix);
        return commit_measurement(existing, &value, prefix, &known).map_err(|e| e.to_string());
    }
    let number = match parse_field_number(&value) {
        FieldNumber::Refused => return Err("not a finite number".to_string()),
        FieldNumber::Number(n) => Some(n),
        FieldNumber::NotANumber => None,
    };
    let pv = if !unit_text.is_empty() {
        // A number gets the unit; anything else is kept as text.
        match number {
            Some(n) => ParamValue::Measurement {
                value: n,
                unit: unit.to_string(),
            },
            None => ParamValue::Text(format!("{value} {unit}")),
        }
    } else if let Some(n) = number {
        ParamValue::Number(n)
    } else if value.eq_ignore_ascii_case("true") {
        ParamValue::Bool(true)
    } else if value.eq_ignore_ascii_case("false") {
        ParamValue::Bool(false)
    } else {
        ParamValue::Text(value)
    };
    Ok(pv)
}

/// Commit the edit modal's buffer for `key` to its draft. A refused entry
/// leaves the draft as it is and says why in the modal's error line.
pub(super) fn commit_param_buffer(
    modal: &mut crate::library::state::EditRowModalState,
    key: &str,
    template: Option<&ParameterTemplate>,
) {
    let Some((value, unit)) = modal.param_buf.get(key).cloned() else {
        return;
    };
    let prefix = modal
        .param_prefix
        .get(key)
        .copied()
        .unwrap_or(SiPrefix::One);
    let slot = measurement_slot(template, key);
    modal.note = None;
    match modal_param_value(value, &unit, prefix, modal.draft.parameters.get(key), slot) {
        Ok(pv) => {
            modal.draft.parameters.insert(key.to_string(), pv);
            modal.error = None;
        }
        Err(reason) => modal.error = Some(format!("Parameter \"{key}\": {reason}.")),
    }
}

/// The user picked a unit for `key`. A class-template slot fixes its unit,
/// so a pick on one is refused; any other parameter takes the unit and
/// keeps the number and prefix it shows.
pub(super) fn pick_unit(
    modal: &mut crate::library::state::EditRowModalState,
    key: &str,
    unit: String,
    template: Option<&ParameterTemplate>,
) {
    if let Some(slot) = measurement_slot(template, key) {
        modal.error = Some(format!(
            "Parameter \"{key}\": the class template fixes its unit to {}.",
            slot.unit.as_deref().unwrap_or_default()
        ));
        return;
    }
    let entry = modal
        .param_buf
        .entry(key.to_string())
        .or_insert_with(|| (String::new(), String::new()));
    entry.1 = unit;
    let before = modal
        .draft
        .parameters
        .get(key)
        .and_then(ParamValue::as_quantity)
        .map(|q| q.unit().clone());
    // A pick is a whole edit: apply it to the draft now.
    commit_param_buffer(modal, key, template);
    if modal.error.is_none()
        && let Some(note) = modal
            .draft
            .parameters
            .get(key)
            .and_then(|v| unit_change_note(key, before.as_ref(), v))
    {
        modal.note = Some(note);
    }
}

/// What is wrong with the number the user has typed so far, if it is
/// already clear: a prefix or unit typed into it (`4k7`, `10 k`). `None`
/// for a number, and for text that may still become one (`1.`, `-`).
pub(super) fn number_field_problem(
    modal: &crate::library::state::EditRowModalState,
    key: &str,
    template: Option<&ParameterTemplate>,
) -> Option<String> {
    let (value, unit) = modal.param_buf.get(key)?;
    // The slot's unit applies even when the buffer holds none (a slot
    // that stores a plain number or text).
    if measurement_slot(template, key).is_none() {
        Unit::from_alias(unit.trim())?;
    }
    match parse_mantissa(value) {
        Err(e @ (MantissaError::HasLetters(_) | MantissaError::Comma(_))) => {
            Some(format!("Parameter \"{key}\": {e}."))
        }
        _ => None,
    }
}

/// Every buffered parameter edit that was typed but never committed
/// (Enter), as the value `CommitParam` would store. `Err((key, reason))`
/// names the first entry that is refused, so Save can stop instead of
/// dropping it. `tags` is skipped: its own buffer is flushed separately.
/// An entry still showing what the row opened with is skipped, so a
/// stored value is only rewritten when it was edited.
pub(super) fn uncommitted_param_edits(
    draft: &signex_library::ParamMap,
    buf: &std::collections::HashMap<String, (String, String)>,
    prefixes: &std::collections::HashMap<String, SiPrefix>,
    template: Option<&ParameterTemplate>,
) -> Result<Vec<(String, ParamValue)>, (String, String)> {
    use crate::library::state::EditRowModalState;
    let mut keys: Vec<&String> = buf.keys().filter(|k| k.as_str() != "tags").collect();
    keys.sort();
    let mut edits = Vec::new();
    for key in keys {
        let (value, unit) = &buf[key];
        let prefix = prefixes.get(key).copied();
        let untouched = draft.get(key).is_some_and(|pv| {
            EditRowModalState::param_buf_entry(pv) == (value.clone(), unit.clone())
                && EditRowModalState::param_prefix_seed(pv) == prefix
        });
        if untouched {
            continue;
        }
        match modal_param_value(
            value.clone(),
            unit,
            prefix.unwrap_or(SiPrefix::One),
            draft.get(key),
            measurement_slot(template, key),
        ) {
            Ok(pv) => edits.push((key.clone(), pv)),
            Err(reason) => return Err((key.clone(), reason)),
        }
    }
    Ok(edits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical_kilo_ohms() -> ParamValue {
        ParamValue::Measurement {
            value: 4700.0,
            unit: "ohm".to_string(),
        }
    }

    fn kilo_ohms() -> ParamValue {
        ParamValue::Measurement {
            value: 4.7,
            unit: "kΩ".to_string(),
        }
    }

    /// A measurement cell no longer commits inline: the buffer cannot say
    /// which prefix it is in. Every buffer is refused and the stored value
    /// kept, so a cell showing `4.7 kΩ` never takes `470` as 470 kΩ.
    #[test]
    fn an_inline_commit_to_a_measurement_cell_is_refused() {
        for existing in [kilo_ohms(), canonical_kilo_ohms()] {
            for buf in ["470", "4.7", "10"] {
                let refusal = param_value_for_commit(Some(&existing), buf, false)
                    .expect_err("a measurement is edited in the dialog");
                assert_eq!(refusal.expected, MEASUREMENT_IN_MODAL);
                assert_eq!(refusal.kept, "4.7 kΩ");
            }
        }
    }

    #[test]
    fn a_measurement_cell_refuses_a_buffer_that_is_not_a_number() {
        // Arrange — `4k7` is the engineering shorthand the register
        // named; it is not an `f64` and must not retype the parameter.
        let existing = kilo_ohms();

        // Act
        let committed = param_value_for_commit(Some(&existing), "4k7", false);

        // Assert
        let refusal = committed.expect_err("an unparseable buffer must be refused");
        assert_eq!(refusal.kept, "4.7 kΩ");
        assert_eq!(refusal.expected, MEASUREMENT_IN_MODAL);
    }

    #[test]
    fn a_number_cell_refuses_a_buffer_that_is_not_a_number() {
        // Arrange
        let existing = ParamValue::Number(3.0);

        // Act
        let committed = param_value_for_commit(Some(&existing), "3 pieces", false);

        // Assert
        let refusal = committed.expect_err("an unparseable buffer must be refused");
        assert_eq!(refusal.kept, "3");
        assert_eq!(refusal.expected, "a finite number");
    }

    #[test]
    fn a_number_cell_commits_a_parseable_buffer() {
        // Arrange
        let existing = ParamValue::Number(3.0);

        // Act
        let committed = param_value_for_commit(Some(&existing), "12.5", false);

        // Assert
        assert_eq!(committed.ok(), Some(ParamValue::Number(12.5)));
    }

    #[test]
    fn an_untyped_cell_takes_the_buffer_verbatim() {
        // Arrange / Act
        let fresh = param_value_for_commit(None, "4k7", false);
        let text = param_value_for_commit(Some(&ParamValue::Text("old".into())), "4k7", false);

        // Assert
        assert_eq!(fresh.ok(), Some(ParamValue::Text("4k7".to_string())));
        assert_eq!(text.ok(), Some(ParamValue::Text("4k7".to_string())));
    }

    /// #612 — the defect itself. An unreadable buffer used to become
    /// `Text`, taking the boolean type with it and committing that to
    /// the library, so anything keying on the type lost the row.
    #[test]
    fn a_bool_cell_refuses_a_buffer_it_cannot_read_rather_than_retyping() {
        // Arrange
        let existing = ParamValue::Bool(true);

        // Act
        let committed = param_value_for_commit(Some(&existing), "maybe", false);

        // Assert
        let refusal = committed.expect_err("an unreadable boolean buffer must be refused");
        assert_eq!(refusal.expected, "true or false");
        assert_eq!(refusal.kept, ParamValue::Bool(true).display());
    }

    /// The accepted set is exactly what it was before #612 — the fix
    /// closes the retyping path without narrowing or widening what a
    /// boolean cell takes.
    #[test]
    fn a_bool_cell_still_takes_true_and_false_in_any_case() {
        // Arrange
        let existing = ParamValue::Bool(false);

        // Act / Assert
        for (buf, expected) in [
            ("true", true),
            ("TRUE", true),
            ("True", true),
            ("false", false),
            ("FALSE", false),
            ("False", false),
        ] {
            assert_eq!(
                param_value_for_commit(Some(&existing), buf, false).ok(),
                Some(ParamValue::Bool(expected)),
                "{buf} must still commit as a boolean"
            );
        }
    }

    /// A cell that is not typed boolean is unaffected: `"maybe"` is a
    /// perfectly good text value and must still commit.
    #[test]
    fn refusing_a_boolean_buffer_does_not_touch_untyped_cells() {
        // Arrange / Act
        let committed = param_value_for_commit(None, "maybe", false);

        // Assert
        assert_eq!(committed.ok(), Some(ParamValue::Text("maybe".to_string())));
    }

    const NON_FINITE: [&str; 5] = ["nan", "inf", "-inf", "1e999", "-1e999"];

    /// #828 -- a typed numeric cell refuses a non-finite buffer.
    #[test]
    fn typed_cells_refuse_non_finite_buffers() {
        for bad in NON_FINITE {
            let number = param_value_for_commit(Some(&ParamValue::Number(3.0)), bad, false);
            assert!(number.is_err(), "number cell took {bad:?}");
            let measurement = param_value_for_commit(Some(&kilo_ohms()), bad, false);
            assert!(measurement.is_err(), "measurement cell took {bad:?}");
        }
    }

    /// #828 -- the edit modal never stores a non-finite number, with or
    /// without a unit, and tells the caller to keep the draft.
    #[test]
    fn the_modal_refuses_non_finite_entries() {
        for bad in NON_FINITE {
            assert!(modal_param_value(bad.to_string(), "", SiPrefix::One, None, None).is_err());
            assert!(modal_param_value(bad.to_string(), "V", SiPrefix::One, None, None).is_err());
        }
    }

    #[test]
    fn the_modal_still_types_ordinary_entries() {
        let one = SiPrefix::One;
        assert_eq!(
            modal_param_value("2.5".into(), "", one, None, None),
            Ok(ParamValue::Number(2.5))
        );
        assert_eq!(
            modal_param_value("2.5".into(), "V", one, None, None),
            Ok(ParamValue::Measurement {
                value: 2.5,
                unit: "V".into()
            })
        );
        assert_eq!(
            modal_param_value("TRUE".into(), "", one, None, None),
            Ok(ParamValue::Bool(true))
        );
        assert_eq!(
            modal_param_value("abc".into(), "furlong", one, None, None),
            Ok(ParamValue::Text("abc furlong".into()))
        );
    }

    /// The number field takes a number, not `4k7` or `abc`, in a known unit.
    #[test]
    fn the_modal_refuses_letters_in_a_known_unit() {
        for typed in ["4k7", "10k", "4.7uF", "abc"] {
            assert!(
                modal_param_value(typed.into(), "ohm", SiPrefix::One, None, None).is_err(),
                "{typed}"
            );
        }
    }

    /// 4.7 with the k picked stores 4700 ohm, whatever was stored before.
    #[test]
    fn the_modal_stores_the_picked_prefix_as_a_base_value() {
        let kilo = SiPrefix::Kilo;
        let want = ParamValue::Measurement {
            value: 4700.0,
            unit: "ohm".into(),
        };
        assert_eq!(
            modal_param_value("4.7".into(), "ohm", kilo, None, None),
            Ok(want.clone())
        );
        // #1606: a legacy `{4.7, "kΩ"}` edited to 5.6 + k is 5600 ohm,
        // not 5.6 ohm; the same 4.7 + k is no change at all.
        assert_eq!(
            modal_param_value("5.6".into(), "ohm", kilo, Some(&kilo_ohms()), None),
            Ok(ParamValue::Measurement {
                value: 5600.0,
                unit: "ohm".into()
            })
        );
        assert_eq!(
            modal_param_value("4.7".into(), "ohm", kilo, Some(&kilo_ohms()), None),
            Ok(kilo_ohms())
        );
    }

    /// A unit that takes no prefix ignores a stale picked prefix.
    #[test]
    fn the_modal_ignores_a_prefix_for_percent() {
        assert_eq!(
            modal_param_value("1".into(), "%", SiPrefix::Kilo, None, None),
            Ok(ParamValue::Measurement {
                value: 1.0,
                unit: "%".into()
            })
        );
    }

    /// Save keeps an untouched legacy row's bytes: nothing is rewritten
    /// for a buffer that still holds what the row opened with.
    #[test]
    fn an_untouched_legacy_parameter_is_not_rewritten_on_save() {
        use crate::library::state::EditRowModalState;
        let mut draft = signex_library::ParamMap::new();
        draft.insert("value".into(), kilo_ohms());
        let buf = std::collections::HashMap::from([(
            "value".to_string(),
            EditRowModalState::param_buf_entry(&kilo_ohms()),
        )]);
        let prefixes = std::collections::HashMap::from([("value".to_string(), SiPrefix::Kilo)]);
        let edits = uncommitted_param_edits(&draft, &buf, &prefixes, None).unwrap();
        assert!(edits.is_empty());
    }

    fn slot(name: &str, unit: &str) -> ParamSlot {
        ParamSlot {
            name: name.into(),
            kind: ParamKind::Measurement,
            unit: Some(unit.into()),
            allow_zero: false,
            allow_negative: false,
            display_plus_minus: false,
        }
    }

    fn template(slot: ParamSlot) -> ParameterTemplate {
        ParameterTemplate {
            class: "resistor".into(),
            required_params: vec![slot],
            ..ParameterTemplate::default()
        }
    }

    /// A slot fixes the unit and refuses what breaks it.
    #[test]
    fn the_modal_refuses_a_value_that_breaks_its_slot() {
        let cap = slot("value", "F");
        let zero = modal_param_value("0".into(), "F", SiPrefix::One, None, Some(&cap));
        assert!(zero.unwrap_err().contains("zero not allowed"));
        let res = slot("value", "ohm");
        let neg = modal_param_value("-1".into(), "ohm", SiPrefix::One, None, Some(&res));
        assert!(neg.unwrap_err().contains("negative not allowed"));
    }

    /// A stale `F` in the buffer of a resistor's value slot is not taken:
    /// the slot's unit is.
    #[test]
    fn the_slot_unit_wins_over_the_buffer_unit() {
        let res = slot("value", "ohm");
        let stored = modal_param_value("4.7".into(), "F", SiPrefix::Kilo, None, Some(&res));
        assert_eq!(stored, Ok(canonical_kilo_ohms()));
    }

    fn blank_row() -> signex_library::ComponentRow {
        use signex_library::{
            ComponentClass, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
            PlmReserved, PrimitiveRef,
        };
        let now = chrono::Utc::now();
        signex_library::ComponentRow {
            alternate_symbols: Vec::new(),
            row_id: uuid::Uuid::now_v7(),
            internal_pn: InternalPn::new("R1"),
            class: ComponentClass::generic(),
            datasheet: DatasheetRef::default(),
            datasheet_label: String::new(),
            description: String::new(),
            more_datasheets: Vec::new(),
            state: LifecycleState::Draft,
            symbol_ref: PrimitiveRef::new(uuid::Uuid::nil(), uuid::Uuid::nil()),
            footprint_ref: None,
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("Acme", "MPN"),
            alternates: Vec::new(),
            supply: Vec::new(),
            parameters: signex_library::ParamMap::new(),
            plm: PlmReserved::default(),
            version: "0.0.1".into(),
            released: false,
            symbol_version: String::new(),
            footprint_version: String::new(),
            sim_version: String::new(),
            created: now,
            updated: now,
            content_hash: [0u8; 32],
            server_revision: None,
        }
    }

    fn modal_with(value: ParamValue) -> crate::library::state::EditRowModalState {
        let mut draft = blank_row();
        draft.class = signex_library::ComponentClass::new("resistor");
        draft.parameters.insert("value".into(), value);
        crate::library::state::EditRowModalState::new(
            crate::library::state::EditorAddress::new(
                std::path::PathBuf::from("lib.snxlib"),
                "Resistors".into(),
                signex_library::RowId::from_uuid(draft.row_id),
            ),
            draft,
        )
    }

    /// Picking F (or "no unit") on a slot parameter is refused: the draft
    /// keeps its unit and the modal says why.
    #[test]
    fn a_unit_pick_on_a_slot_parameter_is_refused() {
        let t = template(slot("value", "ohm"));
        for pick in ["F", ""] {
            let mut modal = modal_with(canonical_kilo_ohms());
            pick_unit(&mut modal, "value", pick.into(), Some(&t));
            assert_eq!(
                modal.draft.parameters["value"],
                canonical_kilo_ohms(),
                "{pick:?}"
            );
            assert!(
                modal
                    .error
                    .as_deref()
                    .is_some_and(|e| e.contains("fixes its unit"))
            );
        }
    }

    /// A parameter that is not a slot still takes a unit pick, keeping
    /// number and prefix.
    #[test]
    fn a_unit_pick_on_a_free_parameter_keeps_number_and_prefix() {
        let t = template(slot("other", "ohm"));
        let mut modal = modal_with(canonical_kilo_ohms());
        pick_unit(&mut modal, "value", "V".into(), Some(&t));
        assert_eq!(
            modal.draft.parameters["value"],
            ParamValue::Measurement {
                value: 4700.0,
                unit: "V".into()
            }
        );
    }

    /// Save is blocked by a typed value the slot refuses, and a legacy
    /// wrong-unit value left alone is not rewritten by it.
    #[test]
    fn save_refuses_a_slot_violation_and_keeps_a_legacy_one() {
        let t = template(slot("value", "ohm"));
        let legacy = ParamValue::Measurement {
            value: 4.7,
            unit: "V".into(),
        };
        let mut modal = modal_with(legacy.clone());
        // Untouched: nothing to commit.
        let edits = uncommitted_param_edits(
            &modal.draft.parameters,
            &modal.param_buf,
            &modal.param_prefix,
            Some(&t),
        )
        .unwrap();
        assert!(edits.is_empty());
        // Typed negative: Save is refused with the key and the reason.
        modal
            .param_buf
            .insert("value".into(), ("-4.7".into(), "V".into()));
        let err = uncommitted_param_edits(
            &modal.draft.parameters,
            &modal.param_buf,
            &modal.param_prefix,
            Some(&t),
        )
        .unwrap_err();
        assert_eq!(err.0, "value");
        assert!(err.1.contains("negative not allowed"), "{}", err.1);
    }

    /// `4k7` in a known unit shows its refusal as it is typed.
    #[test]
    fn a_prefix_typed_into_the_number_is_flagged_at_once() {
        let mut modal = modal_with(canonical_kilo_ohms());
        modal
            .param_buf
            .insert("value".into(), ("4k7".into(), "ohm".into()));
        assert!(number_field_problem(&modal, "value", None).is_some());
        modal
            .param_buf
            .insert("value".into(), ("4.7".into(), "ohm".into()));
        assert!(number_field_problem(&modal, "value", None).is_none());
    }

    /// A measurement-slot cell whose key is missing or holds text never
    /// commits the buffer as text inline; it is refused.
    #[test]
    fn a_slot_cell_without_a_measurement_is_refused_inline() {
        assert!(param_value_for_commit(None, "10", true).is_err());
        assert!(param_value_for_commit(Some(&ParamValue::Text("x".into())), "10", true).is_err());
        assert!(param_value_for_commit(None, "10", false).is_ok());
    }

    /// `4k7` is flagged as typed even when the buffer holds no unit (a slot
    /// stored as a plain number), because the slot supplies it.
    #[test]
    fn a_prefix_typed_into_a_unitless_slot_buffer_is_flagged() {
        let t = template(slot("value", "ohm"));
        let mut modal = modal_with(ParamValue::Number(4.7));
        modal
            .param_buf
            .insert("value".into(), ("4k7".into(), String::new()));
        assert!(number_field_problem(&modal, "value", Some(&t)).is_some());
        assert!(number_field_problem(&modal, "value", None).is_none());
    }

    /// Enter on a slot value the field cannot express rewrites nothing.
    #[test]
    fn modal_enter_on_a_slot_value_the_field_cannot_express_rewrites_nothing() {
        let cases = [
            (
                ParamValue::Measurement {
                    value: 4.7,
                    unit: "meg".into(),
                },
                slot("value", "ohm"),
            ),
            (
                ParamValue::Measurement {
                    value: 4.7,
                    unit: "kV".into(),
                },
                slot("value", "F"),
            ),
            (ParamValue::Number(4.7), slot("value", "ohm")),
        ];
        for (stored, slot) in cases {
            let t = template(slot);
            let mut modal = modal_with(stored.clone());
            commit_param_buffer(&mut modal, "value", Some(&t));
            assert_eq!(modal.draft.parameters["value"], stored);
            assert!(modal.error.is_none(), "{stored:?}: {:?}", modal.error);
        }
    }

    /// Changing a free parameter's unit says the value is reinterpreted.
    #[test]
    fn a_free_unit_change_is_noted() {
        let mut modal = modal_with(canonical_kilo_ohms());
        pick_unit(&mut modal, "value", "%".into(), None);
        assert!(modal.error.is_none(), "a notice is not a refusal");
        let note = modal.note.expect("a notice");
        assert!(note.contains("reinterpreted"), "{note}");
    }

    /// The modal stores the same for the same action: picking k on a slot
    /// holding a plain number or an unknown unit.
    #[test]
    fn the_modal_stores_a_prefix_pick_on_a_slot_number_or_unknown_unit() {
        let t = template(slot("value", "ohm"));
        for stored in [
            ParamValue::Number(4.7),
            ParamValue::Measurement {
                value: 4.7,
                unit: "meg".into(),
            },
        ] {
            let mut modal = modal_with(stored.clone());
            modal.param_prefix.insert("value".into(), SiPrefix::Kilo);
            commit_param_buffer(&mut modal, "value", Some(&t));
            assert_eq!(
                modal.draft.parameters["value"],
                canonical_kilo_ohms(),
                "{stored:?}"
            );
        }
    }
}
