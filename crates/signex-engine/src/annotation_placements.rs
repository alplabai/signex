//! Annotating a sheet file placed more than once (#1835).
//!
//! Each placement is numbered as if it were a sheet of its own, seeing the
//! designators it resolves to there (`Symbol::designator_in`). The result goes
//! back as one `reference` (the first placement's) plus an instance entry per
//! non-root placement, so removing a placement never moves the others.

use std::collections::HashMap;

use signex_types::schematic::Symbol;

use crate::EngineError;
use crate::annotation::{AnnotateScope, number_symbols};
use crate::command::AnnotateMode;

use super::Engine;

impl Engine {
    /// Number every placement of `scope.placements` over the original
    /// designators, then write the outcome into the document. On error the
    /// document is untouched.
    pub(super) fn number_placements(
        &mut self,
        mode: AnnotateMode,
        next_by_prefix: &mut HashMap<String, u32>,
        scope: AnnotateScope<'_>,
        assigned: &mut HashMap<usize, String>,
    ) -> Result<(), EngineError> {
        let originals: Vec<Symbol> = self.document.symbols.clone();
        let mut finals: Vec<Vec<String>> = Vec::with_capacity(scope.placements.len());
        for placement in scope.placements {
            let mut view = originals.clone();
            for symbol in &mut view {
                symbol.reference = symbol.designator_in(&placement.path).to_string();
            }
            let at = AnnotateScope {
                sheet: placement.sheet,
                ..scope
            };
            number_symbols(&mut view, mode, next_by_prefix, at, assigned)?;
            finals.push(view.into_iter().map(|s| s.reference).collect());
        }
        for (idx, symbol) in self.document.symbols.iter_mut().enumerate() {
            let touched = scope
                .placements
                .iter()
                .zip(&finals)
                .any(|(p, refs)| refs[idx] != originals[idx].designator_in(&p.path));
            if !touched {
                continue;
            }
            let all_unannotated = finals.iter().all(|refs| refs[idx].trim().ends_with('?'));
            for (placement, refs) in scope.placements.iter().zip(&finals) {
                if placement.path.is_root() {
                    continue;
                }
                if all_unannotated {
                    symbol.clear_instance_designator(&placement.path);
                } else {
                    symbol.set_instance_designator(&placement.path, refs[idx].clone());
                }
            }
            symbol.reference.clone_from(&finals[0][idx]);
        }
        Ok(())
    }
}
