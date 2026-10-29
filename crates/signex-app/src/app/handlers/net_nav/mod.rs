//! Cross-sheet net navigation (#1270, #1271, #1272): jump to a port's
//! partners, step back and forward through the sheets visited, highlight a
//! net on every sheet, and the hover insight.
//!
//! Connectivity is the stitched project netlist's: a [`signex_net::NetIndex`]
//! built on a blocking thread and cached per project (`state::net_nav`). A
//! request made while the index is stale starts the build and is remembered
//! as pending; the answer re-runs it. Nothing here re-derives a net.

use iced::Task;

use super::super::*;
use crate::app::state::net_nav::PendingNetAction;

mod highlight;
mod hover;
mod index_job;
mod messages;
mod navigate;
mod target;

#[cfg(test)]
mod tests;

impl Signex {
    pub(crate) fn dispatch_net_nav_message(&mut self, msg: NetNavMsg) -> Task<Message> {
        let task = match msg {
            NetNavMsg::PartnerJumpRequested => self.jump_to_partner(),
            NetNavMsg::SheetBackRequested => self.step_sheet_history(false),
            NetNavMsg::SheetForwardRequested => self.step_sheet_history(true),
            NetNavMsg::NetHighlightRequested => self.highlight_net_under_cursor(),
            NetNavMsg::RelatedNetHighlightRequested => self.highlight_related_net_objects(),
            NetNavMsg::HighlightStepRequested { forward } => self.step_net_highlight(forward),
            NetNavMsg::HighlightClearRequested => self.clear_net_highlight(),
            // A timer tick: nothing to refresh until the dwell is answered.
            NetNavMsg::HoverDwellTicked => return self.hover_dwell_ticked(),
            NetNavMsg::IndexResolved(outcome) => self.apply_net_index(&outcome),
        };
        self.sync_diagnostics_panel_ctx();
        Task::batch([task, self.finish_update()])
    }

    /// Run again what was waiting for the index.
    fn run_pending_net_action(&mut self, action: PendingNetAction) -> Task<Message> {
        match action {
            PendingNetAction::JumpToPartner => self.jump_to_partner(),
            PendingNetAction::HighlightUnderCursor => self.highlight_net_under_cursor(),
            PendingNetAction::HighlightRelated => self.highlight_related_net_objects(),
            PendingNetAction::StepHighlight { forward } => self.step_net_highlight(forward),
            PendingNetAction::HoverDwell => self.hover_dwell_ticked(),
        }
    }
}
