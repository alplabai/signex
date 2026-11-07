//! Applying the `undo_depth` setting to every undo history of the app.

use signex_history::undo_depth_in_range;

use super::Signex;

impl Signex {
    /// Set how many undo steps every history keeps. `false`, and nothing
    /// changed, when `depth` is outside `1..=MAX_UNDO_DEPTH`. Lowering it
    /// drops the oldest steps at once; a save point that falls off the bottom
    /// leaves that document dirty until its next save.
    pub fn set_undo_depth(&mut self, depth: usize) -> bool {
        if !undo_depth_in_range(depth) {
            return false;
        }
        self.document_state.undo_depth = depth;
        self.sync_undo_depth();
        true
    }

    /// Bring every history to the current depth, the ones a document opened
    /// since the last call included. A history already at the depth is not
    /// touched, so this runs after every update.
    pub(crate) fn sync_undo_depth(&mut self) {
        let depth = self.document_state.undo_depth;
        let docs = &mut self.document_state;
        for engine in docs.engines.values_mut() {
            engine.set_undo_depth(depth);
        }
        for editor in docs.symbol_editors.values_mut() {
            editor.history.set_cap(depth);
        }
        for editor in docs.footprint_editors.values_mut() {
            editor.history.set_cap(depth);
        }
        for history in docs.net_class_history.values_mut() {
            history.set_undo_depth(depth);
        }
        for history in docs.rules_history.values_mut() {
            history.set_undo_depth(depth);
        }
        for tab in docs.project_tabs.values_mut() {
            tab.general_history.set_undo_depth(depth);
        }
        for preview in self.library.editors.values_mut() {
            preview.history.set_undo_depth(depth);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use signex_types::schematic::SchematicSheet;

    use crate::app::{FootprintDocument, Message, Signex, SymbolDocument};

    fn app_with_every_history() -> Signex {
        let (mut app, _boot) = Signex::new();
        let docs = &mut app.document_state;
        let sheet = PathBuf::from("/p/a.snxsch");
        docs.engines.insert(
            sheet,
            signex_engine::Engine::new(SchematicSheet::default()).unwrap(),
        );
        let symbol = PathBuf::from("/p/a.snxsym");
        let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("S"));
        docs.symbol_editors
            .insert(symbol.clone(), SymbolDocument::new(symbol, file));
        let footprint = PathBuf::from("/p/a.snxfpt");
        let file =
            signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("F"));
        docs.footprint_editors
            .insert(footprint.clone(), FootprintDocument::new(footprint, file));
        let project = crate::app::state::ProjectId::test_id(1);
        docs.net_class_history.insert(project, Default::default());
        docs.rules_history.insert(project, Default::default());
        docs.project_tabs.insert(project, Default::default());
        app
    }

    fn depths(app: &Signex) -> Vec<usize> {
        let d = &app.document_state;
        let mut all = Vec::new();
        all.extend(d.engines.values().map(|e| e.undo_depth()));
        all.extend(d.symbol_editors.values().map(|e| e.history.cap()));
        all.extend(d.footprint_editors.values().map(|e| e.history.cap()));
        all.extend(d.net_class_history.values().map(|h| h.undo_depth()));
        all.extend(d.rules_history.values().map(|h| h.undo_depth()));
        all.extend(
            d.project_tabs
                .values()
                .map(|t| t.general_history.undo_depth()),
        );
        all
    }

    #[test]
    fn the_setting_reaches_every_kind_of_history_and_refuses_a_bad_depth() {
        let mut app = app_with_every_history();
        assert!(depths(&app).iter().all(|d| *d == 100), "{:?}", depths(&app));
        assert!(app.set_undo_depth(7));
        assert_eq!(depths(&app), vec![7; 6]);
        for bad in [0, 1001] {
            assert!(!app.set_undo_depth(bad));
        }
        assert_eq!(depths(&app), vec![7; 6], "a refused depth changes nothing");
    }

    #[test]
    fn a_document_opened_later_gets_the_depth_on_the_next_update() {
        let mut app = app_with_every_history();
        assert!(app.set_undo_depth(5));
        app.document_state.engines.insert(
            PathBuf::from("/p/b.snxsch"),
            signex_engine::Engine::new(SchematicSheet::default()).unwrap(),
        );
        let _task = app.update(Message::noop());
        assert!(
            app.document_state
                .engines
                .values()
                .all(|e| e.undo_depth() == 5),
            "every engine follows the setting"
        );
    }
}
