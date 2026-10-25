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
        self.interaction_state
            .active_canvas_mut()
            .clear_content_cache();
        let paths: Vec<_> = self.document_state.engines.keys().cloned().collect();
        for path in paths {
            self.report_missing_fonts(&path);
        }
    }
}
