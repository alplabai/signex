use serde::{Deserialize, Serialize};

use crate::array::Array;
use crate::constraint::Constraint;
use crate::entity::Entity;
use crate::parameter::ParameterTable;
use crate::plane::Plane;

/// Top-level container for a footprint sketch. Persisted as part of
/// a `Footprint`'s schema (v2+).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SketchData {
    #[serde(default)]
    pub planes: Vec<Plane>,
    #[serde(default)]
    pub entities: Vec<Entity>,
    #[serde(default)]
    pub constraints: Vec<Constraint>,
    #[serde(default)]
    pub arrays: Vec<Array>,
    #[serde(default)]
    pub parameters: ParameterTable,
}
