use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Net identity
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NetId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NetClassId(pub String);

// ---------------------------------------------------------------------------
// Net class
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetClass {
    pub name: String,
    #[serde(default)]
    pub clearance: f64,
    #[serde(default)]
    pub trace_width: f64,
    #[serde(default)]
    pub via_diameter: f64,
    #[serde(default)]
    pub via_drill: f64,
    #[serde(default)]
    pub diff_pair_gap: f64,
    #[serde(default)]
    pub diff_pair_width: f64,
}

// ---------------------------------------------------------------------------
// Differential pair
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffPair {
    pub positive_net: String,
    pub negative_net: String,
    pub class: String,
}
