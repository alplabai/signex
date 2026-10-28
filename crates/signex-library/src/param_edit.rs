//! What an editor shows and stores for a measurement parameter.
//!
//! Editors present a measurement as a number field, an SI-prefix picker
//! and a fixed unit ([`ParamValue::measurement_parts`]) and store the
//! canonical `{base value, unit token}` ([`commit_measurement`]). Users
//! never type a prefix or a unit into the number field.

use signex_types::quantity::{
    MantissaError, SiPrefix, Unit, ValueStyle, format_quantity, quantity_from_parts,
};

use crate::param::ParamValue;

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

/// The value an editor stores for `mantissa` in `prefix` of `unit`.
///
/// A commit that leaves the base value unchanged at display precision
/// (compared as formatted strings, not bits) returns `existing` as it is,
/// so an untouched row keeps its bytes and its content hash, and a legacy
/// `{4.7, "kΩ"}` is only rewritten when the user really changes it.
/// Otherwise the canonical `{base, unit token}` is stored.
pub fn commit_measurement(
    existing: Option<&ParamValue>,
    mantissa: &str,
    prefix: SiPrefix,
    unit: &Unit,
) -> Result<ParamValue, MantissaError> {
    let typed = quantity_from_parts(mantissa, prefix, unit.clone())?;
    if let Some(old) = existing.and_then(|v| v.as_quantity().map(|q| (v, q)))
        && old.1.unit() == typed.unit()
        && format_quantity(&old.1, ValueStyle::Engineering, false)
            == format_quantity(&typed, ValueStyle::Engineering, false)
    {
        return Ok(old.0.clone());
    }
    Ok(ParamValue::Measurement {
        value: typed.base(),
        unit: typed.unit().token().to_string(),
    })
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
}
