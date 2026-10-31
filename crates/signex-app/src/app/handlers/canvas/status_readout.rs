//! The status bar's cursor X/Y and zoom readouts: each window asks the
//! canvas it is showing, never a stored copy that another canvas wrote
//! (#1789).

use super::super::super::*;

/// The surface a window's canvas area is showing.
enum Surface<'a> {
    SymbolEditor(&'a SymbolDocument),
    FootprintEditor(&'a FootprintDocument),
    Pcb,
    Schematic,
}

impl Signex {
    /// What `window_id` shows. Standalone symbol and footprint editors are
    /// main-window tabs; an undocked window holds one schematic or PCB.
    fn status_surface(&self, window_id: iced::window::Id) -> Surface<'_> {
        let docs = &self.document_state;
        if self.ui_state.main_window_id == Some(window_id)
            && let Some(tab) = docs.tabs.get(docs.active_tab)
        {
            if let Some(editor) = tab
                .kind
                .as_symbol_editor()
                .and_then(|path| docs.symbol_editors.get(path))
            {
                return Surface::SymbolEditor(editor);
            }
            if let Some(editor) = tab
                .kind
                .as_footprint_editor()
                .and_then(|path| docs.footprint_editors.get(path))
            {
                return Surface::FootprintEditor(editor);
            }
        }
        if self.pcb_for_window(window_id).is_some() {
            Surface::Pcb
        } else {
            Surface::Schematic
        }
    }

    /// The zoom, in percent, of the canvas `window_id` is showing.
    pub(crate) fn status_zoom(&self, window_id: iced::window::Id) -> f64 {
        match self.status_surface(window_id) {
            Surface::SymbolEditor(editor) => editor.camera.zoom_percent(),
            Surface::FootprintEditor(editor) => editor.state.zoom_percent(),
            Surface::Pcb => self
                .interaction_state
                .pcb_canvas_for_window(window_id)
                .zoom_percent(),
            Surface::Schematic => self
                .interaction_state
                .canvas_for_window(window_id)
                .zoom_percent(),
        }
    }

    /// The cursor's world position in mm over the canvas `window_id` is
    /// showing. A library editor that has not seen the cursor reads the
    /// origin, as the app does before the first hover.
    pub(crate) fn status_cursor(&self, window_id: iced::window::Id) -> (f64, f64) {
        let own = match self.status_surface(window_id) {
            Surface::SymbolEditor(editor) => Some(editor.cursor_mm.unwrap_or_default()),
            Surface::FootprintEditor(editor) => Some(editor.state.cursor_mm.unwrap_or_default()),
            Surface::Pcb | Surface::Schematic => None,
        };
        own.unwrap_or((self.ui_state.cursor_x, self.ui_state.cursor_y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::TabKind;
    use std::path::PathBuf;

    const SCHEMATIC_ZOOM: f64 = 100.0;

    /// A main window whose only tab is the standalone editor `kind`
    /// builds; the schematic canvas camera sits at its default zoom and the
    /// schematic cursor at (7, 8).
    fn app_showing(kind: fn(PathBuf) -> TabKind) -> (Signex, iced::window::Id, PathBuf) {
        let (mut app, _task) = Signex::new();
        let window = iced::window::Id::unique();
        app.ui_state.main_window_id = Some(window);
        app.ui_state.cursor_x = 7.0;
        app.ui_state.cursor_y = 8.0;
        let path = PathBuf::from("readout.snxlib-file");
        app.document_state.tabs.push(crate::app::TabInfo {
            title: "e".into(),
            path: path.clone(),
            cached_document: None,
            project_id: None,
            kind: kind(path.clone()),
        });
        app.document_state.active_tab = app.document_state.tabs.len() - 1;
        (app, window, path)
    }

    #[test]
    fn a_symbol_editor_tab_reads_the_editors_zoom_and_cursor() {
        let (mut app, window, path) = app_showing(TabKind::SymbolEditor);
        let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("s"));
        let mut editor = SymbolDocument::new(path.clone(), file);
        editor.camera.scale *= 2.5;
        editor.cursor_mm = Some((1.5, -2.5));
        app.document_state.symbol_editors.insert(path, editor);

        assert_eq!(app.status_zoom(window), SCHEMATIC_ZOOM * 2.5);
        assert_eq!(app.status_cursor(window), (1.5, -2.5));
    }

    #[test]
    fn a_footprint_editor_tab_reads_the_editors_zoom_and_cursor() {
        let (mut app, window, path) = app_showing(TabKind::FootprintEditor);
        let file =
            signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("f"));
        let mut editor = FootprintDocument::new(path.clone(), file);
        editor.state.view_px_per_mm *= 3.0;
        editor.state.cursor_mm = Some((4.0, 5.0));
        app.document_state.footprint_editors.insert(path, editor);

        assert_eq!(app.status_zoom(window), SCHEMATIC_ZOOM * 3.0);
        assert_eq!(app.status_cursor(window), (4.0, 5.0));
    }

    #[test]
    fn a_footprint_canvas_cursor_event_carries_its_zoom_to_the_readout() {
        let (mut app, window, path) = app_showing(TabKind::FootprintEditor);
        let file =
            signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("f"));
        app.document_state
            .footprint_editors
            .insert(path.clone(), FootprintDocument::new(path.clone(), file));
        let _ = app.update(Message::Library(
            crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                path,
                msg: crate::library::messages::PrimitiveEdit::Footprint(
                    crate::library::messages::FootprintEditorMsg::CursorAt {
                        x_mm: 0.0,
                        y_mm: 0.0,
                        px_per_mm: 60.0,
                    },
                ),
            },
        ));

        assert_eq!(app.status_zoom(window), 200.0);
    }

    #[test]
    fn a_window_without_an_editor_keeps_reading_the_schematic() {
        let (app, window, _path) = app_showing(|_| TabKind::Schematic);
        assert_eq!(app.status_zoom(window), SCHEMATIC_ZOOM);
        assert_eq!(app.status_cursor(window), (7.0, 8.0));
    }
}
