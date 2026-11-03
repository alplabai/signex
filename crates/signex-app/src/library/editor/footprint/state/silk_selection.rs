//! The top-silk selection across a re-bake, and which silk entries Pads
//! mode may edit (#1718). See `docs/FOOTPRINT_SKETCH.md`, "Bake ownership".

use signex_bake::SilkRemaps;
use signex_library::primitive::footprint::{Footprint, FpGraphic};

use super::FootprintEditorState;
use super::context_menu::FootprintContextTarget;

impl FootprintEditorState {
    /// Follow the selected top-silk entry, and a silk context-menu target,
    /// to where a bake merge moved it; drop it when the merge removed it.
    pub fn remap_silk_selection(&mut self, remaps: &SilkRemaps) {
        let follow = |idx: usize| remaps.silk_f.get(idx).copied().flatten();
        self.selected_silk_f = self.selected_silk_f.and_then(follow);
        let menu_target = self.context_menu.as_ref().map(|menu| menu.target);
        if let Some(FootprintContextTarget::SilkF(idx)) = menu_target {
            match (follow(idx), self.context_menu.as_mut()) {
                (Some(moved), Some(menu)) => menu.target = FootprintContextTarget::SilkF(moved),
                _ => self.context_menu = None,
            }
        }
    }

    /// Drop the top-silk selection and a silk context-menu target.
    pub fn clear_silk_selection(&mut self) {
        self.selected_silk_f = None;
        if let Some(FootprintContextTarget::SilkF(_)) =
            self.context_menu.as_ref().map(|menu| menu.target)
        {
            self.context_menu = None;
        }
    }
}

/// Whether `silk_f[idx]` was baked from the sketch. Pads mode shows such
/// an entry but never edits or deletes it: the next bake would undo the
/// edit or bring the entry back.
pub fn is_sketch_owned_silk(fp: &Footprint, idx: usize) -> bool {
    fp.silk_f.get(idx).is_some_and(FpGraphic::is_baked)
}

/// Tell the user why a Pads-mode silk edit or delete did nothing.
pub fn report_sketch_owned_silk(idx: usize) {
    crate::diagnostics::log_warning(format!(
        "Silk graphic {idx} is baked from the sketch; edit it in Sketch mode"
    ));
}
