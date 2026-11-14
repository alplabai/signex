//! Align on a board (#2373): the Align rows and the PCB Active Bar's Align
//! dropdown run the same commands as on a sheet, and on a board they line
//! the selected footprints up by their origins as one undo step, built by
//! `signex_engine::pcb::align`. A selected pad stands for its footprint; a
//! locked footprint refuses the whole run, saying why.

use signex_engine::pcb::align::{AlignTo, align_commands};

use crate::active_bar::ActiveBarAction;
use crate::app::Signex;

/// The board alignment an Align action asks for.
fn align_to(action: &ActiveBarAction) -> Option<AlignTo> {
    Some(match action {
        ActiveBarAction::AlignLeft => AlignTo::Left,
        ActiveBarAction::AlignRight => AlignTo::Right,
        ActiveBarAction::AlignTop => AlignTo::Top,
        ActiveBarAction::AlignBottom => AlignTo::Bottom,
        ActiveBarAction::AlignHorizontalCenters => AlignTo::HorizontalCenters,
        ActiveBarAction::AlignVerticalCenters => AlignTo::VerticalCenters,
        ActiveBarAction::DistributeHorizontally => AlignTo::DistributeHorizontally,
        ActiveBarAction::DistributeVertically => AlignTo::DistributeVertically,
        ActiveBarAction::AlignToGrid => AlignTo::Grid,
        _ => return None,
    })
}

impl Signex {
    /// Align the board selection when the active tab is a PCB. Whether it
    /// was one (the sheet path is then skipped).
    pub(crate) fn align_pcb_selection(&mut self, action: &ActiveBarAction) -> bool {
        if !self.has_active_pcb() {
            return false;
        }
        let Some(to) = align_to(action) else {
            return true;
        };
        let footprints = self.pcb_selected_footprints();
        let Some(board) = self.active_pcb() else {
            return true;
        };
        let built = align_commands(board, &footprints, to);
        self.run_placement("Align", built);
        true
    }
}

#[cfg(test)]
#[path = "pcb_align_tests.rs"]
mod tests;
