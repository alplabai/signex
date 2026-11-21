//! The GUI half of the schematic edit tails (epic #2326, slice 2): core
//! events projected into the canvases, the panel context and the Messages
//! panel. The core half is `app::state::sheet_change`.
//!
//! Projection runs inside the wrapper's own call, never deferred: the
//! active canvas is the window `run_in_window_document` swapped in, and the
//! drag invalidation counts depend on it (`canvas/drag_invalidation_tests.rs`).

mod document_changed;
mod notice;
#[cfg(test)]
mod notice_tests;

pub(crate) use notice::log_notice;

use crate::app::Signex;
use crate::app::core::event::CoreEvent;

/// Which tail's refresh a wrapper asks for. A session hint of the GUI; it
/// never rides on an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RefreshScope {
    /// The gateway's: the active canvas, its page and overlay, the
    /// Properties selection fields. An empty patch only asks for the
    /// library drift check.
    ActiveCanvas {
        clear_overlay: bool,
        update_selection_info: bool,
    },
    /// The by-path tail's: every slot showing the sheet, and when it is the
    /// active one its selection and ERC markers.
    Sheet,
}

impl Signex {
    /// Project `events` in order; the panel context is re-derived once at the
    /// end when a document was repainted.
    pub(crate) fn project_events(&mut self, events: Vec<CoreEvent>, scope: RefreshScope) {
        let mut repainted = false;
        for event in events {
            match event {
                CoreEvent::Notice(notice) => log_notice(&notice),
                CoreEvent::DocumentChanged(changed) => {
                    repainted |= self.project_document_changed(&changed, scope);
                }
            }
        }
        // The document's dirty mark has just been set (or stayed set), and
        // `panel_ctx.projects[*].sheets[*].is_dirty` drives the red dot on
        // the project-tree row.
        if repainted {
            self.refresh_panel_ctx();
        }
    }
}
