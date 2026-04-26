//! Schematic / PCB / shared embed types per LIBRARY_PLAN §4–§5.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Generic parameter cell. String/number/bool/measurement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum ParamValue {
    Text(String),
    Number(f64),
    Bool(bool),
    Measurement { value: f64, unit: String },
}

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
}
