//! `Engine::exec_place` — see `exec/mod.rs`.

use crate::*;
use signex_types::schematic::{SelectedItem, SelectedKind};

impl Engine {
    /// Reconcile junction dots after a command mutated wire geometry (move /
    /// rotate / mirror), removed wires (delete), or added one (place), and
    /// return the document patch including `JUNCTIONS` if any dot was
    /// minted or removed. Shared by all five arms so none of them can drift
    /// back into leaving a junction-less T (issue #402) or a stale,
    /// silently net-merging dot (issue #422) behind.
    fn reconciled_patch(&mut self, items: &[SelectedItem]) -> DocumentPatch {
        let mut patch = DocumentPatch::from_selected_items(items);
        if self.reconcile_wire_junctions(items) {
            patch |= DocumentPatch::JUNCTIONS;
        }
        patch
    }

    pub(crate) fn exec_place(
        &mut self,
        before: SchematicSheet,
        cmd: Command,
    ) -> Result<CommandResult, EngineError> {
        match cmd {
            Command::DeleteSelection { items } => {
                let mut changed = false;

                for item in &items {
                    changed |= self.remove_selected_item(item);
                }

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SelectionDeleted,
                    document: self.reconciled_patch(&items),
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::MoveSelection { items, dx, dy } => {
                let mut changed = false;

                for item in &items {
                    changed |= self.move_selected_item(item, dx, dy);
                }

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SelectionMoved,
                    document: self.reconciled_patch(&items),
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::RotateSelection {
                items,
                angle_degrees,
            } => {
                let mut changed = false;

                for item in &items {
                    changed |= self.rotate_selected_item(item, angle_degrees);
                }

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SelectionRotated,
                    document: self.reconciled_patch(&items),
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::MirrorSelection { items, axis } => {
                let mut changed = false;

                for item in &items {
                    changed |= self.mirror_selected_item(item, axis);
                }

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SelectionMirrored,
                    document: self.reconciled_patch(&items),
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceBus { bus } => {
                self.document.buses.push(bus);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::BUSES,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceLabel { label } => {
                self.document.labels.push(label);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::LABELS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceSymbol { symbol } => {
                self.document.symbols.push(symbol);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::SYMBOLS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceWireSegment { wire } => {
                let wire_item = SelectedItem::new(wire.uuid, SelectedKind::Wire);
                self.document.wires.push(wire);

                // Reconcile through the same shared path move / rotate /
                // mirror / delete use: mints the new wire's T both ways (its
                // own endpoints landing on something, and something else's
                // endpoint landing on its interior) and drops any stale
                // minted dot the new wire's own placement would otherwise
                // silently re-justify into merging two nets (issue #422).
                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: self.reconciled_patch(&[wire_item]),
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceJunction { junction } => {
                self.document.junctions.push(junction);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::JUNCTIONS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceNoConnect { no_connect } => {
                self.document.no_connects.push(no_connect);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::NO_CONNECTS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceBusEntry { bus_entry } => {
                self.document.bus_entries.push(bus_entry);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::BUS_ENTRIES,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceTextNote { text_note } => {
                self.document.text_notes.push(text_note);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::TEXT_NOTES,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::PlaceSchDrawing { drawing } => {
                self.document.drawings.push(drawing);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::DRAWINGS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            _ => unreachable!(),
        }
    }
}
