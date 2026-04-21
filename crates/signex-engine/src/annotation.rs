use crate::command::AnnotateMode;
use crate::patch::{DocumentPatch, PatchPair, SemanticPatch};
use crate::EngineError;

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

        // Phase 1: reset to '?' if requested.
        if matches!(mode, AnnotateMode::ResetOnly | AnnotateMode::ResetAndRenumber) {
            for symbol in self.document.symbols.iter_mut() {
                if !is_designator_target(symbol) {
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

        let mut changed = false;
        for idx in order {
            let symbol = &mut self.document.symbols[idx];
            if !is_designator_target(symbol) {
                continue;
            }
            if !symbol.reference.ends_with('?') {
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
