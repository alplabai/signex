//! PCB trace calculator edits: apply the edit, and put any failure the
//! edit produced on the Messages panel as well as in the dialog.

use iced::Task;
use signex_widgets::trace_calculator::TraceCalculatorMessage;

use super::super::*;

impl Signex {
    pub(super) fn handle_pcb_trace_calculator_edited(
        &mut self,
        message: TraceCalculatorMessage,
    ) -> Task<Message> {
        if let Some(failure) = self.ui_state.pcb_trace_calculator.update(message) {
            crate::diagnostics::log_warning(format!("PCB trace calculator: {failure}"));
            // This dispatcher does not run `finish_update`, so republish
            // the log or the record waits for some unrelated message.
            self.sync_diagnostics_panel_ctx();
        }
        Task::none()
    }
}
