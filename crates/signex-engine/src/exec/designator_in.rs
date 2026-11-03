//! `Command::SetSymbolDesignatorIn` — one symbol's designator at one sheet
//! placement (#1151).
//!
//! A child sheet placed more than once carries a designator per placement
//! in each symbol's instance entries (#829); `Symbol::designator_in` reads
//! them. Editing a designator on a tab opened through a placement writes
//! that placement's entry, so the canvas, the export and the netlist all
//! show the edited value for it, and the other placements keep theirs.

use crate::*;
use signex_types::schematic::InstancePath;
use uuid::Uuid;

impl Engine {
    pub(super) fn set_symbol_designator_in(
        &mut self,
        before: SchematicSheet,
        symbol: Uuid,
        placement: InstancePath,
        reference: String,
    ) -> Result<CommandResult, EngineError> {
        let reference = reference.trim().to_string();
        let Some(sym) = self.document.symbols.iter_mut().find(|s| s.uuid == symbol) else {
            return Ok(CommandResult::unchanged());
        };
        if placement.is_root() || sym.designator_in(&placement) == reference {
            return Ok(CommandResult::unchanged());
        }

        if reference.is_empty() {
            // No designator of its own: the placement falls back to `reference`.
            if !sym.clear_instance_designator(&placement) {
                // Nothing to drop: no history, no dirty document.
                return Ok(CommandResult::unchanged());
            }
        } else {
            // The resolver reads the first valid entry for the path, so the
            // first one found is the one to change; a blank one becomes valid.
            sym.set_instance_designator(&placement, reference);
        }

        let patch_pair = PatchPair {
            semantic: SemanticPatch::TextUpdated,
            document: DocumentPatch::SYMBOLS,
        };
        self.record_history(before, patch_pair);
        Ok(CommandResult::changed(patch_pair))
    }
}
