//! What a pasted part is called, per the "Designators on paste" preference.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use signex_engine::PlacementPayload;
use signex_types::designator::{
    alphabetic_prefix, merge_counters, seed_counters, split_reference, take_next,
};

use crate::app::Signex;
use crate::paste_prefs::PasteDesignators;

/// The numbering of one paste, or of several copies pasted in a row: the
/// preference and, for Next free, the counters the copies share.
pub(crate) struct PasteNumbering {
    mode: PasteDesignators,
    counters: HashMap<String, u32>,
    /// Designators already in use, for Keep unless taken.
    used: HashSet<String>,
}

impl PasteNumbering {
    pub(crate) fn new(mode: PasteDesignators, counters: HashMap<String, u32>) -> Self {
        Self {
            mode,
            counters,
            used: HashSet::new(),
        }
    }

    /// The same numbering that also knows which designators are in use.
    pub(crate) fn with_used(mut self, used: HashSet<String>) -> Self {
        self.used = used;
        self
    }

    /// The designator a part called `old` gets under Keep unless taken: `None`
    /// keeps it (unnumbered, or free), otherwise the next number above every
    /// one in use. Either way the result counts as used for the next part.
    fn keep_if_free(&mut self, old: &str) -> Option<String> {
        let (prefix, number) = split_reference(old);
        let number = number.filter(|_| !prefix.is_empty())?;
        if self.used.insert(old.to_owned()) {
            let highest = self.counters.entry(prefix.to_owned()).or_insert(0);
            *highest = (*highest).max(number);
            return None;
        }
        let new = take_next(prefix, &mut self.counters)?;
        self.used.insert(new.clone());
        Some(new)
    }

    /// `payload` with the designators the preference gives its parts. The
    /// units of one part always stay one part.
    pub(crate) fn apply(&mut self, payload: &PlacementPayload) -> PlacementPayload {
        match self.mode {
            PasteDesignators::Keep => payload.clone(),
            PasteDesignators::KeepIfFree => payload.redesignate_parts(|old| self.keep_if_free(old)),
            PasteDesignators::Reset => payload.redesignate_parts(|old| {
                let prefix = alphabetic_prefix(old);
                (!prefix.is_empty()).then(|| format!("{prefix}?"))
            }),
            PasteDesignators::NextFree => payload.redesignate_parts(|old| {
                let prefix = alphabetic_prefix(old);
                if prefix.is_empty() {
                    return None;
                }
                take_next(prefix, &mut self.counters)
            }),
        }
    }
}

impl Signex {
    /// The numbering for a paste onto `target`, in the saved preference's mode.
    pub(crate) fn paste_numbering(&self, target: &Path) -> PasteNumbering {
        self.paste_numbering_with(target, self.ui_state.paste_designators)
    }

    /// The numbering for a paste onto `target` in `mode`. Next free starts
    /// from the project's numbers read when the paste began and folds in what
    /// the target's live document holds now, so an edit made since (a stamp's
    /// earlier click included) is counted too. Without a read (none was
    /// needed or it was for another sheet) the live sheet alone is counted.
    pub(crate) fn paste_numbering_with(
        &self,
        target: &Path,
        mode: PasteDesignators,
    ) -> PasteNumbering {
        if !mode.reads_project() {
            return PasteNumbering::new(mode, HashMap::new());
        }
        let read = self
            .interaction_state
            .paste_counters
            .as_ref()
            .filter(|c| c.target == target);
        let mut counters = read.map(|c| c.counters.clone()).unwrap_or_default();
        let mut used = read.map(|c| c.used.clone()).unwrap_or_default();
        if let Some(live) = self.document_state.engines.get(target) {
            let references = || {
                live.document()
                    .symbols
                    .iter()
                    .filter(|s| !s.is_power && !s.reference.starts_with('#'))
                    .map(|s| s.reference.as_str())
            };
            merge_counters(&mut counters, &seed_counters(references()));
            used.extend(references().map(|r| r.trim().to_owned()));
        }
        PasteNumbering::new(mode, counters).with_used(used)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{SchematicSheet, Symbol};

    fn part(reference: &str, lib: &str, unit: u32) -> Symbol {
        Symbol {
            uuid: uuid::Uuid::now_v7(),
            lib_id: lib.to_owned(),
            reference: reference.to_owned(),
            unit,
            ..Default::default()
        }
    }

    fn payload() -> PlacementPayload {
        PlacementPayload::new(SchematicSheet {
            uuid: uuid::Uuid::now_v7(),
            paper_size: "A4".to_owned(),
            symbols: vec![
                part("R1", "lib:r", 1),
                part("U4", "lib:dual", 1),
                part("R2", "lib:r", 1),
                part("U4", "lib:dual", 2),
            ],
            ..Default::default()
        })
    }

    fn references(p: &PlacementPayload) -> Vec<String> {
        p.fragment()
            .symbols
            .iter()
            .map(|s| s.reference.clone())
            .collect()
    }

    #[test]
    fn keep_leaves_the_designators_alone() {
        let mut numbering = PasteNumbering::new(PasteDesignators::Keep, HashMap::new());
        assert_eq!(
            references(&numbering.apply(&payload())),
            ["R1", "U4", "R2", "U4"]
        );
    }

    #[test]
    fn reset_makes_every_part_unnumbered_and_units_still_share_one() {
        let mut numbering = PasteNumbering::new(PasteDesignators::Reset, HashMap::new());
        assert_eq!(
            references(&numbering.apply(&payload())),
            ["R?", "U?", "R?", "U?"]
        );
    }

    #[test]
    fn keep_if_free_renumbers_only_the_parts_whose_designator_is_taken() {
        let counters = seed_counters(["R2", "U1"]);
        let used = HashSet::from(["R2".to_owned(), "U1".to_owned()]);
        let mut numbering =
            PasteNumbering::new(PasteDesignators::KeepIfFree, counters).with_used(used);
        // R1 and U4 (both units) are free and kept; R2 is taken.
        assert_eq!(
            references(&numbering.apply(&payload())),
            ["R1", "U4", "R3", "U4"]
        );
    }

    #[test]
    fn keep_if_free_moves_units_of_a_taken_part_together() {
        let counters = seed_counters(["U4"]);
        let used = HashSet::from(["U4".to_owned()]);
        let mut numbering =
            PasteNumbering::new(PasteDesignators::KeepIfFree, counters).with_used(used);
        assert_eq!(
            references(&numbering.apply(&payload())),
            ["R1", "U5", "R2", "U5"]
        );
    }

    #[test]
    fn keep_if_free_leaves_unannotated_parts_alone() {
        let mut numbering = PasteNumbering::new(PasteDesignators::KeepIfFree, HashMap::new());
        let rq = payload().redesignate_parts(|_| Some("R?".to_owned()));
        assert_eq!(references(&numbering.apply(&rq)), ["R?", "R?", "R?", "R?"]);
    }

    #[test]
    fn next_free_numbers_parts_after_the_project_and_units_share_one() {
        let counters = seed_counters(["R7", "U2", "C9"]);
        let mut numbering = PasteNumbering::new(PasteDesignators::NextFree, counters);
        assert_eq!(
            references(&numbering.apply(&payload())),
            ["R8", "U3", "R9", "U3"]
        );
        // A second copy carries on from the first.
        assert_eq!(
            references(&numbering.apply(&payload())),
            ["R10", "U4", "R11", "U4"]
        );
    }
}
