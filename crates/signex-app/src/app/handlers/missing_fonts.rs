//! The "font is not installed" notices in the Messages panel (#510).
//!
//! The document keeps the name its author chose; these only tell the user
//! that the canvas and the exports draw the bundled default instead. Each
//! family is reported once per document (`DocumentState::missing_fonts`).

use std::path::Path;

use super::super::*;

impl Signex {
    /// Report the missing families of the schematic at `path`. A no-op until
    /// the installed-font scan has landed: with no catalog nothing can be
    /// called missing, and the scan's completion re-checks every document.
    pub(crate) fn report_missing_fonts(&mut self, path: &Path) {
        let Some(catalog) = crate::fonts::catalog::installed() else {
            return;
        };
        let Some(engine) = self.document_state.engines.get(path) else {
            return;
        };
        let messages =
            self.document_state
                .missing_fonts
                .new_messages(path, engine.document(), catalog);
        for message in messages {
            tracing::warn!(target: "signex::fonts", "{message}");
        }
    }

    pub(crate) fn report_missing_fonts_for_active_document(&mut self) {
        if let Some(path) = self.document_state.active_path.clone() {
            self.report_missing_fonts(&path);
        }
    }

    /// Forget what was reported for `path`, so opening it again reports again.
    pub(crate) fn forget_missing_font_reports(&mut self, path: &Path) {
        self.document_state.missing_fonts.forget(path);
    }

    /// The scan finished: text that names an installed family now draws
    /// with it, and every open document is checked for the ones that are not.
    pub(crate) fn handle_installed_fonts_scanned(&mut self) {
        // Every window's slot: an undocked window holds flags and hit boxes
        // sized for the bundled font until its cache is dropped.
        for slot in self.interaction_state.all_schematic_canvases_mut() {
            slot.clear_content_cache();
        }
        let paths: Vec<_> = self.document_state.engines.keys().cloned().collect();
        for path in paths {
            self.report_missing_fonts(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_scan_clears_every_window_canvas_cache() {
        let (mut app, _task) = Signex::new();
        let window = iced::window::Id::unique();
        app.interaction_state
            .canvases
            .insert(window, crate::canvas::CanvasSlot::new());
        let main = app.interaction_state.canvas.content_cache_clears;
        let undocked = app.interaction_state.canvases[&window].content_cache_clears;

        app.handle_installed_fonts_scanned();

        assert!(app.interaction_state.canvas.content_cache_clears > main);
        assert!(app.interaction_state.canvases[&window].content_cache_clears > undocked);
    }
}
