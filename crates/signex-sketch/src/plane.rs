use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlaneId(pub Uuid);

impl PlaneId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for PlaneId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Plane {
    pub id: PlaneId,
    #[serde(flatten)]
    pub kind: PlaneKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "PascalCase")]
pub enum PlaneKind {
    /// Board top — Z=0 in footprint local coords. Geometry on this
    /// plane bakes into 2D footprint primitives (pads, silkscreen,
    /// courtyard).
    BoardTop,
    /// Body top — at Z = `offset_z_expr` evaluated in mm. Used for
    /// 3D extrude profiles in v0.14+.
    BodyTop {
        /// Expression evaluated to a length (mm). v0.13 preserves
        /// the string but does not bake; v0.14 uses it.
        offset_z_expr: String,
    },
}
