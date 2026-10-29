//! What an editor shows and stores for a measurement parameter.
//!
//! Editors present a measurement as a number field, an SI-prefix picker
//! and a fixed unit ([`ParamValue::measurement_parts`]) and store the
//! canonical `{base value, unit token}` ([`commit_measurement`]). Users
//! never type a prefix or a unit into the number field.

use signex_types::quantity::{MantissaError, SiPrefix, Unit, quantity_from_parts};

use crate::param::ParamValue;
use crate::templates::{ParamSlot, TemplateViolation, slot_violations};

/// The three parts an editor shows for a stored measurement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasurementParts {
    /// Number-field text, in the prefix below (`4.7` for 4700 Ω).
    pub mantissa: String,
    /// The prefix the value displays with; the picker starts here.
    pub prefix: SiPrefix,
    pub unit: Unit,
}

impl ParamValue {
    /// A measurement as number field + prefix + unit, reading the
    /// canonical `{4700, "ohm"}` and the legacy `{4.7, "kΩ"}` alike. `None`
    /// for other variants and for a measurement whose unit string is not
    /// a known unit ([`Unit::Other`]): that one keeps its raw text editor
    /// and its string.
    pub fn measurement_parts(&self) -> Option<MeasurementParts> {
        let q = self.as_quantity()?;
        if matches!(q.unit(), Unit::Other(_)) {
            return None;
        }
        let (mantissa, prefix) = q.eng_parts();
        Some(MeasurementParts {
            mantissa,
            prefix,
            unit: q.unit().clone(),
        })
    }
}

/// Whether the field still holds what it was seeded with from `existing`:
/// the same number text in the same prefix.
fn field_unchanged(existing: Option<&ParamValue>, mantissa: &str, prefix: SiPrefix) -> bool {
    existing
        .and_then(ParamValue::measurement_parts)
        .is_some_and(|p| p.mantissa == mantissa.trim() && p.prefix == prefix)
}

/// The value an editor stores for `mantissa` in `prefix` of `unit`.
///
/// A commit that leaves the stored value as it is returns `existing`
/// untouched, so an untouched row keeps its bytes and its content hash and
/// a legacy `{4.7, "kΩ"}` is only rewritten when the user really changes
/// it. That is the case when the typed number is the same quantity (the
/// stored base value, compared exactly, not through a rounded display),
/// or when the field still holds the text it was seeded with (the seed is
/// rounded to 15 digits, a 16 to 17 digit value must survive a retype). A
/// typed number that differs, even past the 15th digit, is stored as the
/// canonical `{base, unit token}`.
pub fn commit_measurement(
    existing: Option<&ParamValue>,
    mantissa: &str,
    prefix: SiPrefix,
    unit: &Unit,
) -> Result<ParamValue, MantissaError> {
    let typed = quantity_from_parts(mantissa, prefix, unit.clone())?;
    if let Some(old) = existing {
        let same_unit = old.as_quantity().is_some_and(|q| q.unit() == typed.unit());
        if same_unit
            && (old.as_quantity().as_ref() == Some(&typed)
                || field_unchanged(existing, mantissa, prefix))
        {
            return Ok(old.clone());
        }
    }
    Ok(ParamValue::Measurement {
        value: typed.base(),
        unit: typed.unit().token().to_string(),
    })
}

/// The known unit a template slot fixes, if it fixes one.
pub fn slot_unit(slot: &ParamSlot) -> Option<Unit> {
    slot.unit
        .as_deref()
        .and_then(|u| Unit::from_alias(u.trim()))
}

/// Why `value` does not suit `slot` (zero, negative or wrong unit), in the
/// words an editor shows next to the field.
pub fn slot_problem(slot: &ParamSlot, value: &ParamValue) -> Option<String> {
    slot_violations(slot, value)
        .iter()
        .find_map(violation_words)
}

/// The words for one violation; a missing required value has its own badge.
pub fn violation_words(v: &TemplateViolation) -> Option<String> {
    match v {
        TemplateViolation::MissingRequired { .. } => None,
        TemplateViolation::WrongKind { expected, .. } => Some(format!("expects {expected:?}")),
        TemplateViolation::WrongUnit {
            expected, found, ..
        } => Some(format!("unit {found}, expected {expected}")),
        TemplateViolation::ZeroNotAllowed { .. } => Some("zero not allowed".to_string()),
        TemplateViolation::NegativeNotAllowed { .. } => Some("negative not allowed".to_string()),
    }
}

/// What a measurement slot's field shows for `value`: the number text and
/// the prefix its picker shows. A measurement in a known unit shows its
/// engineering number and prefix (the prefix falls back to none where the
/// slot unit lacks it: `{4.7, "kV"}` on a farad slot shows `4.7` with no
/// prefix); a unit the editor does not know, a plain number and text show
/// the stored number as written, with no prefix.
pub fn slot_field_seed(value: &ParamValue, slot_unit: &Unit) -> (String, SiPrefix) {
    if let Some(parts) = value.measurement_parts() {
        let prefix = if slot_unit.allowed_prefixes().contains(&parts.prefix) {
            parts.prefix
        } else {
            SiPrefix::One
        };
        return (parts.mantissa, prefix);
    }
    let text = match value {
        ParamValue::Measurement { value, .. } => value.to_string(),
        ParamValue::Number(n) => n.to_string(),
        other => other.display(),
    };
    (text, SiPrefix::One)
}

/// [`commit_measurement`] for a parameter whose class template has a
/// measurement `slot` with a known unit: the slot fixes the unit, and a
/// typed value that breaks the slot (zero or negative where the slot does
/// not allow it) is refused with the reason. The edit is not stored.
///
/// A field still showing what it was seeded with ([`slot_field_seed`]: the
/// text and prefix the cell showed) is a no-op, whatever the stored value
/// is: a legacy wrong-unit, zero, unknown-unit or plain-number value stays
/// exactly as it is until the user edits the field. Nothing is rewritten
/// and nothing is refused.
pub fn commit_slot_measurement(
    existing: Option<&ParamValue>,
    mantissa: &str,
    prefix: SiPrefix,
    slot: &ParamSlot,
) -> Result<ParamValue, String> {
    let unit = slot_unit(slot).ok_or_else(|| format!("{} has no known unit", slot.name))?;
    let prefix = if unit.allowed_prefixes().contains(&prefix) {
        prefix
    } else {
        SiPrefix::One
    };
    if let Some(old) = existing
        && slot_field_seed(old, &unit) == (mantissa.trim().to_string(), prefix)
    {
        return Ok(old.clone());
    }
    let value = commit_measurement(existing, mantissa, prefix, &unit).map_err(|e| e.to_string())?;
    if existing == Some(&value) {
        return Ok(value);
    }
    match slot_problem(slot, &value) {
        Some(why) => Err(format!("{why}; the value is not stored")),
        None => Ok(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meas(value: f64, unit: &str) -> ParamValue {
        ParamValue::Measurement {
            value,
            unit: unit.into(),
        }
    }

    #[test]
    fn legacy_and_canonical_open_as_the_same_number_prefix_and_unit() {
        let want = MeasurementParts {
            mantissa: "4.7".into(),
            prefix: SiPrefix::Kilo,
            unit: Unit::Ohm,
        };
        assert_eq!(meas(4.7, "kΩ").measurement_parts(), Some(want.clone()));
        assert_eq!(meas(4700.0, "ohm").measurement_parts(), Some(want));
    }

    #[test]
    fn an_unknown_unit_has_no_parts_and_keeps_its_text_editor() {
        assert_eq!(meas(1.5, "furlong").measurement_parts(), None);
        assert_eq!(ParamValue::Text("x".into()).measurement_parts(), None);
    }

    #[test]
    fn four_point_seven_kilo_stores_4700_ohm() {
        let v = commit_measurement(None, "4.7", SiPrefix::Kilo, &Unit::Ohm).unwrap();
        assert_eq!(v, meas(4700.0, "ohm"));
    }

    #[test]
    fn a_legacy_value_edited_stores_the_base_value() {
        let legacy = meas(4.7, "kΩ");
        let v = commit_measurement(Some(&legacy), "10", SiPrefix::Kilo, &Unit::Ohm).unwrap();
        assert_eq!(v, meas(10_000.0, "ohm"));
    }

    #[test]
    fn an_unchanged_commit_keeps_the_stored_value_untouched() {
        for stored in [meas(4.7, "kΩ"), meas(4700.0, "ohm")] {
            let parts = stored.measurement_parts().unwrap();
            let v = commit_measurement(Some(&stored), &parts.mantissa, parts.prefix, &parts.unit)
                .unwrap();
            assert_eq!(v, stored);
            // Same prefix-free spelling of the same value is also a no-op.
            let v = commit_measurement(Some(&stored), "4700", SiPrefix::One, &Unit::Ohm).unwrap();
            assert_eq!(v, stored);
        }
    }

    #[test]
    fn a_typed_prefix_letter_is_refused() {
        assert!(commit_measurement(None, "4k7", SiPrefix::One, &Unit::Ohm).is_err());
        assert!(commit_measurement(None, "4.7", SiPrefix::Kilo, &Unit::Percent).is_err());
    }

    fn slot(name: &str, unit: &str) -> ParamSlot {
        ParamSlot {
            name: name.into(),
            kind: crate::templates::ParamKind::Measurement,
            unit: Some(unit.into()),
            allow_zero: false,
            allow_negative: false,
            display_plus_minus: false,
        }
    }

    /// An edit past the 15th digit is a real edit, not a display no-op.
    #[test]
    fn an_edit_past_the_fifteenth_digit_is_stored() {
        let stored = meas(1.0000000000000002, "ohm");
        let v = commit_measurement(
            Some(&stored),
            "1.0000000000000004",
            SiPrefix::One,
            &Unit::Ohm,
        )
        .unwrap();
        assert_eq!(v, meas(1.0000000000000004, "ohm"));
    }

    /// Retyping the seed of a 17-digit value (rounded to 15 digits in the
    /// field) leaves the stored value alone.
    #[test]
    fn retyping_the_seed_of_a_long_value_is_a_no_op() {
        let stored = meas(1.0000000000000002, "ohm");
        let parts = stored.measurement_parts().unwrap();
        let v =
            commit_measurement(Some(&stored), &parts.mantissa, parts.prefix, &parts.unit).unwrap();
        assert_eq!(v, stored);
    }

    #[test]
    fn a_slot_refuses_zero_and_negative_and_says_why() {
        let cap = slot("value", "F");
        let err = commit_slot_measurement(None, "0", SiPrefix::One, &cap).unwrap_err();
        assert!(err.contains("zero not allowed"), "{err}");
        let res = slot("value", "ohm");
        let err = commit_slot_measurement(None, "-4.7", SiPrefix::Kilo, &res).unwrap_err();
        assert!(err.contains("negative not allowed"), "{err}");
        // A slot that allows it takes it, and a resistor takes zero.
        let mut offset = slot("offset", "V");
        offset.allow_negative = true;
        assert_eq!(
            commit_slot_measurement(None, "-5", SiPrefix::Milli, &offset).unwrap(),
            meas(-0.005, "V")
        );
        assert_eq!(
            commit_slot_measurement(None, "0", SiPrefix::One, &res).unwrap(),
            meas(0.0, "ohm")
        );
    }

    /// A legacy wrong-unit value stays as it is until it is edited; an
    /// edit stores the number in the slot's unit.
    #[test]
    fn a_wrong_unit_legacy_value_is_kept_until_edited() {
        let res = slot("value", "ohm");
        let stored = meas(4.7, "V");
        let parts = stored.measurement_parts().unwrap();
        let kept =
            commit_slot_measurement(Some(&stored), &parts.mantissa, parts.prefix, &res).unwrap();
        assert_eq!(kept, stored);
        let fixed = commit_slot_measurement(Some(&stored), "5.6", SiPrefix::Kilo, &res).unwrap();
        assert_eq!(fixed, meas(5600.0, "ohm"));
    }

    /// A zero legacy capacitor is not refused when left alone.
    #[test]
    fn a_zero_legacy_value_is_kept_when_untouched() {
        let cap = slot("value", "F");
        let stored = meas(0.0, "F");
        let parts = stored.measurement_parts().unwrap();
        let kept =
            commit_slot_measurement(Some(&stored), &parts.mantissa, parts.prefix, &cap).unwrap();
        assert_eq!(kept, stored);
    }

    #[test]
    fn an_overflowing_number_is_refused() {
        let res = slot("value", "ohm");
        assert!(commit_slot_measurement(None, "1e308", SiPrefix::Mega, &res).is_err());
    }

    /// Enter on a slot value the field cannot express is a no-op: an
    /// unknown unit, a prefix the slot unit lacks, and a plain number.
    #[test]
    fn an_unchanged_slot_field_keeps_values_it_cannot_express() {
        let res = slot("value", "ohm");
        let cap = slot("value", "F");
        let cases = [
            (meas(4.7, "meg"), "4.7", &res),
            (meas(4.7, "kV"), "4.7", &cap),
            (ParamValue::Number(4.7), "4.7", &res),
        ];
        for (stored, shown, slot) in cases {
            let unit = slot_unit(slot).unwrap();
            let (text, prefix) = slot_field_seed(&stored, &unit);
            assert_eq!(text, shown, "{stored:?}");
            let kept = commit_slot_measurement(Some(&stored), &text, prefix, slot).unwrap();
            assert_eq!(kept, stored, "{stored:?}");
        }
    }

    /// Editing such a value stores it in the slot's unit.
    #[test]
    fn editing_a_value_the_field_cannot_express_stores_the_slot_unit() {
        let res = slot("value", "ohm");
        let v = commit_slot_measurement(Some(&ParamValue::Number(4.7)), "10", SiPrefix::Kilo, &res)
            .unwrap();
        assert_eq!(v, meas(10_000.0, "ohm"));
    }
}
