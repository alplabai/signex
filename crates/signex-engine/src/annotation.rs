use crate::EngineError;
use crate::command::AnnotateMode;
use crate::patch::{DocumentPatch, PatchPair, SemanticPatch};

use std::collections::{HashMap, HashSet};

use signex_types::schematic::{Symbol, designatable_parts};
use uuid::Uuid;

use super::Engine;

impl Engine {
    /// Cross-sheet annotation variant. Like `Command::AnnotateAll` but uses
    /// (and updates) an externally-owned per-prefix counter, so every
    /// sheet in a project can share one global numbering pass. Returns
    /// whether anything changed.
    pub fn annotate_with_seed(
        &mut self,
        mode: AnnotateMode,
        next_by_prefix: &mut std::collections::HashMap<String, u32>,
    ) -> Result<bool, EngineError> {
        self.annotate_with_seed_and_locks(mode, next_by_prefix, &std::collections::HashSet::new())
    }

    /// Same as `annotate_with_seed`, but skips every symbol whose uuid
    /// appears in `locked`. Used by the Annotate dialog's per-row lock
    /// checkboxes so the user can exclude individual designators from
    /// reannotation (Altium's "Lock" column behaviour).
    pub fn annotate_with_seed_and_locks(
        &mut self,
        mode: AnnotateMode,
        next_by_prefix: &mut std::collections::HashMap<String, u32>,
        locked: &std::collections::HashSet<uuid::Uuid>,
    ) -> Result<bool, EngineError> {
        let before = self.document.clone();
        let is_designator_target = |sym: &signex_types::schematic::Symbol| -> bool {
            !sym.is_power && !sym.reference.starts_with('#') && !locked.contains(&sym.uuid)
        };

        // Units of one multi-unit part are grouped BEFORE any reset, while
        // their designators still say which instances belong together, so no
        // pass can renumber a part's units apart (#691).
        let mut order: Vec<usize> = (0..self.document.symbols.len()).collect();
        order.sort_by(|a, b| {
            let sa = &self.document.symbols[*a];
            let sb = &self.document.symbols[*b];
            sa.position
                .y
                .partial_cmp(&sb.position.y)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(
                    sa.position
                        .x
                        .partial_cmp(&sb.position.x)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
                .then(sa.uuid.cmp(&sb.uuid))
        });

        let part_of = designatable_parts(&self.document.symbols, &order);
        let locked_designator = locked_designator_by_part(&self.document.symbols, &part_of, locked);

        // Phase 1: reset to '?' if requested.
        if matches!(
            mode,
            AnnotateMode::ResetOnly | AnnotateMode::ResetAndRenumber
        ) {
            for (idx, symbol) in self.document.symbols.iter_mut().enumerate() {
                if !is_designator_target(symbol) {
                    continue;
                }
                if let Some(designator) = part_of
                    .get(&idx)
                    .and_then(|part| locked_designator.get(part))
                {
                    symbol.reference = designator.clone();
                    continue;
                }
                let prefix: String = symbol
                    .reference
                    .chars()
                    .take_while(|c| c.is_ascii_alphabetic())
                    .collect();
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
            let prefix: String = symbol
                .reference
                .chars()
                .take_while(|c| c.is_ascii_alphabetic())
                .collect();
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
        let mut designator_of_part: HashMap<usize, String> = HashMap::new();
        for idx in order {
            let symbol = &mut self.document.symbols[idx];
            if !is_designator_target(symbol) {
                continue;
            }
            if !symbol.reference.ends_with('?') {
                continue;
            }
            let part = part_of.get(&idx).copied();
            if let Some(designator) = part.and_then(|p| designator_of_part.get(&p)) {
                symbol.reference = designator.clone();
                changed = true;
                continue;
            }
            let prefix: String = symbol
                .reference
                .chars()
                .take_while(|c| c.is_ascii_alphabetic())
                .collect();
            if prefix.is_empty() {
                continue;
            }
            let next = next_by_prefix.entry(prefix.clone()).or_insert(0);
            *next += 1;
            symbol.reference = format!("{prefix}{next}");
            if let Some(part) = part {
                designator_of_part.insert(part, symbol.reference.clone());
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

/// The designator of a part that has a locked unit, so a reset leaves its
/// unlocked units on it instead of renumbering them away from the locked one.
fn locked_designator_by_part(
    symbols: &[Symbol],
    part_of: &HashMap<usize, usize>,
    locked: &HashSet<Uuid>,
) -> HashMap<usize, String> {
    part_of
        .iter()
        .filter(|(idx, _)| locked.contains(&symbols[**idx].uuid))
        .map(|(idx, part)| (*part, symbols[*idx].reference.clone()))
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
}
