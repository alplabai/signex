//! The Library field beside a pad's own template name: the pad form's way to
//! name the library of a template no other pad uses.

use super::super::*;

impl Signex {
    pub(crate) fn fp_editor_set_next_pad_template_library(&mut self, value: String) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.template_library = value;
        }
        self.refresh_panel_ctx();
        true
    }

    pub(crate) fn fp_editor_set_selected_pad_template_library(
        &mut self,
        idx: usize,
        value: String,
    ) -> bool {
        self.with_selected_pad_field(idx, Some("pad_template_library"), |pad| {
            pad.template_library = value
        });
        true
    }
}
