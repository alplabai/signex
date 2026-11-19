//! Unions (#1300): a named group of objects on one sheet that select, move
//! and copy together. A union owns no geometry; it lists its members by
//! uuid. File format: `docs/architecture/snxsch-refinement-objects.md`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// `skip_serializing_if` predicate: a lock is written only when set.
fn is_false(value: &bool) -> bool {
    !*value
}

/// One union of sheet objects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchUnion {
    pub uuid: Uuid,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// The uuids of the member objects. A member that names no object on the
    /// sheet is dropped when the sheet loads.
    #[serde(default)]
    pub members: Vec<Uuid>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
}
