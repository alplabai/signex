//! Cross Probe and Cross Select Mode between the schematic and the board
//! (#1312). A part is matched by sheet occurrence and unit UUID against the
//! footprint's stored link, a net by name (`signex_net::cross_probe`).
//!
//! Cross select is passive: it follows a selection made on one side into
//! every canvas on screen on the other side, never opens a document and never
//! takes focus. Cross Probe is explicit: it opens or brings forward the other
//! side, selects and frames. Both run in one pass after each update
//! ([`Signex::sync_cross_select`]); design notes in `docs/DECISIONS.md`.

use iced::Task;
use signex_i18n::{MessageId, ids, t};

use crate::app::state::ErrorNotice;
use crate::app::{CrossProbeMsg, Message, Signex};

mod apply;
mod probe;
mod surfaces;

impl Signex {
    pub(crate) fn dispatch_cross_probe_message(&mut self, msg: CrossProbeMsg) -> Task<Message> {
        match msg {
            CrossProbeMsg::ProbeRequested => self.request_cross_probe(),
            CrossProbeMsg::CrossSelectModeToggled => {
                let enabled = !self.interaction_state.cross_select.enabled;
                self.interaction_state.cross_select.enabled = enabled;
                self.say(if enabled {
                    ids::CROSS_SELECT_ON
                } else {
                    ids::CROSS_SELECT_OFF
                });
            }
        }
        Task::none()
    }

    /// The pass after every update: follow a selection the user changed into
    /// the other side (Cross Select Mode), run a pending Cross Probe, then
    /// remember every selection so nothing written here is read back as the
    /// user's change. Skipped inside a window's event, whose canvases are
    /// swapped into the main slots; the outer update runs it.
    pub(crate) fn sync_cross_select(&mut self) -> Task<Message> {
        if self.interaction_state.acting_window.is_some() {
            return Task::none();
        }
        if self.interaction_state.cross_select.enabled {
            let now = self.selection_stamps();
            let changed = self.interaction_state.cross_select.changed(&now);
            if let Some(source) = changed {
                let source_empty = now
                    .iter()
                    .find(|(surface, _)| *surface == source)
                    .is_some_and(|(_, stamp)| stamp.is_empty);
                self.mirror_selection_from(source, source_empty);
            }
        }
        let task = self.run_pending_probe();
        let now = self.selection_stamps();
        self.interaction_state.cross_select.record(now);
        task
    }

    /// One line in the Messages panel, from the catalog.
    fn say(&self, id: MessageId) {
        crate::diagnostics::log_info(t!(self.ui_state.i18n, id).into_string());
    }

    /// An explicit Cross Probe found nothing to show: say why in Messages and
    /// on the notice card, so the command never ends in silence.
    fn refuse_probe(&mut self, id: MessageId) {
        let detail = t!(self.ui_state.i18n, id).into_string();
        let title = t!(self.ui_state.i18n, ids::CROSS_PROBE_TITLE).into_string();
        crate::diagnostics::log_info(detail.clone());
        self.document_state.error_notice = Some(ErrorNotice::new(title, detail));
    }
}
