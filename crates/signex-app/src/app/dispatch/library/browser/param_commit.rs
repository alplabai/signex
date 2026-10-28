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

use signex_library::ParamValue;
use signex_types::numeric::parse_finite_f64;

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
        Some(pv @ ParamValue::Measurement { .. }) => measurement_for_commit(pv, buf),
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

/// The cell shows a measurement through `ParamValue::display` (`4.7 kΩ`
/// for `{4700, "ohm"}`; `100 nF` for the legacy `{0.1, "uF"}`), so a
/// number typed into it is read in the prefix the user saw, not in the
/// stored unit's base. The result is stored in canonical base form
/// (`{5600, "ohm"}`); retyping the value already shown changes nothing, so
/// an untouched row keeps its stored form and content hash. A unit this
/// build does not know keeps its string and takes the number as typed.
fn measurement_for_commit(existing: &ParamValue, buf: &str) -> Result<ParamValue, ParamRefusal> {
    use signex_types::quantity::{Unit, quantity_from_parts};
    const EXPECTED: &str = "a finite number; the cell's unit is kept";
    let ParamValue::Measurement { unit, .. } = existing else {
        return Err(refusal(EXPECTED, Some(existing)));
    };
    let typed = parse_finite_f64(buf).map_err(|_| refusal(EXPECTED, Some(existing)))?;
    let known = existing
        .as_quantity()
        .filter(|q| !matches!(q.unit(), Unit::Other(_)));
    let Some(shown) = known else {
        return Ok(ParamValue::Measurement {
            value: typed,
            unit: unit.clone(),
        });
    };
    let (_, prefix) = shown.eng_parts();
    let entered = quantity_from_parts(buf, prefix, shown.unit().clone())
        .map_err(|_| refusal(EXPECTED, Some(existing)))?;
    if entered == shown {
        return Ok(existing.clone());
    }
    Ok(ParamValue::Measurement {
        value: entered.base(),
        unit: entered.unit().token().to_string(),
    })
}

fn refusal(expected: &'static str, existing: Option<&ParamValue>) -> ParamRefusal {
    ParamRefusal {
        expected,
        kept: existing.map(ParamValue::display).unwrap_or_default(),
    }
}

/// What the edit modal's "commit parameter" stores for a typed `value` and
/// `unit`. `None` means the entry was refused and the draft must stay as
/// it is: a `nan` / `inf` / overflowing entry is a deliberate-looking
/// number that would otherwise be stored as a non-finite parameter.
pub(super) fn modal_param_value(value: String, unit: &str) -> Option<ParamValue> {
    use crate::numeric_field::{FieldNumber, parse_field_number};
    let number = match parse_field_number(&value) {
        FieldNumber::Refused => return None,
        FieldNumber::Number(n) => Some(n),
        FieldNumber::NotANumber => None,
    };
    let pv = if !unit.trim().is_empty() {
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
    Some(pv)
}

/// Every buffered parameter edit that was typed but never committed
/// (Enter), as the value `CommitParam` would store. `Err(key)` names the
/// first entry that is refused, so Save can stop instead of dropping it.
/// `tags` is skipped: its own buffer is flushed separately.
pub(super) fn uncommitted_param_edits(
    draft: &signex_library::ParamMap,
    buf: &std::collections::HashMap<String, (String, String)>,
) -> Result<Vec<(String, ParamValue)>, String> {
    use crate::library::state::EditRowModalState;
    let mut keys: Vec<&String> = buf.keys().filter(|k| k.as_str() != "tags").collect();
    keys.sort();
    let mut edits = Vec::new();
    for key in keys {
        let (value, unit) = &buf[key];
        let untouched = draft.get(key).is_some_and(|pv| {
            EditRowModalState::param_buf_entry(pv) == (value.clone(), unit.clone())
        });
        if untouched {
            continue;
        }
        match modal_param_value(value.clone(), unit) {
            Some(pv) => edits.push((key.clone(), pv)),
            None => return Err(key.clone()),
        }
    }
    Ok(edits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kilo_ohms() -> ParamValue {
        ParamValue::Measurement {
            value: 4.7,
            unit: "kΩ".to_string(),
        }
    }

    #[test]
    fn a_measurement_cell_keeps_its_unit_when_the_buffer_parses() {
        // Arrange
        let existing = kilo_ohms();

        // Act
        let committed = param_value_for_commit(Some(&existing), "10");

        // Assert
        assert_eq!(
            committed.ok(),
            Some(ParamValue::Measurement {
                value: 10_000.0,
                unit: "ohm".to_string(),
            }),
            "typed in the displayed kΩ, stored as base"
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
        assert_eq!(refusal.expected, "a finite number; the cell's unit is kept");
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
            assert_eq!(modal_param_value(bad.to_string(), ""), None, "{bad:?}");
            assert_eq!(modal_param_value(bad.to_string(), "V"), None, "{bad:?}");
        }
    }

    #[test]
    fn the_modal_still_types_ordinary_entries() {
        assert_eq!(
            modal_param_value("2.5".into(), ""),
            Some(ParamValue::Number(2.5))
        );
        assert_eq!(
            modal_param_value("2.5".into(), "V"),
            Some(ParamValue::Measurement {
                value: 2.5,
                unit: "V".into()
            })
        );
        assert_eq!(
            modal_param_value("TRUE".into(), ""),
            Some(ParamValue::Bool(true))
        );
        assert_eq!(
            modal_param_value("abc".into(), "V"),
            Some(ParamValue::Text("abc V".into()))
        );
    }
}
