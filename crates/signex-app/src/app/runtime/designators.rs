//! The designators a canvas draws for the placement its tab was opened
//! through (#1151).
//!
//! A child sheet placed more than once carries a designator per placement
//! (#829). Descending through a sheet symbol records that placement for the
//! tab (#1133), and the export prints its designators; the canvas draws the
//! same ones, so the screen matches the page.

use std::collections::HashMap;

use signex_types::schematic::InstancePath;

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

    /// The placement whose designators the schematic at `path` shows and
    /// edits (#1200):
    /// - the tab's recorded placement while the walk still reaches the sheet
    ///   through it;
    /// - else the default placement, as the "Current page" export picks it,
    ///   so a child opened from the Projects panel shows what it prints;
    /// - `None` when the walk does not reach the sheet (or no project
    ///   hierarchy is known for it), and then `reference` is shown.
    ///
    /// The Navigator's hierarchy describes the focused document's project, so
    /// it is consulted only for the focused document; another path (an
    /// undocked tab) keeps its recorded placement.
    pub(crate) fn shown_placement(&self, path: &std::path::Path) -> Option<InstancePath> {
        let document_state = &self.document_state;
        let recorded = document_state.tab_placements.get(path);
        let hierarchy = document_state
            .panel_ctx
            .navigator
            .hierarchy
            .as_ref()
            .filter(|_| document_state.active_path.as_deref() == Some(path));
        let Some(hierarchy) = hierarchy else {
            return recorded.cloned();
        };
        let live = hierarchy.placements_of(path);
        match recorded {
            Some(recorded) if live.contains(recorded) => Some(recorded.clone()),
            _ => crate::app::state::default_placement(live),
        }
    }

    /// The designators the schematic at `path` shows at its
    /// [shown placement](Self::shown_placement), for the symbols where they
    /// differ from `reference`. Empty when there is no such placement or
    /// `path` has no engine.
    pub(crate) fn designators_for_path(
        &self,
        path: &std::path::Path,
    ) -> HashMap<uuid::Uuid, String> {
        let (Some(placement), Some(engine)) = (
            self.shown_placement(path),
            self.document_state.engines.get(path),
        ) else {
            return HashMap::new();
        };
        engine
            .document()
            .symbols
            .iter()
            .filter(|symbol| symbol.retargets_designator_in(&placement))
            .map(|symbol| (symbol.uuid, symbol.designator_in(&placement).to_string()))
            .collect()
    }

    /// The commands that set `symbol`'s designator to `value` on the active
    /// tab, applied as one undo step:
    /// - on a sheet placed more than once, the instance entry of the shown
    ///   placement, so the edit changes what this tab shows and exports and
    ///   the other placements keep theirs;
    /// - on a sheet placed once (or not at all), the symbol's own
    ///   `reference`, dropping any entry for that one placement, so a simple
    ///   design keeps a simple file (#1200).
    pub(crate) fn designator_edit_commands(
        &self,
        symbol: uuid::Uuid,
        value: String,
    ) -> Vec<signex_engine::Command> {
        let active = self.document_state.active_path.as_deref();
        let placement = active.and_then(|path| self.shown_placement(path));
        let placed_more_than_once = active.is_some_and(|path| {
            match self.document_state.panel_ctx.navigator.hierarchy.as_ref() {
                Some(hierarchy) => hierarchy.placements_of(path).len() > 1,
                // Without a hierarchy, a recorded placement is all there is
                // to go on; keep editing it, as before.
                None => self.document_state.tab_placements.get(path).is_some(),
            }
        });
        let set_reference = signex_engine::Command::UpdateText {
            target: signex_engine::TextTarget::SymbolReference(symbol),
            value: value.clone(),
        };
        match placement {
            Some(placement) if placed_more_than_once => {
                vec![signex_engine::Command::SetSymbolDesignatorIn {
                    symbol,
                    placement,
                    reference: value,
                }]
            }
            Some(placement) if !placement.is_root() => vec![
                set_reference,
                signex_engine::Command::SetSymbolDesignatorIn {
                    symbol,
                    placement,
                    reference: String::new(),
                },
            ],
            _ => vec![set_reference],
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
