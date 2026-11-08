//! PCB canvas hover and selection (#2191). Read-only: nothing here edits
//! the board. The index and the selection live in
//! `InteractionState::pcb_selection`.

use iced::Task;
use signex_types::pcb::PcbBoard;

use crate::app::{Message, Signex, TabDocument};
use crate::canvas::CanvasEvent;
use crate::pcb_canvas::{PcbCanvasMsg, PcbSelectionState};

impl Signex {
    pub(crate) fn dispatch_pcb_canvas_message(&mut self, msg: PcbCanvasMsg) -> Task<Message> {
        match msg {
            PcbCanvasMsg::PointerMoved {
                at,
                zoom_pct,
                tolerance_mm,
            } => {
                if self.with_pcb_selection(|sel, board| sel.hover_at(board, at, tolerance_mm))
                    == Some(true)
                {
                    self.refresh_pcb_highlight();
                }
                // The status readout follows the pointer as before.
                self.update(Message::CanvasEvent(CanvasEvent::CursorAt {
                    x: at.x as f32,
                    y: at.y as f32,
                    zoom_pct,
                }))
            }
            PcbCanvasMsg::Clicked {
                at,
                tolerance_mm,
                mode,
            } => {
                if self.with_pcb_selection(|sel, board| sel.click(board, at, tolerance_mm, mode))
                    == Some(true)
                {
                    self.refresh_pcb_highlight();
                }
                Task::none()
            }
            PcbCanvasMsg::BoxSelected { from, to, mode } => {
                if self.with_pcb_selection(|sel, board| sel.box_select(board, from, to, mode))
                    == Some(true)
                {
                    self.refresh_pcb_highlight();
                }
                Task::none()
            }
        }
    }

    /// Esc on a board: drop the selection and the hover.
    pub(crate) fn clear_pcb_selection(&mut self) {
        if self.interaction_state.pcb_selection.clear() {
            self.refresh_pcb_highlight();
        }
    }

    /// The board in view changed: forget the index, repaint the highlight.
    pub(crate) fn note_pcb_board_changed(&mut self) {
        let document = self
            .active_pcb()
            .and_then(|_| self.document_state.tabs.get(self.document_state.active_tab))
            .map(|tab| tab.path.clone());
        self.interaction_state
            .pcb_selection
            .board_changed(document.as_deref());
        self.refresh_pcb_highlight();
    }

    fn with_pcb_selection<R>(
        &mut self,
        f: impl FnOnce(&mut PcbSelectionState, &PcbBoard) -> R,
    ) -> Option<R> {
        let board = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|tab| tab.cached_document.as_ref())
            .and_then(TabDocument::as_pcb)?;
        Some(f(&mut self.interaction_state.pcb_selection, board))
    }

    /// Hand the canvas the copper to paint for the current selection.
    fn refresh_pcb_highlight(&mut self) {
        let highlight = self
            .with_pcb_selection(|sel, board| sel.highlight(board))
            .unwrap_or_default();
        self.interaction_state.pcb_canvas.set_highlight(highlight);
    }
}
