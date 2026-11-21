//! Differential pair classes (#1282): a named set of pair rules (impedance,
//! trace width and gap) a Differential Pair directive on the schematic can
//! name. Stored per project in `.snxprj` under `schematic_design`
//! (`docs/architecture/snxprj-format.md`).

use serde::{Deserialize, Serialize};

use crate::project::UnknownJson;

fn default_true() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

/// One differential pair class of the project. Every rule is optional; an
/// unset rule leaves the board's own rule in force.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiffPairClass {
    pub name: String,
    /// Differential impedance (ohm).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impedance_ohm: Option<f64>,
    /// Allowed impedance deviation, plus or minus (percent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impedance_tolerance_pct: Option<f64>,
    /// Trace width of each half (mm).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_mm: Option<f64>,
    /// Gap between the two halves (mm).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap_mm: Option<f64>,
    /// Pair nets by their suffix (`_P`/`_N`, `DP`/`DN`, `+`/`-`); written
    /// only when switched off.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub pair_by_suffix: bool,
    /// Keys written by a newer build, kept so a save does not drop them.
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

impl DiffPairClass {
    /// A class with no rules set that pairs nets by suffix.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            impedance_ohm: None,
            impedance_tolerance_pct: None,
            width_mm: None,
            gap_mm: None,
            pair_by_suffix: true,
            unknown: UnknownJson::default(),
        }
    }
}
