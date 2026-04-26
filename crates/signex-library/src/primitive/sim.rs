//! `SimModel` primitive — SPICE / Verilog-A model body, reusable across MPNs.
//!
//! Per `v0.9-library-refactor-plan.md` §2.3, a `SimModel` carries the model
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
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
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
