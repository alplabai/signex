//! What the status bar says about the gesture in flight: a drag's wire
//! behaviour (#1266) or the Measure tool's prompt (#1256).

use super::super::super::*;
use crate::canvas::FLIP_WIRES_MODIFIER;

impl Signex {
    /// How the drag in flight in `window` treats wires, `None` outside a drag.
    pub(crate) fn move_status(&self, window_id: iced::window::Id) -> Option<String> {
        let keeps = self
            .interaction_state
            .canvas_for_window(window_id)
            .move_behaviour
            .get()?;
        Some(if keeps {
            format!("Dragging with wires ({FLIP_WIRES_MODIFIER}: plain move)")
        } else {
            format!("Moving without wires ({FLIP_WIRES_MODIFIER}: drag with wires)")
        })
    }

    /// The one activity segment the status bar shows.
    pub(crate) fn status_activity(&self, window_id: iced::window::Id) -> Option<String> {
        // A symbol editor's own segment; the schematic gestures behind it
        // are not what this window shows.
        if let Some(editor) = self.status_symbol_editor(window_id) {
            return Some(crate::library::editor::symbol::status::segment(
                editor,
                &self.ui_state.i18n,
            ));
        }
        self.move_status(window_id)
            .or_else(|| self.measure_status(window_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #2262: the symbol editor's part, pins and hints sit in the global
    /// status bar, not in a strip of its own; a schematic drag behind the
    /// tab does not show.
    #[test]
    fn a_symbol_editor_tab_shows_its_own_segment() {
        let (mut app, _task) = Signex::new();
        let window = iced::window::Id::unique();
        app.ui_state.main_window_id = Some(window);
        let path = std::path::PathBuf::from("s.snxsym");
        app.document_state.tabs.push(crate::app::TabInfo {
            title: "s".into(),
            path: path.clone(),
            cached_document: None,
            project_id: None,
            kind: crate::app::TabKind::SymbolEditor(path.clone()),
        });
        app.document_state.active_tab = app.document_state.tabs.len() - 1;
        let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("s"));
        let editor = SymbolDocument::new(path.clone(), file);
        let expected = crate::library::editor::symbol::status::segment(&editor, &app.ui_state.i18n);
        app.document_state.symbol_editors.insert(path, editor);
        app.interaction_state
            .canvas_for_window(window)
            .move_behaviour
            .set(Some(true));

        assert_eq!(app.status_activity(window), Some(expected));
    }

    #[test]
    fn the_status_names_the_drag_behaviour_and_the_flip_modifier() {
        let (app, _task) = Signex::new();
        let window = iced::window::Id::unique();
        assert!(
            app.status_activity(window).is_none(),
            "no gesture, no segment"
        );

        let slot = app.interaction_state.canvas_for_window(window);
        slot.move_behaviour.set(Some(true));
        assert_eq!(
            app.status_activity(window).as_deref(),
            Some("Dragging with wires (Alt: plain move)")
        );
        slot.move_behaviour.set(Some(false));
        assert_eq!(
            app.status_activity(window).as_deref(),
            Some("Moving without wires (Alt: drag with wires)")
        );
    }
}
