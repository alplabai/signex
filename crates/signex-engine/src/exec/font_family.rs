//! `Command::SetTextFontFamily` — one text item's font family.

use crate::*;
use signex_types::font::normalize_family;

impl Engine {
    pub(super) fn set_text_font_family(
        &mut self,
        before: SchematicSheet,
        target: TextTarget,
        font_family: Option<String>,
    ) -> Result<CommandResult, EngineError> {
        // A blank name is no name: clearing and "" mean the same thing.
        let font_family = normalize_family(font_family);

        let (slot, document_patch) = match target {
            TextTarget::Label(uuid) => (
                self.document
                    .labels
                    .iter_mut()
                    .find(|label| label.uuid == uuid)
                    .map(|label| &mut label.font_family),
                DocumentPatch::LABELS,
            ),
            TextTarget::TextNote(uuid) => (
                self.document
                    .text_notes
                    .iter_mut()
                    .find(|note| note.uuid == uuid)
                    .map(|note| &mut note.font_family),
                DocumentPatch::TEXT_NOTES,
            ),
            TextTarget::SymbolReference(uuid) => {
                let symbol = self.document.symbols.iter_mut().find(|s| s.uuid == uuid);
                let slot = match symbol {
                    // A symbol with no stored text property has nowhere to
                    // keep a family: refuse, do not silently ignore.
                    Some(symbol) => Some(
                        symbol
                            .ref_text
                            .as_mut()
                            .map(|prop| &mut prop.font_family)
                            .ok_or(EngineError::NoTextToStyle("reference"))?,
                    ),
                    None => None,
                };
                (slot, DocumentPatch::SYMBOLS)
            }
            TextTarget::SymbolValue(uuid) => {
                let symbol = self.document.symbols.iter_mut().find(|s| s.uuid == uuid);
                let slot = match symbol {
                    Some(symbol) => Some(
                        symbol
                            .val_text
                            .as_mut()
                            .map(|prop| &mut prop.font_family)
                            .ok_or(EngineError::NoTextToStyle("value"))?,
                    ),
                    None => None,
                };
                (slot, DocumentPatch::SYMBOLS)
            }
        };

        let Some(slot) = slot else {
            return Ok(CommandResult::unchanged());
        };
        if *slot == font_family {
            return Ok(CommandResult::unchanged());
        }
        *slot = font_family;

        let patch_pair = PatchPair {
            semantic: SemanticPatch::StyleUpdated,
            document: document_patch,
        };
        self.record_history(before, patch_pair);
        Ok(CommandResult::changed(patch_pair))
    }
}
