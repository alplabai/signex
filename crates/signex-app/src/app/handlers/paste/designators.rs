//! What a pasted part is called, per the "Designators on paste" preference.

use std::collections::HashMap;
use std::path::Path;

use signex_engine::PlacementPayload;
use signex_types::designator::{alphabetic_prefix, seed_counters, take_next};

use crate::app::Signex;
use crate::paste_prefs::PasteDesignators;

/// The numbering of one paste, or of several copies pasted in a row: the
/// preference and, for Next free, the counters the copies share.
pub(crate) struct PasteNumbering {
    mode: PasteDesignators,
    counters: HashMap<String, u32>,
}

impl PasteNumbering {
    pub(crate) fn new(mode: PasteDesignators, counters: HashMap<String, u32>) -> Self {
        Self { mode, counters }
    }

    /// `payload` with the designators the preference gives its parts. The
    /// units of one part always stay one part.
    pub(crate) fn apply(&mut self, payload: &PlacementPayload) -> PlacementPayload {
        match self.mode {
            PasteDesignators::Keep => payload.clone(),
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
    /// The numbering for a paste onto `target`. Next free counts from every
    /// designator in the target's project, the open sheet's live document
    /// included, the way Annotate does.
    pub(crate) fn paste_numbering(&self, target: &Path) -> PasteNumbering {
        let mode = self.ui_state.paste_designators;
        if mode != PasteDesignators::NextFree {
            return PasteNumbering::new(mode, HashMap::new());
        }
        let (_pages, project) =
            crate::app::project_sheets::assemble_project_sheets_for(&self.document_state, target);
        let live = self
            .document_state
            .engines
            .get(target)
            .map(|e| e.document());
        let symbols = project
            .sheets
            .values()
            .chain(live)
            .flat_map(|sheet| sheet.symbols.iter())
            .filter(|s| !s.is_power && !s.reference.starts_with('#'));
        PasteNumbering::new(mode, seed_counters(symbols.map(|s| s.reference.as_str())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{SchematicSheet, Symbol};

    fn part(reference: &str, lib: &str, unit: u32) -> Symbol {
        Symbol {
            uuid: uuid::Uuid::new_v4(),
            lib_id: lib.to_owned(),
            reference: reference.to_owned(),
            unit,
            ..Default::default()
        }
    }

    fn payload() -> PlacementPayload {
        PlacementPayload::new(SchematicSheet {
            uuid: uuid::Uuid::new_v4(),
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
