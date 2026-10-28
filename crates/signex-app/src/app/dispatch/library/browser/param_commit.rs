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

use signex_library::{ParamValue, commit_measurement};
use signex_types::numeric::parse_finite_f64;
use signex_types::quantity::{SiPrefix, Unit};

use crate::library::prefix_picker::effective_prefix;

/// What a measurement cell accepts, for the refusal report.
const EXPECTED_NUMBER: &str = "a plain number; the prefix and unit are kept";

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
) -> Result<ParamValue, ParamRefusal> {
    match existing {
        // A measurement in a known unit is read as a number in the prefix
        // it displays (`4.7` for 4.7 kΩ) and stored canonical
        // (`{4700, "ohm"}`); an unchanged number keeps the stored value
        // as it is. The grid opens measurement cells in the edit modal,
        // which has the prefix picker, so this is the commit of a buffer
        // that is already in the cell. An unknown unit string is kept as
        // written next to the typed number.
        Some(existing_value @ ParamValue::Measurement { unit, .. }) => {
            match existing_value.measurement_parts() {
                Some(parts) => {
                    commit_measurement(Some(existing_value), buf, parts.prefix, &parts.unit)
                        .map_err(|_| refusal(EXPECTED_NUMBER, existing))
                }
                None => parse_finite_f64(buf)
                    .map(|value| ParamValue::Measurement {
                        value,
                        unit: unit.clone(),
                    })
                    .map_err(|_| refusal(EXPECTED_NUMBER, existing)),
            }
        }
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

/// What the edit modal's "commit parameter" stores for a typed `value`,
/// the picked `prefix` and `unit`. `Err` carries the reason the entry was
/// refused, and the draft must stay as it is.
///
/// A known unit makes the value a measurement: `value` is a plain number
/// (no prefix or unit typed into it) in `prefix`, stored canonical as
/// `{base, unit token}`, and an unchanged number keeps `existing` as it
/// is. A unit string that is not a known unit keeps the raw number and
/// its string. A `nan` / `inf` / overflowing entry is refused either way.
pub(super) fn modal_param_value(
    value: String,
    unit: &str,
    prefix: SiPrefix,
    existing: Option<&ParamValue>,
) -> Result<ParamValue, String> {
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
pub(super) fn commit_param_buffer(modal: &mut crate::library::state::EditRowModalState, key: &str) {
    let Some((value, unit)) = modal.param_buf.get(key).cloned() else {
        return;
    };
    let prefix = modal
        .param_prefix
        .get(key)
        .copied()
        .unwrap_or(SiPrefix::One);
    match modal_param_value(value, &unit, prefix, modal.draft.parameters.get(key)) {
        Ok(pv) => {
            modal.draft.parameters.insert(key.to_string(), pv);
            modal.error = None;
        }
        Err(reason) => modal.error = Some(format!("Parameter \"{key}\": {reason}.")),
    }
}

/// Every buffered parameter edit that was typed but never committed
/// (Enter), as the value `CommitParam` would store. `Err((key, reason))`
/// names the first entry that is refused, so Save can stop instead of
/// dropping it. `tags` is skipped: its own buffer is flushed separately.
pub(super) fn uncommitted_param_edits(
    draft: &signex_library::ParamMap,
    buf: &std::collections::HashMap<String, (String, String)>,
    prefixes: &std::collections::HashMap<String, SiPrefix>,
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

    #[test]
    fn a_legacy_measurement_cell_stores_the_base_value_in_the_unit_it_shows() {
        // Arrange — the cell shows `4.7 kΩ`, so `10` is 10 kΩ.
        let existing = kilo_ohms();

        // Act
        let committed = param_value_for_commit(Some(&existing), "10");

        // Assert — canonical, never `{10, "kΩ"}` and never 10 Ω.
        assert_eq!(
            committed.ok(),
            Some(ParamValue::Measurement {
                value: 10_000.0,
                unit: "ohm".to_string(),
            })
        );
    }

    #[test]
    fn an_unchanged_measurement_buffer_is_a_no_op() {
        // Arrange
        for existing in [kilo_ohms(), canonical_kilo_ohms()] {
            // Act — the cell's own seed, committed again.
            let parts = existing.measurement_parts().unwrap();
            let committed = param_value_for_commit(Some(&existing), &parts.mantissa);

            // Assert — the stored bytes (and so the row hash) are kept.
            assert_eq!(committed.ok(), Some(existing));
        }
    }

    #[test]
    fn an_unknown_unit_keeps_its_string() {
        let existing = ParamValue::Measurement {
            value: 1.5,
            unit: "furlong".to_string(),
        };
        assert_eq!(
            param_value_for_commit(Some(&existing), "2").ok(),
            Some(ParamValue::Measurement {
                value: 2.0,
                unit: "furlong".to_string(),
            })
        );
    }

    #[test]
    fn a_measurement_cell_refuses_a_buffer_that_is_not_a_number() {
        // Arrange — `4k7` is the engineering shorthand the register
        // named; it is not an `f64` and must not retype the parameter.
        let existing = kilo_ohms();

        // Act
        let committed = param_value_for_commit(Some(&existing), "4k7");

        // Assert
        let refusal = committed.expect_err("an unparseable buffer must be refused");
        assert_eq!(refusal.kept, "4.7 kΩ");
        assert_eq!(refusal.expected, EXPECTED_NUMBER);
    }

    #[test]
    fn a_number_cell_refuses_a_buffer_that_is_not_a_number() {
        // Arrange
        let existing = ParamValue::Number(3.0);

        // Act
        let committed = param_value_for_commit(Some(&existing), "3 pieces");

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
        let committed = param_value_for_commit(Some(&existing), "12.5");

        // Assert
        assert_eq!(committed.ok(), Some(ParamValue::Number(12.5)));
    }

    #[test]
    fn an_untyped_cell_takes_the_buffer_verbatim() {
        // Arrange / Act
        let fresh = param_value_for_commit(None, "4k7");
        let text = param_value_for_commit(Some(&ParamValue::Text("old".into())), "4k7");

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
        let committed = param_value_for_commit(Some(&existing), "maybe");

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
                param_value_for_commit(Some(&existing), buf).ok(),
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
        let committed = param_value_for_commit(None, "maybe");

        // Assert
        assert_eq!(committed.ok(), Some(ParamValue::Text("maybe".to_string())));
    }

    const NON_FINITE: [&str; 5] = ["nan", "inf", "-inf", "1e999", "-1e999"];

    /// #828 -- a typed numeric cell refuses a non-finite buffer.
    #[test]
    fn typed_cells_refuse_non_finite_buffers() {
        for bad in NON_FINITE {
            let number = param_value_for_commit(Some(&ParamValue::Number(3.0)), bad);
            assert!(number.is_err(), "number cell took {bad:?}");
            let measurement = param_value_for_commit(Some(&kilo_ohms()), bad);
            assert!(measurement.is_err(), "measurement cell took {bad:?}");
        }
    }

    /// #828 -- the edit modal never stores a non-finite number, with or
    /// without a unit, and tells the caller to keep the draft.
    #[test]
    fn the_modal_refuses_non_finite_entries() {
        for bad in NON_FINITE {
            assert!(modal_param_value(bad.to_string(), "", SiPrefix::One, None).is_err());
            assert!(modal_param_value(bad.to_string(), "V", SiPrefix::One, None).is_err());
        }
    }

    #[test]
    fn the_modal_still_types_ordinary_entries() {
        let one = SiPrefix::One;
        assert_eq!(
            modal_param_value("2.5".into(), "", one, None),
            Ok(ParamValue::Number(2.5))
        );
        assert_eq!(
            modal_param_value("2.5".into(), "V", one, None),
            Ok(ParamValue::Measurement {
                value: 2.5,
                unit: "V".into()
            })
        );
        assert_eq!(
            modal_param_value("TRUE".into(), "", one, None),
            Ok(ParamValue::Bool(true))
        );
        assert_eq!(
            modal_param_value("abc".into(), "furlong", one, None),
            Ok(ParamValue::Text("abc furlong".into()))
        );
    }

    /// The number field takes a number, not `4k7` or `abc`, in a known unit.
    #[test]
    fn the_modal_refuses_letters_in_a_known_unit() {
        for typed in ["4k7", "10k", "4.7uF", "abc"] {
            assert!(
                modal_param_value(typed.into(), "ohm", SiPrefix::One, None).is_err(),
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
            modal_param_value("4.7".into(), "ohm", kilo, None),
            Ok(want.clone())
        );
        // #1606: a legacy `{4.7, "kΩ"}` edited to 5.6 + k is 5600 ohm,
        // not 5.6 ohm; the same 4.7 + k is no change at all.
        assert_eq!(
            modal_param_value("5.6".into(), "ohm", kilo, Some(&kilo_ohms())),
            Ok(ParamValue::Measurement {
                value: 5600.0,
                unit: "ohm".into()
            })
        );
        assert_eq!(
            modal_param_value("4.7".into(), "ohm", kilo, Some(&kilo_ohms())),
            Ok(kilo_ohms())
        );
    }

    /// A unit that takes no prefix ignores a stale picked prefix.
    #[test]
    fn the_modal_ignores_a_prefix_for_percent() {
        assert_eq!(
            modal_param_value("1".into(), "%", SiPrefix::Kilo, None),
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
        let edits = uncommitted_param_edits(&draft, &buf, &prefixes).unwrap();
        assert!(edits.is_empty());
    }
}
