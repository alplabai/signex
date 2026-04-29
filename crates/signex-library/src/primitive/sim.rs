//! `SimModel` primitive — SPICE / Verilog-A model body, reusable across MPNs.
//!
//! Per `v0.9-refactor-2-plan.md` §2.3, a `SimModel` carries the model
//! body and a default symbol-pin → SPICE-node mapping. A binding `Component`
//! references it via `Revision::sim_ref` and may override the node map for
//! a specific MPN.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// SPICE / behavioural model dialect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SimKind {
    Spice3,
    Ngspice,
    LtSpice,
    VerilogA,
}

/// Reusable simulation model. Bound by `Component::sim_ref`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimModel {
    pub uuid: Uuid,
    pub name: String,
    pub kind: SimKind,
    /// SPICE / Verilog-A model body.
    pub body: String,
    /// Default symbol-pin number → SPICE-node mapping. Component revisions can
    /// override on a per-MPN basis.
    #[serde(default)]
    pub default_node_map: BTreeMap<String, String>,
    /// Semver-style revision string. Stage 14 of
    /// `v0.9-snxlib-as-file-plan.md`: sim models version independently
    /// of the bound symbols and component rows. Defaults to `"0.0.1"`.
    #[serde(default = "default_sim_version")]
    pub version: String,
    /// Released-flag: locks edit-in-place under Team mode.
    #[serde(default)]
    pub released: bool,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
}

fn default_sim_version() -> String {
    "0.0.1".to_string()
}

impl SimModel {
    pub fn empty(name: impl Into<String>, kind: SimKind) -> Self {
        let now = Utc::now();
        Self {
            uuid: Uuid::now_v7(),
            name: name.into(),
            kind,
            body: String::new(),
            default_node_map: BTreeMap::new(),
            version: default_sim_version(),
            released: false,
            created: now,
            updated: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sim_model_round_trip() {
        let mut node_map = BTreeMap::new();
        node_map.insert("1".into(), "in".into());
        node_map.insert("2".into(), "out".into());

        let s = SimModel {
            uuid: Uuid::now_v7(),
            name: "LM358".into(),
            kind: SimKind::Spice3,
            body: ".SUBCKT LM358 IN OUT VCC GND\n.ENDS".into(),
            default_node_map: node_map,
            version: "0.0.1".into(),
            released: false,
            created: Utc::now(),
            updated: Utc::now(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: SimModel = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn sim_kind_round_trip_all_variants() {
        for k in [
            SimKind::Spice3,
            SimKind::Ngspice,
            SimKind::LtSpice,
            SimKind::VerilogA,
        ] {
            let json = serde_json::to_string(&k).unwrap();
            let back: SimKind = serde_json::from_str(&json).unwrap();
            assert_eq!(k, back);
        }
    }

    #[test]
    fn empty_sim_model_carries_no_body() {
        let s = SimModel::empty("test", SimKind::Spice3);
        assert!(s.body.is_empty());
        assert!(s.default_node_map.is_empty());
    }
}
