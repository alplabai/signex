//! `Engine::exec_edits` — see `exec/mod.rs`.

use crate::*;

impl Engine {
    /// Write one boolean symbol flag; a missing symbol or an unchanged
    /// value is a no-op that records no history entry.
    fn set_symbol_flag(
        &mut self,
        before: SchematicSheet,
        symbol_id: uuid::Uuid,
        flag: impl Fn(&mut signex_types::schematic::Symbol) -> &mut bool,
        value: bool,
    ) -> Result<CommandResult, EngineError> {
        let changed = self
            .document
            .symbols
            .iter_mut()
            .find(|s| s.uuid == symbol_id)
            .map(|s| {
                let slot = flag(s);
                let differs = *slot != value;
                *slot = value;
                differs
            })
            .unwrap_or(false);

        if !changed {
            return Ok(CommandResult::unchanged());
        }

        let patch_pair = PatchPair {
            semantic: SemanticPatch::SymbolFieldsUpdated,
            document: DocumentPatch::SYMBOLS,
        };
        self.record_history(before, patch_pair);
        Ok(CommandResult::changed(patch_pair))
    }

    pub(crate) fn exec_edits(
        &mut self,
        before: SchematicSheet,
        cmd: Command,
    ) -> Result<CommandResult, EngineError> {
        match cmd {
            Command::ReplaceDocument { document } => {
                self.document = document;

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::DocumentReplaced,
                    document: DocumentPatch::FULL,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::UpdateText { target, value } => {
                let document_patch = match target {
                    TextTarget::Label(_) => DocumentPatch::LABELS,
                    TextTarget::TextNote(_) => DocumentPatch::TEXT_NOTES,
                    TextTarget::SymbolReference(_) | TextTarget::SymbolValue(_) => {
                        DocumentPatch::SYMBOLS
                    }
                };

                let changed = match target {
                    TextTarget::Label(uuid) => self
                        .document
                        .labels
                        .iter_mut()
                        .find(|label| label.uuid == uuid)
                        .map(|label| {
                            if label.text == value {
                                false
                            } else {
                                label.text = value;
                                true
                            }
                        })
                        .unwrap_or(false),
                    TextTarget::TextNote(uuid) => self
                        .document
                        .text_notes
                        .iter_mut()
                        .find(|text_note| text_note.uuid == uuid)
                        .map(|text_note| {
                            if text_note.text == value {
                                false
                            } else {
                                text_note.text = value;
                                true
                            }
                        })
                        .unwrap_or(false),
                    TextTarget::SymbolReference(uuid) => self
                        .document
                        .symbols
                        .iter_mut()
                        .find(|symbol| symbol.uuid == uuid)
                        .map(|symbol| {
                            if symbol.reference == value {
                                false
                            } else {
                                symbol.reference = value;
                                true
                            }
                        })
                        .unwrap_or(false),
                    TextTarget::SymbolValue(uuid) => self
                        .document
                        .symbols
                        .iter_mut()
                        .find(|symbol| symbol.uuid == uuid)
                        .map(|symbol| {
                            if symbol.value == value {
                                false
                            } else {
                                symbol.value = value;
                                true
                            }
                        })
                        .unwrap_or(false),
                };

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::TextUpdated,
                    document: document_patch,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::UpdateLabelProps {
                label_id,
                font_size_mm,
                justify,
                rotation_degrees,
            } => {
                let changed = self
                    .document
                    .labels
                    .iter_mut()
                    .find(|l| l.uuid == label_id)
                    .map(|l| {
                        let mut any = false;
                        if let Some(fs) = font_size_mm
                            && (l.font_size - fs).abs() > 1e-6
                        {
                            l.font_size = fs;
                            any = true;
                        }
                        if let Some(j) = justify
                            && l.justify != j
                        {
                            l.justify = j;
                            any = true;
                        }
                        if let Some(r) = rotation_degrees
                            && (l.rotation - r).abs() > 1e-6
                        {
                            l.rotation = r;
                            any = true;
                        }
                        any
                    })
                    .unwrap_or(false);

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::LabelsMutated,
                    document: DocumentPatch::LABELS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::SetSymbolRotation {
                symbol_id,
                rotation_degrees,
            } => {
                let changed = self
                    .document
                    .symbols
                    .iter_mut()
                    .find(|s| s.uuid == symbol_id)
                    .map(|s| {
                        if (s.rotation - rotation_degrees).abs() < 1e-6 {
                            false
                        } else {
                            s.rotation = rotation_degrees;
                            true
                        }
                    })
                    .unwrap_or(false);

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SelectionRotated,
                    document: DocumentPatch::SYMBOLS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::SetSymbolLocked { symbol_id, locked } => {
                self.set_symbol_flag(before, symbol_id, |s| &mut s.locked, locked)
            }
            Command::SetSymbolDnp { symbol_id, dnp } => {
                self.set_symbol_flag(before, symbol_id, |s| &mut s.dnp, dnp)
            }
            Command::UpdateSymbolTextSize {
                symbol_id,
                field,
                font_size_mm,
            } => {
                use command::SymbolTextField;
                let changed = self
                    .document
                    .symbols
                    .iter_mut()
                    .find(|s| s.uuid == symbol_id)
                    .map(|s| {
                        let tp = match field {
                            SymbolTextField::Reference => s.ref_text.as_mut(),
                            SymbolTextField::Value => s.val_text.as_mut(),
                        };
                        if let Some(tp) = tp
                            && (tp.font_size - font_size_mm).abs() > 1e-6
                        {
                            tp.font_size = font_size_mm;
                            true
                        } else {
                            false
                        }
                    })
                    .unwrap_or(false);

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SymbolFieldsUpdated,
                    document: DocumentPatch::SYMBOLS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::UpdateSymbolLibId { symbol_id, lib_id } => {
                let changed = self
                    .document
                    .symbols
                    .iter_mut()
                    .find(|s| s.uuid == symbol_id)
                    .map(|s| {
                        if s.lib_id == lib_id {
                            false
                        } else {
                            s.lib_id = lib_id;
                            true
                        }
                    })
                    .unwrap_or(false);

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SymbolFieldsUpdated,
                    document: DocumentPatch::SYMBOLS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::UpdateSymbolFootprint {
                symbol_id,
                footprint,
            } => {
                let changed = self
                    .document
                    .symbols
                    .iter_mut()
                    .find(|symbol| symbol.uuid == symbol_id)
                    .map(|symbol| {
                        if symbol.footprint == footprint {
                            false
                        } else {
                            symbol.footprint = footprint;
                            true
                        }
                    })
                    .unwrap_or(false);

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SymbolFieldsUpdated,
                    document: DocumentPatch::SYMBOLS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            Command::SetSymbolField {
                symbol_id,
                key,
                value,
            } => {
                // Reject overwrites of reserved keys — those have their
                // own commands so ref/value/footprint moves stay tracked.
                match key.to_lowercase().as_str() {
                    "reference" | "value" | "footprint" => {
                        return Ok(CommandResult::unchanged());
                    }
                    _ => {}
                }
                let changed = self
                    .document
                    .symbols
                    .iter_mut()
                    .find(|s| s.uuid == symbol_id)
                    .map(|symbol| {
                        if value.is_empty() {
                            symbol.fields.remove(&key).is_some()
                        } else if symbol.fields.get(&key) == Some(&value) {
                            false
                        } else {
                            symbol.fields.insert(key, value);
                            true
                        }
                    })
                    .unwrap_or(false);
                if !changed {
                    return Ok(CommandResult::unchanged());
                }
                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SymbolFieldsUpdated,
                    document: DocumentPatch::SYMBOLS,
                };
                self.record_history(before, patch_pair);
                Ok(CommandResult::changed(patch_pair))
            }
            Command::UpdateSymbolFields {
                symbol_id,
                reference,
                value,
                footprint,
            } => {
                let changed = self
                    .document
                    .symbols
                    .iter_mut()
                    .find(|symbol| symbol.uuid == symbol_id)
                    .map(|symbol| {
                        let changed = symbol.reference != reference
                            || symbol.value != value
                            || symbol.footprint != footprint;

                        if changed {
                            symbol.reference = reference;
                            symbol.value = value;
                            symbol.footprint = footprint;
                        }

                        changed
                    })
                    .unwrap_or(false);

                if !changed {
                    return Ok(CommandResult::unchanged());
                }

                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SymbolFieldsUpdated,
                    document: DocumentPatch::SYMBOLS,
                };

                self.record_history(before, patch_pair);

                Ok(CommandResult::changed(patch_pair))
            }
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_sheet;

    fn engine_with_symbol() -> (Engine, uuid::Uuid) {
        let uuid = uuid::Uuid::new_v4();
        let mut sheet = test_sheet();
        sheet.symbols.push(signex_types::schematic::Symbol {
            uuid,
            lib_id: "Device:R".to_string(),
            reference: "R1".to_string(),
            value: "10k".to_string(),
            position: signex_types::schematic::Point::new(0.0, 0.0),
            ..Default::default()
        });
        (Engine::new(sheet).unwrap(), uuid)
    }

    #[test]
    fn set_symbol_dnp_applies_and_one_undo_reverts() {
        let (mut engine, id) = engine_with_symbol();
        let r = engine
            .execute(Command::SetSymbolDnp {
                symbol_id: id,
                dnp: true,
            })
            .unwrap();
        assert!(r.changed);
        assert!(engine.document().symbols[0].dnp);
        assert!(!engine.document().symbols[0].locked);
        assert!(engine.undo().unwrap().is_some());
        assert!(!engine.document().symbols[0].dnp);
        assert!(!engine.can_undo());
    }

    #[test]
    fn set_symbol_locked_applies_and_one_undo_reverts() {
        let (mut engine, id) = engine_with_symbol();
        let r = engine
            .execute(Command::SetSymbolLocked {
                symbol_id: id,
                locked: true,
            })
            .unwrap();
        assert!(r.changed);
        assert!(engine.document().symbols[0].locked);
        assert!(!engine.document().symbols[0].dnp);
        assert!(engine.undo().unwrap().is_some());
        assert!(!engine.document().symbols[0].locked);
        assert!(!engine.can_undo());
    }

    #[test]
    fn unchanged_or_unknown_symbol_flag_records_no_history() {
        let (mut engine, id) = engine_with_symbol();
        let same = engine
            .execute(Command::SetSymbolDnp {
                symbol_id: id,
                dnp: false,
            })
            .unwrap();
        assert!(!same.changed);
        let missing = engine
            .execute(Command::SetSymbolLocked {
                symbol_id: uuid::Uuid::new_v4(),
                locked: true,
            })
            .unwrap();
        assert!(!missing.changed);
        assert!(!engine.can_undo());
    }
}
