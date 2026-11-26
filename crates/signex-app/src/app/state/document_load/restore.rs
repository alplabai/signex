//! A recovery copy restored over the open document.

use std::path::Path;

use crate::app::state::DocumentState;
use crate::autosave::RestoredDocument;

impl DocumentState {
    /// A recovery copy brings back content, not identity: the restored
    /// document keeps the id the open one holds (#2172), so the copy's own
    /// id (the file's, as autosave last saw it) never reaches a second
    /// document or goes unreported. Reads the open document, so it runs
    /// before the copy is installed.
    pub(crate) fn keep_live_document_id(&self, path: &Path, restored: &mut RestoredDocument) {
        match restored {
            RestoredDocument::Schematic(sheet) => {
                if let Some(engine) = self.engines.get(path) {
                    sheet.uuid = engine.document().uuid;
                }
            }
            RestoredDocument::Symbol(file) => {
                if let Some(editor) = self.symbol_editors.get(path) {
                    file.file_uuid = editor.draft.file.file_uuid;
                }
            }
            RestoredDocument::Footprint(file) => {
                if let Some(editor) = self.footprint_editors.get(path) {
                    file.file_uuid = editor.file.file_uuid;
                }
            }
            RestoredDocument::Pcb(board) => {
                if let Some(live) = self.pcb_board(path) {
                    board.uuid = live.uuid;
                }
            }
            RestoredDocument::Project(data) => {
                if let Some(project) = self.projects.iter().find(|p| p.path == path) {
                    data.document_id = project.data.document_id;
                }
            }
            RestoredDocument::Row(_) => {}
        }
    }
}
