//! The designators a canvas draws for the placement its tab was opened
//! through (#1151).
//!
//! A child sheet placed more than once carries a designator per placement
//! (#829). Descending through a sheet symbol records that placement for the
//! tab (#1133), and the export prints its designators; the canvas draws the
//! same ones, so the screen matches the page.

use std::collections::HashMap;

use super::super::*;

impl Signex {
    /// Re-resolve the active canvas's designators, clearing its content cache
    /// only when they changed.
    pub(crate) fn refresh_canvas_designators(&mut self) {
        let designators = match self.document_state.active_path.clone() {
            Some(path) => self.designators_for_path(&path),
            None => HashMap::new(),
        };
        let canvas = self.interaction_state.active_canvas_mut();
        if canvas.designators != designators {
            canvas.designators = designators;
            canvas.clear_content_cache();
        }
    }

    /// The designators the schematic at `path` shows at its tab's recorded
    /// placement, for the symbols where they differ from `reference`. Empty
    /// when the tab has no recorded placement or `path` has no engine.
    pub(crate) fn designators_for_path(
        &self,
        path: &std::path::Path,
    ) -> HashMap<uuid::Uuid, String> {
        let document_state = &self.document_state;
        let (Some(placement), Some(engine)) = (
            document_state.tab_placements.get(path),
            document_state.engines.get(path),
        ) else {
            return HashMap::new();
        };
        engine
            .document()
            .symbols
            .iter()
            .filter(|symbol| symbol.retargets_designator_in(placement))
            .map(|symbol| (symbol.uuid, symbol.designator_in(placement).to_string()))
            .collect()
    }

    /// The command that sets `symbol`'s designator to `value` on the active
    /// tab: the instance entry of the tab's recorded placement when it has
    /// one, so the edit changes what this tab shows and exports, else the
    /// symbol's own `reference`.
    pub(crate) fn designator_edit_command(
        &self,
        symbol: uuid::Uuid,
        value: String,
    ) -> signex_engine::Command {
        let placement = self
            .document_state
            .active_path
            .as_deref()
            .and_then(|path| self.document_state.tab_placements.get(path));
        match placement {
            Some(placement) => signex_engine::Command::SetSymbolDesignatorIn {
                symbol,
                placement: placement.clone(),
                reference: value,
            },
            None => signex_engine::Command::UpdateText {
                target: signex_engine::TextTarget::SymbolReference(symbol),
                value,
            },
        }
    }

    /// `symbol`'s designator as the active tab shows it.
    pub(crate) fn shown_designator<'a>(
        &'a self,
        symbol: &'a signex_types::schematic::Symbol,
    ) -> &'a str {
        self.interaction_state
            .active_canvas()
            .designators
            .get(&symbol.uuid)
            .map_or(symbol.reference.as_str(), String::as_str)
    }
}
