//! `Engine::exec_structure` — see `exec/mod.rs`.

use crate::*;

impl Engine {
    pub(crate) fn exec_structure(
        &mut self,
        before: SchematicSheet,
        cmd: Command,
    ) -> Result<CommandResult, EngineError> {
        match cmd {
            Command::UpdateSchDrawing { drawing } => {
                use signex_types::schematic::SchDrawing;
                let target_uuid = match &drawing {
                    SchDrawing::Line { uuid, .. }
                    | SchDrawing::Rect { uuid, .. }
                    | SchDrawing::Circle { uuid, .. }
                    | SchDrawing::Arc { uuid, .. }
                    | SchDrawing::Polyline { uuid, .. } => *uuid,
                };
                let mut changed = false;
                for d in self.document.drawings.iter_mut() {
                    let u = match d {
                        SchDrawing::Line { uuid, .. }
                        | SchDrawing::Rect { uuid, .. }
                        | SchDrawing::Circle { uuid, .. }
                        | SchDrawing::Arc { uuid, .. }
                        | SchDrawing::Polyline { uuid, .. } => *uuid,
                    };
                    if u == target_uuid {
                        *d = drawing.clone();
                        changed = true;
                        break;
                    }
                }
                if !changed {
                    return Ok(CommandResult::unchanged());
                }
                // MD-11: drawing geometry changes are NOT text events;
                // emit `DrawingMutated` so consumers don't take the
                // text-reflow path on a `SchDrawing::Line` move.
                let patch_pair = PatchPair {
                    semantic: SemanticPatch::DrawingMutated,
                    document: DocumentPatch::DRAWINGS,
                };
                self.record_history(before, patch_pair);
                Ok(CommandResult::changed(patch_pair))
            }
            Command::UpdateChildSheetStyle {
                sheet_id,
                stroke_width,
                stroke_color,
                fill_color,
            } => {
                let Some(sheet) = self
                    .document
                    .child_sheets
                    .iter_mut()
                    .find(|cs| cs.uuid == sheet_id)
                else {
                    return Ok(CommandResult::unchanged());
                };
                let mut changed = false;
                if let Some(w) = stroke_width
                    && (sheet.stroke_width - w).abs() > f64::EPSILON
                {
                    sheet.stroke_width = w;
                    changed = true;
                }
                if let Some(c) = stroke_color
                    && sheet.stroke_color != c
                {
                    sheet.stroke_color = c;
                    changed = true;
                }
                if let Some(c) = fill_color
                    && sheet.fill_color != c
                {
                    sheet.fill_color = c;
                    changed = true;
                }
                if !changed {
                    return Ok(CommandResult::unchanged());
                }
                // MD-11: stroke width / colour / fill colour are pure
                // style changes, not text edits. `StyleUpdated` lets
                // consumers skip text-reflow work.
                let patch_pair = PatchPair {
                    semantic: SemanticPatch::StyleUpdated,
                    document: DocumentPatch::CHILD_SHEETS,
                };
                self.record_history(before, patch_pair);
                Ok(CommandResult::changed(patch_pair))
            }
            Command::AnnotateAll { mode } => {
                use crate::command::AnnotateMode;
                // Power ports (is_power == true, or reference starting with '#')
                // are net anchors, not real components. Their references
                // carry the net name, not a designator. Skip them in every
                // phase so annotation only touches real parts.
                let is_designator_target = |sym: &signex_types::schematic::Symbol| -> bool {
                    !sym.is_power && !sym.reference.starts_with('#')
                };
                // Phase 1: optionally reset existing numbers back to '?'.
                if matches!(
                    mode,
                    AnnotateMode::ResetOnly | AnnotateMode::ResetAndRenumber
                ) {
                    for symbol in self.document.symbols.iter_mut() {
                        if !is_designator_target(symbol) {
                            continue;
                        }
                        // Keep the alphabetic prefix; drop the digit tail.
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
                    return Ok(CommandResult::changed(patch_pair));
                }

                // Phase 2: find the max number per prefix from already-annotated
                // symbols so Incremental doesn't collide. Skip power ports
                // — their numbers use a different (#PWR) namespace.
                let mut next_by_prefix: std::collections::HashMap<String, u32> =
                    std::collections::HashMap::new();
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
                    let rest = &symbol.reference[prefix.len()..];
                    if let Ok(n) = rest.parse::<u32>() {
                        let entry = next_by_prefix.entry(prefix).or_insert(0);
                        if n > *entry {
                            *entry = n;
                        }
                    }
                }

                // Phase 3: iterate symbols in a stable order (by position,
                // then uuid) and assign sequential numbers to any '?' tails.
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
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SymbolFieldsUpdated,
                    document: DocumentPatch::SYMBOLS,
                };
                self.record_history(before, patch_pair);
                Ok(CommandResult::changed(patch_pair))
            }
            Command::MoveSymbolAbsolute { symbol_id, x, y } => {
                let Some(symbol) = self
                    .document
                    .symbols
                    .iter_mut()
                    .find(|s| s.uuid == symbol_id)
                else {
                    return Ok(CommandResult::unchanged());
                };
                let dx = x - symbol.position.x;
                let dy = y - symbol.position.y;
                if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
                    return Ok(CommandResult::unchanged());
                }
                symbol.position.x = x;
                symbol.position.y = y;
                if let Some(rt) = symbol.ref_text.as_mut() {
                    rt.position.x += dx;
                    rt.position.y += dy;
                }
                if let Some(vt) = symbol.val_text.as_mut() {
                    vt.position.x += dx;
                    vt.position.y += dy;
                }
                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SelectionMoved,
                    document: DocumentPatch::SYMBOLS,
                };
                self.record_history(before, patch_pair);
                Ok(CommandResult::changed(patch_pair))
            }
            Command::ReorderObjects { items, direction } => {
                use crate::command::ReorderDirection;
                use signex_types::schematic::SelectedKind;
                if items.is_empty() {
                    return Ok(CommandResult::unchanged());
                }

                // Helper: given a Vec and a reference uuid, return the
                // insert position for JustAbove (ref_idx + 1) or
                // JustBelow (ref_idx). Returns None when the reference
                // uuid isn't in the Vec — caller falls back to
                // no-change.
                fn reorder_slot<T>(
                    vec: &[T],
                    uuid_of: impl Fn(&T) -> uuid::Uuid,
                    direction: ReorderDirection,
                ) -> Option<usize> {
                    match direction {
                        ReorderDirection::ToFront => Some(vec.len()),
                        ReorderDirection::ToBack => Some(0),
                        ReorderDirection::JustAbove(target) => {
                            vec.iter().position(|x| uuid_of(x) == target).map(|i| i + 1)
                        }
                        ReorderDirection::JustBelow(target) => {
                            vec.iter().position(|x| uuid_of(x) == target)
                        }
                    }
                }

                let mut changed = false;
                for item in &items {
                    match item.kind {
                        SelectedKind::Symbol => {
                            if let Some(idx) = self
                                .document
                                .symbols
                                .iter()
                                .position(|s| s.uuid == item.uuid)
                            {
                                let sym = self.document.symbols.remove(idx);
                                if let Some(mut slot) =
                                    reorder_slot(&self.document.symbols, |s| s.uuid, direction)
                                {
                                    slot = slot.min(self.document.symbols.len());
                                    self.document.symbols.insert(slot, sym);
                                    changed = true;
                                } else {
                                    // Reference uuid missing — restore item at its
                                    // original slot so we don't drop it silently.
                                    self.document.symbols.insert(idx, sym);
                                }
                            }
                        }
                        SelectedKind::Wire => {
                            if let Some(idx) =
                                self.document.wires.iter().position(|w| w.uuid == item.uuid)
                            {
                                let w = self.document.wires.remove(idx);
                                if let Some(mut slot) =
                                    reorder_slot(&self.document.wires, |x| x.uuid, direction)
                                {
                                    slot = slot.min(self.document.wires.len());
                                    self.document.wires.insert(slot, w);
                                    changed = true;
                                } else {
                                    self.document.wires.insert(idx, w);
                                }
                            }
                        }
                        SelectedKind::Label => {
                            if let Some(idx) = self
                                .document
                                .labels
                                .iter()
                                .position(|l| l.uuid == item.uuid)
                            {
                                let l = self.document.labels.remove(idx);
                                if let Some(mut slot) =
                                    reorder_slot(&self.document.labels, |x| x.uuid, direction)
                                {
                                    slot = slot.min(self.document.labels.len());
                                    self.document.labels.insert(slot, l);
                                    changed = true;
                                } else {
                                    self.document.labels.insert(idx, l);
                                }
                            }
                        }
                        SelectedKind::TextNote => {
                            if let Some(idx) = self
                                .document
                                .text_notes
                                .iter()
                                .position(|t| t.uuid == item.uuid)
                            {
                                let t = self.document.text_notes.remove(idx);
                                if let Some(mut slot) =
                                    reorder_slot(&self.document.text_notes, |x| x.uuid, direction)
                                {
                                    slot = slot.min(self.document.text_notes.len());
                                    self.document.text_notes.insert(slot, t);
                                    changed = true;
                                } else {
                                    self.document.text_notes.insert(idx, t);
                                }
                            }
                        }
                        // Signex schematics have no explicit z-order for
                        // drawings, junctions, no-connects, or bus entries —
                        // render order is file order. Left out intentionally.
                        _ => {}
                    }
                }
                if !changed {
                    return Ok(CommandResult::unchanged());
                }
                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SelectionMoved,
                    document: DocumentPatch::FULL,
                };
                self.record_history(before, patch_pair);
                Ok(CommandResult::changed(patch_pair))
            }
            Command::ReconcileChildSheetPins {
                child_filename,
                ports,
            } => {
                let mut changed = false;
                for child in self
                    .document
                    .child_sheets
                    .iter_mut()
                    .filter(|child| child.filename == child_filename)
                {
                    changed |= sheet::reconcile_child_sheet_pins(child, &ports);
                }

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SheetPinsReconciled,
                    document: DocumentPatch::CHILD_SHEETS,
                };
                self.record_history(before, patch_pair);
                Ok(CommandResult::changed(patch_pair))
            }
            Command::SetPaperSize { paper_size } => {
                if self.document.paper_size == paper_size {
                    return Ok(CommandResult::unchanged());
                }
                self.document.paper_size = paper_size;

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::StyleUpdated,
                    document: DocumentPatch::PAPER,
                };
                self.record_history(before, patch_pair);
                Ok(CommandResult::changed(patch_pair))
            }
            _ => unreachable!(),
        }
    }
}
