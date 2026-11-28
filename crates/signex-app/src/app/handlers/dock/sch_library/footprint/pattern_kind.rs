//! The Pattern form's kind segment (#2596): Linear and Grid switch the
//! pattern in one undo step, Polar asks for its centre first. The slice's
//! `library::editor::footprint::pattern_kind_picked` does the work.

use signex_bake::array::PatternKind;
use signex_sketch::array::ArrayId;

use super::super::*;

impl Signex {
    pub(crate) fn fp_editor_pattern_kind_picked(
        &mut self,
        array_id: ArrayId,
        kind: PatternKind,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            crate::library::editor::footprint::pattern_kind_picked(editor, array_id, kind);
        }
        self.refresh_panel_ctx();
        true
    }
}

#[cfg(test)]
#[path = "pattern_kind_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "pattern_kind_canvas_tests.rs"]
mod canvas_tests;
