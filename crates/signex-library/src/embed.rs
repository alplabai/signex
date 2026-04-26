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

/// Reference to a datasheet — either remote URL or hash-pinned local PDF.
///
/// Note: `Url` variant uses a named field so the enum is serializable with
/// `tag = "kind"` (serde_json forbids tagged newtype variants over strings).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DatasheetRef {
    Url { url: String },
    HashPinned { hash: String, filename: String },
}

impl DatasheetRef {
    /// Convenience constructor mirroring the original `DatasheetRef::Url(s)` shape.
    pub fn url(s: impl Into<String>) -> Self {
        Self::Url { url: s.into() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SupplierLink {
    pub distributor: String,
    pub sku: String,
    pub url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct VariantOverride {
    #[serde(default)]
    pub fitted: Option<bool>,
    #[serde(default)]
    pub mpn: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpiceModel {
    pub body: String,
    /// pin name → SPICE node mapping
    pub pin_map: BTreeMap<String, String>,
}

/// Identifier for a parameter inheritance template (LIBRARY_PLAN §10).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TemplateId(pub String);

// ── PLM-reserved fields (LIBRARY_PLAN §14.2) ──────────────────────────
// Inert in v0.9; populated when the Plm adapter ships in v3.0.

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct AvlEntry {
    pub manufacturer: String,
    pub mpn: String,
    pub status: String, // "Approved" | "Conditional" | "Disqualified"
    pub notes: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct ComplianceTags {
    #[serde(default)]
    pub rohs: Option<String>,
    #[serde(default)]
    pub reach: Option<String>,
    #[serde(default)]
    pub other: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct PlmLink {
    #[serde(default)]
    pub plm_part_id: Option<String>,
    #[serde(default)]
    pub eco_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PricingSnapshot {
    pub captured_at: chrono::DateTime<chrono::Utc>,
    pub by_distributor: BTreeMap<String, Vec<PriceBreak>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PriceBreak {
    pub qty: u32,
    pub unit_price_usd: f64,
}

/// Cross-domain shared data — MPN, parameters, supply chain, etc.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct SharedSide {
    pub mpn: String,
    pub manufacturer: String,
    pub description: String,
    #[serde(default)]
    pub datasheet: Option<DatasheetRef>,
    #[serde(default)]
    pub suppliers: Vec<SupplierLink>,
    #[serde(default)]
    pub variants: BTreeMap<String, VariantOverride>,
    #[serde(default)]
    pub simulation: Option<SpiceModel>,
    #[serde(default)]
    pub parameters: ParamMap,
    #[serde(default)]
    pub parameter_template: Option<TemplateId>,

    // PLM-reserved (LIBRARY_PLAN §14.2). Inert in v0.9.
    #[serde(default)]
    pub avl: Vec<AvlEntry>,
    #[serde(default)]
    pub compliance: ComplianceTags,
    #[serde(default)]
    pub plm_link: Option<PlmLink>,
    #[serde(default)]
    pub distributor_pricing_hint: Option<PricingSnapshot>,
}

/// Subset of `SharedSide` that gets embedded in both `.snxsch` and `.snxpcb`
/// for fast BOM/fab access without forcing a cross-file dependency.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct SharedSlice {
    pub mpn: String,
    pub manufacturer: String,
    pub description: String,
    #[serde(default)]
    pub bom_params: ParamMap, // value, tolerance, package, etc — keyed for BOM rollup
}

impl SharedSide {
    /// Project the BOM-relevant subset for `.snxsch` / `.snxpcb` embed.
    pub fn slice_for_embed(&self) -> SharedSlice {
        let mut bom_params = ParamMap::new();
        // Conservative subset — anything else stays in shared, snapshot stays small.
        for key in ["value", "tolerance", "package", "rating"] {
            if let Some(v) = self.parameters.get(key) {
                bom_params.insert(key.to_string(), v.clone());
            }
        }
        SharedSlice {
            mpn: self.mpn.clone(),
            manufacturer: self.manufacturer.clone(),
            description: self.description.clone(),
            bom_params,
        }
    }
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

    #[test]
    fn shared_side_round_trips_with_plm_reserved() {
        let s = SharedSide {
            mpn: "RC0805FR-0710KL".into(),
            manufacturer: "Yageo".into(),
            description: "Resistor 10k 1% 0805".into(),
            datasheet: Some(DatasheetRef::url("https://example.com/ds.pdf")),
            suppliers: vec![SupplierLink {
                distributor: "DigiKey".into(),
                sku: "311-10.0KCRCT-ND".into(),
                url: None,
            }],
            variants: BTreeMap::new(),
            simulation: None,
            parameters: ParamMap::new(),
            parameter_template: Some(TemplateId("Resistor".into())),
            avl: vec![AvlEntry {
                manufacturer: "Yageo".into(),
                mpn: "RC0805FR-0710KL".into(),
                status: "Approved".into(),
                notes: None,
            }],
            compliance: ComplianceTags::default(),
            plm_link: None,
            distributor_pricing_hint: None,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: SharedSide = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn shared_slice_only_carries_bom_relevant_params() {
        let mut params = ParamMap::new();
        params.insert("value".into(), ParamValue::Text("10k".into()));
        params.insert(
            "temperature_coefficient".into(),
            ParamValue::Text("X7R".into()),
        );
        let shared = SharedSide {
            parameters: params,
            ..Default::default()
        };
        let slice = shared.slice_for_embed();
        assert!(slice.bom_params.contains_key("value"));
        assert!(!slice.bom_params.contains_key("temperature_coefficient"));
    }
}
