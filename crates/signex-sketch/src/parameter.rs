use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// User-defined parameter table — name → expression text.
/// The full evaluator (Phase 4) consumes this to resolve `DimTarget::Expr`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ParameterTable(pub BTreeMap<String, String>);
