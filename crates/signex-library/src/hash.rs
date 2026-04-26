//! Deterministic content hashing for revisions.
//!
//! Hash is computed over a canonical JSON serialization of (schematic, pcb, shared)
//! using sorted-key BTreeMaps and pretty=false.

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::embed::{PcbSide, SchematicSide, SharedSide};

#[derive(Serialize)]
struct HashInput<'a> {
    schematic: &'a SchematicSide,
    pcb: &'a PcbSide,
    shared: &'a SharedSide,
}

/// Compute the canonical content hash of a revision's three sides.
pub fn hash_revision_content(
    schematic: &SchematicSide,
    pcb: &PcbSide,
    shared: &SharedSide,
) -> [u8; 32] {
    let input = HashInput {
        schematic,
        pcb,
        shared,
    };
    let canon = serde_json::to_vec(&input).expect("HashInput must serialize");
    let mut hasher = Sha256::new();
    hasher.update(&canon);
    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_deterministic() {
        let s = SchematicSide::default();
        let p = PcbSide::default();
        let sh = SharedSide::default();
        assert_eq!(
            hash_revision_content(&s, &p, &sh),
            hash_revision_content(&s, &p, &sh)
        );
    }

    #[test]
    fn hash_changes_when_shared_changes() {
        let s = SchematicSide::default();
        let p = PcbSide::default();
        let mut sh = SharedSide::default();
        let h1 = hash_revision_content(&s, &p, &sh);
        sh.mpn = "NEW".into();
        let h2 = hash_revision_content(&s, &p, &sh);
        assert_ne!(h1, h2);
    }
}
