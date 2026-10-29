//! Copying a primitive container into another library (#813).
//!
//! A copy is a new primitive, not the original: the container and every
//! primitive in it get new uuids, the name is made unique among the names
//! the target library already holds, and the copy starts unreleased so it
//! stays editable under Team mode. The original is only read.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::footprint::FootprintFile;
use super::symbol::SymbolFile;

/// `name` when `taken` does not hold it, else `name (2)`, `name (3)`, ...
/// The first free suffix wins.
pub fn unique_copy_name(name: &str, taken: &HashSet<String>) -> String {
    if !taken.contains(name) {
        return name.to_string();
    }
    (2u32..)
        .map(|n| format!("{name} ({n})"))
        .find(|candidate| !taken.contains(candidate))
        .unwrap_or_else(|| name.to_string())
}

impl SymbolFile {
    /// A copy of this container for another library. `taken_names` are the
    /// symbol names the target library holds; each copied symbol keeps its
    /// name unless that collides, and the names the copy hands out count as
    /// taken for the symbols after it.
    pub fn copy_for_library(&self, taken_names: &HashSet<String>, now: DateTime<Utc>) -> Self {
        let mut copy = self.clone();
        let mut taken = taken_names.clone();
        copy.file_uuid = Uuid::now_v7();
        copy.created = now;
        copy.updated = now;
        for symbol in &mut copy.symbols {
            symbol.uuid = Uuid::now_v7();
            symbol.name = unique_copy_name(&symbol.name, &taken);
            symbol.released = false;
            symbol.created = now;
            symbol.updated = now;
            taken.insert(symbol.name.clone());
        }
        copy
    }
}

impl FootprintFile {
    /// A copy of this container for another library; see
    /// [`SymbolFile::copy_for_library`]. A footprint's STEP attachment is
    /// kept: it is addressed by content hash, so the copy points at the
    /// same model file, which the caller copies into the target library.
    pub fn copy_for_library(&self, taken_names: &HashSet<String>, now: DateTime<Utc>) -> Self {
        let mut copy = self.clone();
        let mut taken = taken_names.clone();
        copy.file_uuid = Uuid::now_v7();
        copy.created = now;
        copy.updated = now;
        for footprint in &mut copy.footprints {
            footprint.uuid = Uuid::now_v7();
            footprint.name = unique_copy_name(&footprint.name, &taken);
            footprint.released = false;
            footprint.created = now;
            footprint.updated = now;
            taken.insert(footprint.name.clone());
        }
        copy
    }

    /// The content hashes of the STEP models this container's footprints
    /// attach, without repeats.
    pub fn step_hashes(&self) -> Vec<String> {
        let mut hashes: Vec<String> = Vec::new();
        for attachment in self
            .footprints
            .iter()
            .filter_map(|f| f.step_attachment.as_ref())
        {
            if !hashes.contains(&attachment.content_hash) {
                hashes.push(attachment.content_hash.clone());
            }
        }
        hashes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Footprint, StepAttachment, Symbol};

    fn names(list: &[&str]) -> HashSet<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_free_name_is_kept() {
        assert_eq!(unique_copy_name("R", &names(&["C"])), "R");
    }

    #[test]
    fn a_taken_name_gets_the_first_free_suffix() {
        assert_eq!(unique_copy_name("R", &names(&["R"])), "R (2)");
        assert_eq!(unique_copy_name("R", &names(&["R", "R (2)"])), "R (3)");
    }

    #[test]
    fn a_symbol_copy_has_new_identity_and_the_stock_name() {
        let original = SymbolFile::from_symbol(Symbol::empty("R"));
        let copy = original.copy_for_library(&HashSet::new(), Utc::now());
        assert_ne!(copy.file_uuid, original.file_uuid);
        assert_ne!(copy.symbols[0].uuid, original.symbols[0].uuid);
        assert_eq!(copy.symbols[0].name, "R");
        assert_eq!(original.symbols[0].name, "R");
    }

    #[test]
    fn a_symbol_copy_is_unreleased() {
        let mut stock = Symbol::empty("R");
        stock.released = true;
        let copy = SymbolFile::from_symbol(stock).copy_for_library(&HashSet::new(), Utc::now());
        assert!(!copy.symbols[0].released);
    }

    #[test]
    fn a_colliding_symbol_name_is_suffixed() {
        let original = SymbolFile::from_symbol(Symbol::empty("R"));
        let copy = original.copy_for_library(&names(&["R"]), Utc::now());
        assert_eq!(copy.symbols[0].name, "R (2)");
    }

    #[test]
    fn a_footprint_copy_keeps_its_step_attachment() {
        let mut footprint = Footprint::empty("0603");
        footprint.step_attachment = Some(StepAttachment {
            content_hash: "abc".into(),
            filename: "0603.step".into(),
            offset_xyz: [0.0; 3],
            rotation_xyz: [0.0; 3],
        });
        let original = FootprintFile::from_footprint(footprint);
        let copy = original.copy_for_library(&names(&["0603"]), Utc::now());
        assert_eq!(copy.footprints[0].name, "0603 (2)");
        assert_ne!(copy.footprints[0].uuid, original.footprints[0].uuid);
        assert_eq!(
            copy.footprints[0].step_attachment,
            original.footprints[0].step_attachment
        );
        assert_eq!(copy.step_hashes(), vec!["abc".to_string()]);
    }
}
