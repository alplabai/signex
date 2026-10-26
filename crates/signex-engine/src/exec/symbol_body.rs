//! `Command::SetSymbolBodyStyle` (#1233): switch a placed symbol between the
//! normal and the alternate (De Morgan) body of its library symbol, refusing
//! when that would disturb a wired pin.

use super::connected_pins::connected_pins_changed;
use crate::*;
use signex_types::schematic::{BodyStyle, instance_pins};
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
            return Ok(CommandResult::unchanged());
        }

        // Judge the switch on the pins the instance shows before and after,
        // so a lib symbol with no alternate (nothing changes on screen) and a
        // switch that only moves unwired pins both go through.
        if let Some(lib) = self.document.lib_symbols.get(&sym.lib_id) {
            let mut proposed = sym.clone();
            proposed.body_style = body_style;
            let old: Vec<_> = instance_pins(lib, sym).collect();
            let new: Vec<_> = instance_pins(lib, &proposed).collect();
            let changed = connected_pins_changed(&self.document, std::iter::once((sym, old, new)));
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
