//! Page-setup handlers for the PDF export modal: size, orientation, margins
//! and scale. Validation lives in `PreviewState::apply_*`; each handler here
//! only forwards the edit and re-rasterises when it changed the options.

use signex_output::{MarginEdge, Orientation, PageSize, PdfScaleMode};

use super::super::super::super::*;

impl Signex {
    /// Run `edit` on the open preview; re-rasterise when it changed the options.
    fn edit_print_preview_page_setup(
        &mut self,
        edit: impl FnOnce(&mut crate::app::state::PreviewState) -> bool,
    ) {
        let changed = self.document_state.preview.as_mut().is_some_and(edit);
        if changed {
            self.rerasterize_print_preview();
        }
    }

    pub(crate) fn handle_print_preview_set_page_size(&mut self, size: PageSize) {
        self.edit_print_preview_page_setup(|p| p.apply_page_size(size));
    }

    pub(crate) fn handle_print_preview_set_orientation(&mut self, orientation: Orientation) {
        self.edit_print_preview_page_setup(|p| p.apply_orientation(orientation));
    }

    pub(crate) fn handle_print_preview_set_custom_dimension(
        &mut self,
        is_width: bool,
        text: String,
    ) {
        self.edit_print_preview_page_setup(|p| p.apply_custom_dimension(is_width, text));
    }

    pub(crate) fn handle_print_preview_set_margin(&mut self, edge: MarginEdge, text: String) {
        self.edit_print_preview_page_setup(|p| p.apply_margin(edge, text));
    }

    pub(crate) fn handle_print_preview_set_scale_mode(&mut self, mode: PdfScaleMode) {
        self.edit_print_preview_page_setup(|p| p.apply_scale_mode(mode));
    }

    pub(crate) fn handle_print_preview_set_scale_percent(&mut self, text: String) {
        self.edit_print_preview_page_setup(|p| p.apply_scale_percent(text));
    }
}
