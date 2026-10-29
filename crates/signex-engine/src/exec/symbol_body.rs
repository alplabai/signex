//! `Command::SetSymbolBodyStyle` (#1233): switch a placed symbol between the
//! normal and the alternate (De Morgan) body of its library symbol, refusing
//! when that would disturb a wired pin.

use super::pin_changes::{instance_pin_changes, shown_pins};
use crate::*;
use signex_net::connected_points;
use signex_types::schematic::BodyStyle;
use uuid::Uuid;

impl Engine {
    pub(super) fn set_symbol_body_style(
        &mut self,
        before: SchematicSheet,
        symbol_id: Uuid,
        body_style: BodyStyle,
    ) -> Result<CommandResult, EngineError> {
        let Some(sym) = self.document.symbols.iter().find(|s| s.uuid == symbol_id) else {
            return Ok(CommandResult::unchanged());
        };
        if sym.body_style == body_style {
            // Choosing the style already shown still replaces an unreadable
            // value kept on load.
            if let Some(sym) = self
                .document
                .symbols
                .iter_mut()
                .find(|s| s.uuid == symbol_id)
                .filter(|s| s.unknown.contains_key("body_style"))
            {
                sym.unknown.remove("body_style");
                let patch_pair = PatchPair {
                    semantic: SemanticPatch::SymbolFieldsUpdated,
                    document: DocumentPatch::SYMBOLS,
                };
                self.record_history(before, patch_pair);
                return Ok(CommandResult::changed(patch_pair));
            }
            return Ok(CommandResult::unchanged());
        }

        // Judge the switch on the pins the instance shows before and after,
        // so a lib symbol with no alternate (nothing changes on screen) and a
        // switch that only moves unwired pins both go through.
        if let Some(lib) = self.document.lib_symbols.get(&sym.lib_id) {
            let mut proposed = sym.clone();
            proposed.body_style = body_style;
            let old = shown_pins(Some(lib), sym);
            let new = shown_pins(Some(lib), &proposed);
            let gate = connected_points(&self.document);
            let changes = instance_pin_changes(&self.document, &gate, sym, &old, &new);
            let changed: Vec<String> = changes
                .lost
                .iter()
                .chain(changes.gained.iter())
                .map(|key| format!("{} pin {key}", sym.reference))
                .collect();
            if !changed.is_empty() {
                return Err(EngineError::SymbolBodyPinsConnected {
                    reference: sym.reference.clone(),
                    pins: changed,
                });
            }
        }

        if let Some(sym) = self
            .document
            .symbols
            .iter_mut()
            .find(|s| s.uuid == symbol_id)
        {
            sym.body_style = body_style;
            // An explicit choice replaces any unreadable value kept on load.
            sym.unknown.remove("body_style");
        }
        let patch_pair = PatchPair {
            semantic: SemanticPatch::SymbolFieldsUpdated,
            document: DocumentPatch::SYMBOLS,
        };
        self.record_history(before, patch_pair);
        Ok(CommandResult::changed(patch_pair))
    }
}
