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

use signex_library::ParamValue;

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
        // Preserve the unit on commit by reading the existing value.
        Some(ParamValue::Measurement { unit, .. }) => buf
            .parse::<f64>()
            .map(|value| ParamValue::Measurement {
                value,
                unit: unit.clone(),
            })
            .map_err(|_| refusal("a number; the cell's unit is kept", existing)),
        Some(ParamValue::Number(_)) => buf
            .parse::<f64>()
            .map(ParamValue::Number)
            .map_err(|_| refusal("a number", existing)),
        // Unchanged by #599, which recorded the two numeric arms only:
        // a buffer that is neither "true" nor "false" still becomes
        // text. Refusing here instead would need a call on what a
        // boolean cell accepts ("1", "0", "yes", "no"), which the
        // register does not make.
        Some(ParamValue::Bool(_)) => {
            if buf.eq_ignore_ascii_case("true") {
                Ok(ParamValue::Bool(true))
            } else if buf.eq_ignore_ascii_case("false") {
                Ok(ParamValue::Bool(false))
            } else {
                Ok(ParamValue::Text(buf.to_string()))
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
                value: 10.0,
                unit: "kΩ".to_string(),
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
        assert_eq!(refusal.expected, "a number; the cell's unit is kept");
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
        assert_eq!(refusal.expected, "a number");
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
}
