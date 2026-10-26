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

    /// Rubber-band the already moved `self.document` and reconcile junctions,
    /// then check the result against the nets the move asks for (the user's intent): any difference
    /// escalates the offending tips (new path, then plain move) and the pass
    /// reruns from the plain-moved sheet, so what is committed always keeps
    /// the nets (or is the old plain move for a warned pin).
    fn rubber_band_guarded(
        &mut self,
        before: &SchematicSheet,
        tips: &[crate::PinTip],
        items: &[SelectedItem],
    ) -> DocumentPatch {
        const MAX_ROUNDS: usize = 6;
        let base = self.document.clone();
        // What a plain move leaves: the reference for the user's own edits.
        let plain_reference = {
            self.reconciled_patch(items);
            std::mem::replace(&mut self.document, base.clone())
        };
        let guard = crate::Guard::new(before, &plain_reference, tips, self.preset_before.clone());
        let mut policy = crate::Policy::default();
        for round in 0.. {
            self.document = base.clone();
            let follow = crate::follow_moved_pins(&mut self.document, tips, items, &policy);
            let mut touched = items.to_vec();
            touched.extend(follow.touched);
            let patch = self.reconciled_patch(&touched);
            let Some(blamed) = guard.check(&self.document, tips, &policy, &follow.edited_tips)
            else {
                self.warn_plain_pins(&guard, tips, &policy);
                return patch;
            };
            // Every blamed tip is still live, so each round takes at least one
            // a step down: at most two rounds per tip, and after MAX_ROUNDS the
            // blamed go straight to a plain move.
            for key in blamed {
                if round >= MAX_ROUNDS {
                    policy.plain.insert(key);
                } else {
                    policy.escalate(key);
                }
            }
        }
        unreachable!("the escalation loop returns from inside")
    }

    /// One notice for the connected pins the guard had to leave as a plain
    /// move: a count and the first few, not a line per pin.
    fn warn_plain_pins(
        &mut self,
        guard: &crate::Guard,
        tips: &[crate::PinTip],
        policy: &crate::Policy,
    ) {
        const NAMED: usize = 3;
        let pins: Vec<String> = tips
            .iter()
            .filter(|tip| policy.plain.contains(&tip.old_key()) && guard.was_connected(tip))
            .map(|tip| tip.label().to_string())
            .collect();
        if pins.is_empty() {
            return;
        }
        let shown = pins
            .iter()
            .take(NAMED)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        self.warnings.push(match pins.len() {
            1 => format!("could not keep {shown} connected without crossing another net"),
            n if n <= NAMED => {
                format!("could not keep {n} pins connected without crossing another net: {shown}")
            }
            n => format!(
                "could not keep {n} pins connected without crossing another net: {shown} and {} more",
                n - NAMED
            ),
        });
        self.unkept_pins.extend(pins);
    }

    /// Move every item by its own delta; with `keep_wires` the wires on the
    /// moved pins follow (see `transform::rubber_band`) inside the same
    /// history entry.
    fn exec_moves(
        &mut self,
        before: SchematicSheet,
        moves: Vec<(SelectedItem, f64, f64)>,
        keep_wires: bool,
    ) -> Result<CommandResult, EngineError> {
        let items: Vec<SelectedItem> = moves.iter().map(|(item, _, _)| *item).collect();
        // Pin tips must be read before the symbols move.
        let tips = keep_wires.then(|| crate::moved_pin_tips(&self.document, &items));

        let mut changed = false;
        for (item, dx, dy) in &moves {
            changed |= self.move_selected_item(item, *dx, *dy);
        }
        if !changed {
            return Ok(CommandResult::unchanged());
        }

        let patch = match tips.filter(|tips| !tips.is_empty()) {
            Some(tips) => self.rubber_band_guarded(&before, &tips, &items),
            None => self.reconciled_patch(&items),
        };
        let patch_pair = PatchPair {
            semantic: SemanticPatch::SelectionMoved,
            document: patch,
        };
        self.record_history(before, patch_pair);
        Ok(CommandResult::changed(patch_pair))
    }

    /// Pin tips of `items` read before a transform turns them, when the
    /// caller asked for wires to be kept.
    fn pin_tips_if(&self, keep_wires: bool, items: &[SelectedItem]) -> Option<Vec<crate::PinTip>> {
        keep_wires.then(|| crate::moved_pin_tips(&self.document, items))
    }

    /// Close a rotate / mirror / transform: with pin tips, run the same
    /// guarded rubber-band pass a move uses, then record one history entry
    /// covering the turn and the wire edits.
    fn finish_transform(
        &mut self,
        before: SchematicSheet,
        items: &[SelectedItem],
        tips: Option<Vec<crate::PinTip>>,
        changed: bool,
        semantic: SemanticPatch,
    ) -> Result<CommandResult, EngineError> {
        if !changed {
            return Ok(CommandResult::unchanged());
        }
        let patch = match tips.filter(|tips| !tips.is_empty()) {
            Some(tips) => self.rubber_band_guarded(&before, &tips, items),
            None => self.reconciled_patch(items),
        };
        let patch_pair = PatchPair {
            semantic,
            document: patch,
        };
        self.record_history(before, patch_pair);
        Ok(CommandResult::changed(patch_pair))
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
            Command::MoveSelection {
                items,
                dx,
                dy,
                keep_wires,
            } => {
                let moves = items.into_iter().map(|item| (item, dx, dy)).collect();
                self.exec_moves(before, moves, keep_wires)
            }
            Command::MoveEach { moves, keep_wires } => self.exec_moves(before, moves, keep_wires),
            Command::RotateSelection {
                items,
                angle_degrees,
                keep_wires,
            } => {
                let tips = self.pin_tips_if(keep_wires, &items);
                let mut changed = false;
                for item in &items {
                    changed |= self.rotate_selected_item(item, angle_degrees);
                }
                self.finish_transform(
                    before,
                    &items,
                    tips,
                    changed,
                    SemanticPatch::SelectionRotated,
                )
            }
            Command::TransformSelection {
                items,
                centre,
                transform,
                keep_wires,
            } => {
                let tips = self.pin_tips_if(keep_wires, &items);
                let changed = self.transform_selection_about(&items, centre, transform)?;
                let semantic = match transform {
                    SelectionTransform::Mirror(_) => SemanticPatch::SelectionMirrored,
                    _ => SemanticPatch::SelectionRotated,
                };
                self.finish_transform(before, &items, tips, changed, semantic)
            }
            Command::MirrorSelection {
                items,
                axis,
                keep_wires,
            } => {
                let tips = self.pin_tips_if(keep_wires, &items);
                let mut changed = false;
                for item in &items {
                    changed |= self.mirror_selected_item(item, axis);
                }
                self.finish_transform(
                    before,
                    &items,
                    tips,
                    changed,
                    SemanticPatch::SelectionMirrored,
                )
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
            Command::RegisterLibSymbol { lib_symbol } => {
                if self.document.lib_symbols.contains_key(&lib_symbol.id) {
                    return Ok(CommandResult::unchanged());
                }
                self.document
                    .lib_symbols
                    .insert(lib_symbol.id.clone(), lib_symbol);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::LIB_SYMBOLS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::ReplaceLibSymbol {
                lib_symbol,
                library_versions,
            } => self.replace_lib_symbol(before, lib_symbol, library_versions),
            Command::PlaceChildSheet { child_sheet } => {
                // A sheet with no target file cannot be stitched or
                // navigated, so a blank filename is rejected up front,
                // before any mutation or history entry.
                //
                // An empty `name` IS allowed: the sheet is still addressable
                // by uuid and filename, parsed files may carry one, and the
                // placing tool is expected to default it (e.g. to the file
                // stem). The netlist then qualifies child nets with an empty
                // segment (a leading `/`), which is cosmetic only.
                if child_sheet.filename.trim().is_empty() {
                    return Err(EngineError::EmptyChildSheetFilename);
                }
                self.document.child_sheets.push(child_sheet);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::CHILD_SHEETS,
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
            Command::PlaceNoErcDirective { directive } => {
                self.document.no_erc_directives.push(directive);

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::ObjectPlaced,
                    document: DocumentPatch::NO_ERC_DIRECTIVES,
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
