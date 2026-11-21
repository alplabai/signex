//! A changed document, projected into the canvases that show it.

use std::path::Path;

use signex_engine::DocumentPatch;

use super::RefreshScope;
use crate::app::Signex;
use crate::app::core::event::{ChangeCause, DocumentChanged};

impl Signex {
    /// Whether the document was repainted (an empty patch on the active
    /// canvas is not).
    pub(super) fn project_document_changed(
        &mut self,
        changed: &DocumentChanged,
        scope: RefreshScope,
    ) -> bool {
        // A step can bring back an older embedded library symbol.
        if changed.cause == ChangeCause::Step {
            self.library.drift.check_pending = true;
        }
        match scope {
            RefreshScope::ActiveCanvas {
                clear_overlay,
                update_selection_info,
            } => {
                if changed.patch == DocumentPatch::NONE {
                    return false;
                }
                self.repaint_active_canvas(changed.patch, clear_overlay, update_selection_info);
            }
            RefreshScope::Sheet => self.repaint_sheet(&changed.path, changed.patch),
        }
        true
    }

    fn repaint_active_canvas(
        &mut self,
        patch: DocumentPatch,
        clear_overlay: bool,
        update_selection_info: bool,
    ) {
        self.sync_canvas_from_visible_schematic(patch.into());
        // The sheet's paper changed (a Page Options edit, or its undo or
        // redo): the panel context and the drawn page follow the document.
        if patch.intersects(DocumentPatch::PAPER)
            && let Some(token) = self
                .active_render_snapshot()
                .map(|snapshot| crate::app::projector::effective_paper_size(&snapshot.paper_size))
        {
            self.document_state.panel_ctx.paper_size = token;
            self.apply_page_dimensions_to_canvas();
        }
        self.interaction_state
            .active_canvas_mut()
            .clear_content_cache();
        if clear_overlay {
            self.interaction_state
                .active_canvas_mut()
                .clear_overlay_cache();
        }
        self.prune_selected_wires();
        if !patch.is_display_only() {
            self.refresh_erc_after_edit();
        }
        if update_selection_info {
            self.update_selection_info();
        }
    }

    /// The windows and tile pane showing `path`; when it is the main
    /// window's active sheet the same refresh keeps that canvas current.
    fn repaint_sheet(&mut self, path: &Path, patch: DocumentPatch) {
        let is_active = self.document_state.active_path.as_deref() == Some(path);
        self.refresh_slots_from_engine(path, is_active, patch.into(), false);
        if is_active {
            self.prune_selected_wires();
            if !patch.is_display_only() {
                self.refresh_erc_after_edit();
            }
        }
    }
}
