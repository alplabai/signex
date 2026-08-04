//! Properties-panel numeric edits that store into an `Option<f64>`.
//!
//! GH #599 — the pad form's `corner_radius_pct` and `hole_rotation_deg`
//! rows used to commit with `parse::<f64>().ok().filter(range)`, which
//! maps three different user intentions onto one `None`:
//!
//!   * the field was emptied      — "clear this value"
//!   * the text is not a number   — "I am mid-edit / I mistyped"
//!   * the number is out of range — "I meant a value near the limit"
//!
//! That single `None` was then written straight over whatever was
//! stored, so a pad at 25 % corner radius edited to `60` lost the 25.
//! `corner_radius_pct` is a persisted UI mirror rather than geometry
//! (`signex-library/src/primitive/footprint/pad.rs` documents that), and
//! it is serialised with `skip_serializing_if = "Option::is_none"`, so
//! clearing it drops the key from the saved `.snxfpt` outright — the
//! value no longer survives a shape switch and back.
//!
//! These helpers keep the three intentions apart and report the two the
//! user did not ask for.

/// Bounds of the pad corner-radius percentage field. The radius is a
/// fraction of the pad's short side, so 50 % is a fully rounded end and
/// nothing above it is expressible.
pub(super) const CORNER_RADIUS_PCT_MIN: f64 = 0.0;
pub(super) const CORNER_RADIUS_PCT_MAX: f64 = 50.0;

/// What the user's text actually asked for.
pub(super) enum OptionalNumberEdit {
    /// The field was emptied. Storing `None` is the request.
    Cleared,
    /// A usable, finite number.
    Accepted(f64),
    /// A finite number outside the field's range, pulled to the nearest
    /// bound. The user's intent is kept rather than erased.
    Clamped { stored: f64, typed: f64 },
    /// Not a number at all — `1,5` from a comma-decimal keyboard,
    /// `1.5mm` with the unit spelled out, a lone `-`, or `inf`. The
    /// stored value has to survive it.
    Unreadable,
}

/// Read an unbounded optional-number field (`hole_rotation_deg`).
pub(super) fn fp_parse_optional_number(value: &str) -> OptionalNumberEdit {
    let text = value.trim();
    if text.is_empty() {
        return OptionalNumberEdit::Cleared;
    }
    match text.parse::<f64>() {
        // `inf` / `-inf` / `NaN` all parse. None of them is a rotation
        // or a percentage, and storing one would poison every later
        // comparison, so they join the unreadable pile.
        Ok(parsed) if parsed.is_finite() => OptionalNumberEdit::Accepted(parsed),
        _ => OptionalNumberEdit::Unreadable,
    }
}

/// Read a range-limited optional-number field (`corner_radius_pct`).
/// Out-of-range clamps to the nearest bound instead of clearing.
pub(super) fn fp_parse_optional_number_in(value: &str, min: f64, max: f64) -> OptionalNumberEdit {
    match fp_parse_optional_number(value) {
        OptionalNumberEdit::Accepted(typed) if !(min..=max).contains(&typed) => {
            OptionalNumberEdit::Clamped {
                stored: typed.clamp(min, max),
                typed,
            }
        }
        other => other,
    }
}

/// Resolve a parsed edit into what to store, reporting anything the
/// user asked for and did not get.
///
/// `None` means **do not write**: the text is not a number, so the
/// caller must leave the stored value alone. `Some(slot)` is the new
/// contents of the `Option<f64>`.
///
/// Levels follow the two outcomes: a clamp is a degraded path that
/// still stored something (`warn!`), a refused edit withheld the user's
/// change outright (`error!`). Neither may be `debug!` — the default
/// filter is `LevelFilter::Info` (`crate::diagnostics`), so a `debug!`
/// record never reaches the Messages panel.
pub(super) fn fp_resolve_optional_number(
    field: &'static str,
    value: &str,
    edit: OptionalNumberEdit,
) -> Option<Option<f64>> {
    match edit {
        OptionalNumberEdit::Cleared => Some(None),
        OptionalNumberEdit::Accepted(number) => Some(Some(number)),
        OptionalNumberEdit::Clamped { stored, typed } => {
            tracing::warn!(
                target: "signex::footprint_pad",
                field = field,
                typed = typed,
                stored = stored,
                "pad field is outside its allowed range; the nearest allowed value was stored"
            );
            Some(Some(stored))
        }
        OptionalNumberEdit::Unreadable => {
            tracing::error!(
                target: "signex::footprint_pad",
                field = field,
                buffer = value.trim(),
                "pad field edit is not a number; the edit was refused and the stored value kept"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(field: &'static str, value: &str, edit: OptionalNumberEdit) -> Option<Option<f64>> {
        fp_resolve_optional_number(field, value, edit)
    }

    #[test]
    fn empty_field_clears_the_stored_value() {
        let edit = fp_parse_optional_number_in("   ", CORNER_RADIUS_PCT_MIN, CORNER_RADIUS_PCT_MAX);
        assert!(matches!(edit, OptionalNumberEdit::Cleared));
        assert_eq!(resolve("Corner radius %", "   ", edit), Some(None));
    }

    #[test]
    fn in_range_number_is_stored_as_typed() {
        let edit = fp_parse_optional_number_in("25", CORNER_RADIUS_PCT_MIN, CORNER_RADIUS_PCT_MAX);
        assert_eq!(resolve("Corner radius %", "25", edit), Some(Some(25.0)));
    }

    // #599 — `60` used to erase the stored 25 and drop the key from the
    // saved `.snxfpt`. It must land on the nearest expressible value.
    #[test]
    fn out_of_range_number_clamps_instead_of_clearing() {
        let edit = fp_parse_optional_number_in("60", CORNER_RADIUS_PCT_MIN, CORNER_RADIUS_PCT_MAX);
        assert!(matches!(
            edit,
            OptionalNumberEdit::Clamped {
                stored: 50.0,
                typed: 60.0
            }
        ));
        assert_eq!(resolve("Corner radius %", "60", edit), Some(Some(50.0)));
    }

    // #599 — the comma-decimal keyboard case. Refusing the write is the
    // whole point: `None` here would have overwritten the stored value.
    #[test]
    fn unreadable_text_refuses_the_write() {
        for typed in ["1,5", "1.5mm", "-", "abc"] {
            let edit =
                fp_parse_optional_number_in(typed, CORNER_RADIUS_PCT_MIN, CORNER_RADIUS_PCT_MAX);
            assert!(
                matches!(edit, OptionalNumberEdit::Unreadable),
                "{typed} should be unreadable"
            );
            assert_eq!(
                resolve("Corner radius %", typed, edit),
                None,
                "{typed} must not be written"
            );
        }
    }

    #[test]
    fn non_finite_numbers_are_unreadable() {
        for typed in ["inf", "-inf", "NaN"] {
            let edit = fp_parse_optional_number(typed);
            assert!(
                matches!(edit, OptionalNumberEdit::Unreadable),
                "{typed} should be unreadable"
            );
        }
    }

    // The unbounded rotation field keeps every finite value, including
    // negatives — it has no range filter and must not grow one here.
    #[test]
    fn unbounded_field_keeps_negative_and_large_values() {
        assert_eq!(
            resolve(
                "Hole rotation (deg)",
                "-45",
                fp_parse_optional_number("-45")
            ),
            Some(Some(-45.0))
        );
        assert_eq!(
            resolve(
                "Hole rotation (deg)",
                "720",
                fp_parse_optional_number("720")
            ),
            Some(Some(720.0))
        );
    }
}
