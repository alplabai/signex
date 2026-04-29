//! Generic parameter map shared by primitives, components, and templates.
//!
//! `ParamMap = BTreeMap<String, ParamValue>` keeps keys sorted so JSON
//! serialisation is deterministic — important for content-hashing and diffing.

use serde::{Deserialize, Serialize};
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
            ParamValue::Measurement { value, unit } => format!("{value} {unit}"),
        }
    }
}

/// Sorted-key parameter map. Determinism on serialisation is load-bearing for
/// `hash_revision_content` and the diff engine.
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
