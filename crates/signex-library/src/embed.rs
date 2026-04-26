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

/// Symbol body — pin list, drawing primitives, anchor.
/// Phase 0 stores opaque S-expression text; Phase 1 promotes to typed AST
/// once the symbol editor lands.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct SymbolBody {
    /// KiCad-format symbol body (S-expression text).
    pub sexpr: String,
}

/// Footprint body — pads, courtyard, silk, fab.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct FootprintBody {
    pub sexpr: String,
}

/// Pointer to an external 3D model file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelRef {
    pub path: String,
    pub offset: [f64; 3],
    pub rotation: [f64; 3],
}

/// Schematic-only embed slice — used in `.snxsch` and the Component Editor.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct SchematicSide {
    pub symbol: SymbolBody,
    #[serde(default)]
    pub schematic_params: ParamMap,
}

/// PCB-only embed slice — used in `.snxpcb` and the Component Editor.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct PcbSide {
    pub footprint: FootprintBody,
    #[serde(default)]
    pub model_3d: Option<ModelRef>,
    #[serde(default)]
    pub pcb_params: ParamMap,
}

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
    fn schematic_side_default_is_round_trippable() {
        let s = SchematicSide::default();
        let json = serde_json::to_string(&s).unwrap();
        let back: SchematicSide = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn pcb_side_with_model_round_trips() {
        let p = PcbSide {
            footprint: FootprintBody {
                sexpr: "(footprint R0805 ...)".into(),
            },
            model_3d: Some(ModelRef {
                path: "shared/3d-models/abc.step".into(),
                offset: [0.0, 0.0, 0.5],
                rotation: [0.0, 0.0, 90.0],
            }),
            pcb_params: ParamMap::new(),
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: PcbSide = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }
}
