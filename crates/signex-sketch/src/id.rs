use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SketchEntityId(pub Uuid);

impl SketchEntityId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SketchEntityId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SketchEntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConstraintId(pub Uuid);

impl ConstraintId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ConstraintId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ConstraintId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
