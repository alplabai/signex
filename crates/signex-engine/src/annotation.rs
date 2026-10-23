use crate::EngineError;
use crate::command::AnnotateMode;
use crate::patch::{DocumentPatch, PatchPair, SemanticPatch};

use std::collections::{HashMap, HashSet};

use signex_types::schematic::{ProjectParts, Symbol, annotation_order};
use uuid::Uuid;

use super::Engine;

/// Where one sheet sits in a project-wide annotation pass. The parts are
/// grouped across every sheet before the first sheet is touched, so units of
/// one part stay together wherever they are placed (#691).
#[derive(Clone, Copy)]
pub struct AnnotateScope<'a> {
    pub parts: &'a ProjectParts,
    /// This sheet's ordinal in the slice `parts` was built from.
    pub sheet: usize,
    /// Symbols the pass must leave alone.
    pub locked: &'a HashSet<Uuid>,
}

impl Engine {
    /// Annotate this document as the only sheet of its project, with an
    /// externally-owned per-prefix counter. Returns whether anything changed.
    pub fn annotate_with_seed(
        &mut self,
        mode: AnnotateMode,
        next_by_prefix: &mut HashMap<String, u32>,
    ) -> Result<bool, EngineError> {
        let locked = HashSet::new();
        let parts = ProjectParts::new(&[&self.document.symbols], &locked);
        let scope = AnnotateScope {
            parts: &parts,
            sheet: 0,
            locked: &locked,
        };
        self.annotate_in_project(mode, next_by_prefix, scope, &mut HashMap::new())
    }

    /// One sheet's turn in a project-wide annotation pass. `next_by_prefix`
    /// and `assigned` (the designator handed to each part so far) are shared
    /// by every sheet of the pass, so numbering and part membership carry over
    /// from sheet to sheet. Symbols in `scope.locked` are skipped (Altium's
    /// "Lock" column); see [`ProjectParts`] for how a lock treats the rest of
    /// its part. Returns whether anything changed.
    pub fn annotate_in_project(
        &mut self,
        mode: AnnotateMode,
        next_by_prefix: &mut HashMap<String, u32>,
        scope: AnnotateScope<'_>,
        assigned: &mut HashMap<usize, String>,
    ) -> Result<bool, EngineError> {
        let before = self.document.clone();
        let AnnotateScope {
            parts,
            sheet,
            locked,
        } = scope;
        let is_designator_target = |sym: &Symbol| -> bool {
            !sym.is_power && !sym.reference.starts_with('#') && !locked.contains(&sym.uuid)
        };
        let order = annotation_order(&self.document.symbols);

        // Phase 1: reset to '?' if requested. A part with a locked unit keeps
        // the locked unit's designator on its other units.
        if matches!(
            mode,
            AnnotateMode::ResetOnly | AnnotateMode::ResetAndRenumber
        ) {
            for (idx, symbol) in self.document.symbols.iter_mut().enumerate() {
                if !is_designator_target(symbol) {
                    continue;
                }
                if let Some(designator) = parts
                    .part(sheet, idx)
                    .and_then(|part| parts.locked_designator(part))
                {
                    symbol.reference = designator.to_string();
                    continue;
                }
                let prefix = alphabetic_prefix(&symbol.reference);
                if !prefix.is_empty() {
                    symbol.reference = format!("{prefix}?");
                }
            }
        }
        if matches!(mode, AnnotateMode::ResetOnly) {
            let patch_pair = PatchPair {
                semantic: SemanticPatch::SymbolFieldsUpdated,
                document: DocumentPatch::SYMBOLS,
            };
            self.record_history(before, patch_pair);
            return Ok(true);
        }

        // Phase 2: merge this sheet's existing numbers into the shared map
        // so later sheets don't collide.
        for symbol in &self.document.symbols {
            if !is_designator_target(symbol) {
                continue;
            }
            let prefix = alphabetic_prefix(&symbol.reference);
            if prefix.is_empty() {
                continue;
            }
            if let Ok(n) = symbol.reference[prefix.len()..].parse::<u32>() {
                let entry = next_by_prefix.entry(prefix).or_insert(0);
                if n > *entry {
                    *entry = n;
                }
            }
        }

        // Phase 3: renumber '?' symbols using the shared counter.
        let mut changed = false;
        for idx in order {
            let symbol = &mut self.document.symbols[idx];
            if !is_designator_target(symbol) {
                continue;
            }
            if !symbol.reference.ends_with('?') {
                continue;
            }
            let part = parts.part(sheet, idx);
            // A locked unit pins its whole part. Only an unannotated part can
            // get here (Phase 1 already moved annotated siblings onto the
            // locked designator), and it stays unannotated.
            if part.is_some_and(|p| parts.locked_designator(p).is_some()) {
                continue;
            }
            if let Some(designator) = part.and_then(|p| assigned.get(&p)) {
                symbol.reference = designator.clone();
                changed = true;
                continue;
            }
            let prefix = alphabetic_prefix(&symbol.reference);
            if prefix.is_empty() {
                continue;
            }
            let next = next_by_prefix.entry(prefix.clone()).or_insert(0);
            *next += 1;
            symbol.reference = format!("{prefix}{next}");
            if let Some(part) = part {
                assigned.insert(part, symbol.reference.clone());
            }
            changed = true;
        }

        if !changed {
            return Ok(false);
        }
        let patch_pair = PatchPair {
            semantic: SemanticPatch::SymbolFieldsUpdated,
            document: DocumentPatch::SYMBOLS,
        };
        self.record_history(before, patch_pair);
        Ok(true)
    }
}

fn alphabetic_prefix(reference: &str) -> String {
    reference
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_sheet;
    use signex_types::schematic::{Point, Symbol};
    use std::collections::HashMap;

    fn unit(reference: &str, lib_id: &str, unit: u32, y: f64) -> Symbol {
        Symbol {
            uuid: uuid::Uuid::new_v4(),
            lib_id: lib_id.to_string(),
            reference: reference.to_string(),
            unit,
            position: Point::new(0.0, y),
            ..Default::default()
        }
    }

    fn annotated(mode: AnnotateMode, symbols: Vec<Symbol>) -> Vec<String> {
        let mut document = test_sheet();
        document.symbols = symbols;
        let mut engine = Engine::new(document).unwrap();
        let mut seed = HashMap::new();
        engine.annotate_with_seed(mode, &mut seed).unwrap();
        engine
            .document
            .symbols
            .iter()
            .map(|s| s.reference.clone())
            .collect()
    }

    #[test]
    fn units_of_one_part_get_one_designator() {
        let refs = annotated(
            AnnotateMode::ResetAndRenumber,
            vec![
                unit("U?", "Amplifier:LM358", 1, 0.0),
                unit("U?", "Amplifier:LM358", 2, 10.0),
            ],
        );
        assert_eq!(refs, ["U1", "U1"]);
    }

    #[test]
    fn two_multi_unit_parts_get_two_designators() {
        let refs = annotated(
            AnnotateMode::ResetAndRenumber,
            vec![
                unit("U?", "Amplifier:LM358", 1, 0.0),
                unit("U?", "Amplifier:LM358", 2, 10.0),
                unit("U?", "Amplifier:LM358", 1, 20.0),
                unit("U?", "Amplifier:LM358", 2, 30.0),
            ],
        );
        assert_eq!(refs, ["U1", "U1", "U2", "U2"]);
    }

    #[test]
    fn reset_never_renumbers_units_of_a_part_apart() {
        // Annotated state: U5 (units 1+2) and U7 (units 1+2), interleaved on
        // the sheet. A reset pass must keep each part's units together.
        let refs = annotated(
            AnnotateMode::ResetAndRenumber,
            vec![
                unit("U5", "Amplifier:LM358", 1, 0.0),
                unit("U7", "Amplifier:LM358", 1, 10.0),
                unit("U5", "Amplifier:LM358", 2, 20.0),
                unit("U7", "Amplifier:LM358", 2, 30.0),
            ],
        );
        assert_eq!(refs[0], refs[2]);
        assert_eq!(refs[1], refs[3]);
        assert_ne!(refs[0], refs[1]);
    }

    fn engine_of(symbols: Vec<Symbol>) -> Engine {
        let mut document = test_sheet();
        document.symbols = symbols;
        Engine::new(document).unwrap()
    }

    fn refs_of(engine: &Engine) -> Vec<String> {
        engine
            .document
            .symbols
            .iter()
            .map(|s| s.reference.clone())
            .collect()
    }

    /// A project-wide pass over `engines`, in order, the way the app drives it.
    fn annotate_project(mode: AnnotateMode, engines: &mut [Engine], locked: &HashSet<Uuid>) {
        let sheets: Vec<&[Symbol]> = engines
            .iter()
            .map(|e| e.document.symbols.as_slice())
            .collect();
        let parts = ProjectParts::new(&sheets, locked);
        let mut seed = HashMap::new();
        let mut assigned = HashMap::new();
        for (sheet, engine) in engines.iter_mut().enumerate() {
            let scope = AnnotateScope {
                parts: &parts,
                sheet,
                locked,
            };
            engine
                .annotate_in_project(mode, &mut seed, scope, &mut assigned)
                .unwrap();
        }
    }

    #[test]
    fn reset_keeps_a_part_split_across_sheets_together() {
        let mut engines = [
            engine_of(vec![unit("U1", "Amplifier:LM358", 1, 0.0)]),
            engine_of(vec![unit("U1", "Amplifier:LM358", 2, 0.0)]),
        ];
        annotate_project(
            AnnotateMode::ResetAndRenumber,
            &mut engines,
            &HashSet::new(),
        );
        assert_eq!(refs_of(&engines[0]), refs_of(&engines[1]));
        assert!(!refs_of(&engines[0])[0].ends_with('?'));
    }

    #[test]
    fn a_locked_unannotated_unit_keeps_its_part_unannotated() {
        let locked_unit = unit("U?", "Amplifier:LM358", 1, 0.0);
        let locked = HashSet::from([locked_unit.uuid]);
        let mut engines = [engine_of(vec![
            locked_unit,
            unit("U?", "Amplifier:LM358", 2, 10.0),
        ])];
        annotate_project(AnnotateMode::Incremental, &mut engines, &locked);
        assert_eq!(refs_of(&engines[0]), ["U?", "U?"]);
    }

    #[test]
    fn a_locked_annotated_unit_keeps_its_siblings_on_its_designator_through_a_reset() {
        let locked_unit = unit("U5", "Amplifier:LM358", 1, 0.0);
        let locked = HashSet::from([locked_unit.uuid]);
        let mut engines = [engine_of(vec![
            locked_unit,
            unit("U5", "Amplifier:LM358", 2, 10.0),
        ])];
        annotate_project(AnnotateMode::ResetAndRenumber, &mut engines, &locked);
        assert_eq!(refs_of(&engines[0]), ["U5", "U5"]);
    }
}
