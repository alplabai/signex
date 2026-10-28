//! Generic parameter map shared by primitives, components, and templates.
//!
//! `ParamMap = BTreeMap<String, ParamValue>` keeps keys sorted so JSON
//! serialisation is deterministic — important for content-hashing and diffing.

use serde::{Deserialize, Serialize};
use signex_types::quantity::{Quantity, ValueStyle, format_quantity, quantity_from_stored};
use std::collections::BTreeMap;

/// One parameter cell. String / number / bool / measurement (value + unit).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum ParamValue {
    Text(String),
    Number(f64),
    Bool(bool),
    Measurement { value: f64, unit: String },
}

impl ParamValue {
    /// Render the value as a human-readable string. Used by the diff formatter
    /// and the parametric search index.
    pub fn display(&self) -> String {
        match self {
            ParamValue::Text(s) => s.clone(),
            ParamValue::Number(n) => n.to_string(),
            ParamValue::Bool(b) => b.to_string(),
            ParamValue::Measurement { value, unit } => match self.as_quantity() {
                Some(q) => format_quantity(&q, ValueStyle::Engineering, false),
                None => format!("{value} {unit}"),
            },
        }
    }

    /// A measurement as a base-unit [`Quantity`]. Reads the canonical
    /// `{4700, "ohm"}` and the legacy prefixed `{4.7, "kΩ"}` as the same
    /// 4700 Ω; an unknown unit string is kept as written. `None` for other
    /// variants and for a non-finite value. New sort, filter, diff and
    /// grouping code should read the value through this, never the raw
    /// `value`. Not yet adopted: the search index (`search_index.rs`) still
    /// indexes the raw `value`; that and the sort/filter paths move over in
    /// the base-unit sort and range-filter phases.
    pub fn as_quantity(&self) -> Option<Quantity> {
        match self {
            ParamValue::Measurement { value, unit } => quantity_from_stored(*value, unit),
            _ => None,
        }
    }
}

/// Sorted-key parameter map. Determinism on serialisation is load-bearing for
/// the content hash and the diff engine.
pub type ParamMap = BTreeMap<String, ParamValue>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn param_value_round_trip_each_variant() {
        let cases = [
            ParamValue::Text("X7R".into()),
            ParamValue::Number(10.5),
            ParamValue::Bool(true),
            ParamValue::Measurement {
                value: 25.0,
                unit: "V".into(),
            },
        ];
        for v in cases {
            let json = serde_json::to_string(&v).unwrap();
            let back: ParamValue = serde_json::from_str(&json).unwrap();
            assert_eq!(v, back);
        }
    }

    #[test]
    fn legacy_and_canonical_measurements_are_one_quantity() {
        let legacy = ParamValue::Measurement {
            value: 4.7,
            unit: "kΩ".into(),
        };
        let canonical = ParamValue::Measurement {
            value: 4700.0,
            unit: "ohm".into(),
        };
        assert_eq!(legacy.as_quantity().unwrap().base(), 4700.0);
        assert_eq!(legacy.as_quantity(), canonical.as_quantity());
        assert_eq!(legacy.display(), "4.7 kΩ");
        assert_eq!(canonical.display(), "4.7 kΩ");
        assert_eq!(ParamValue::Text("x".into()).as_quantity(), None);
        let unknown = ParamValue::Measurement {
            value: 1.5,
            unit: "furlong".into(),
        };
        assert_eq!(unknown.display(), "1.5 furlong");
    }

    #[test]
    fn param_value_display_each_variant() {
        assert_eq!(ParamValue::Text("X7R".into()).display(), "X7R");
        assert_eq!(ParamValue::Number(1.5).display(), "1.5");
        assert_eq!(ParamValue::Bool(true).display(), "true");
        assert_eq!(
            ParamValue::Measurement {
                value: 25.0,
                unit: "V".into()
            }
            .display(),
            "25 V"
        );
    }
}
