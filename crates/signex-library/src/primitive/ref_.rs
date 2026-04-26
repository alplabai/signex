//! `PrimitiveRef` — `(library_id, uuid)` address for a reusable primitive.
//!
//! Per the v0.9 library refactor (§2.4 / §2.6), every primitive lives inside
//! a single library and is identified by its UUID *within that library*.
//! Cross-library references compose the library UUID (from `library.toml::id`)
//! with the primitive UUID. `LibrarySet` (WS-C) resolves these tuples back to
//! the actual primitive struct.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// `(library_id, primitive_uuid)` — the canonical primitive address.
///
/// `Hash + Eq + Ord` so the type works as a key in `HashMap` / `BTreeMap`,
/// which `WhereUsedIndex` and the resolver both need.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PrimitiveRef {
    /// Stable UUID from the source library's `library.toml::id`.
    pub library_id: Uuid,
    /// Primitive UUID within that library.
    pub uuid: Uuid,
}

impl PrimitiveRef {
    pub fn new(library_id: Uuid, uuid: Uuid) -> Self {
        Self { library_id, uuid }
    }
}

impl std::fmt::Display for PrimitiveRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.library_id, self.uuid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_ref_round_trip() {
        let r = PrimitiveRef::new(Uuid::now_v7(), Uuid::now_v7());
        let json = serde_json::to_string(&r).unwrap();
        let back: PrimitiveRef = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn primitive_ref_display_is_slash_separated() {
        let lib = Uuid::nil();
        let u = Uuid::nil();
        let r = PrimitiveRef::new(lib, u);
        assert_eq!(r.to_string(), format!("{lib}/{u}"));
    }

    #[test]
    fn primitive_ref_is_hashable() {
        use std::collections::HashSet;
        let mut s = HashSet::new();
        let r = PrimitiveRef::new(Uuid::now_v7(), Uuid::now_v7());
        s.insert(r);
        assert!(s.contains(&r));
    }
}
