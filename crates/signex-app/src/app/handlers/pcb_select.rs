//! PCB canvas hover and selection (#2191). Read-only: nothing here edits
//! the board. The index and the selection live in
//! `InteractionState::pcb_selection`.

use iced::Task;
use signex_types::pcb::PcbBoard;

use crate::app::{ContextMenuMsg, Message, Signex, TabDocument};
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
                self.close_context_menu_on_press()
            }
            PcbCanvasMsg::BoxSelected { from, to, mode } => {
                if self.with_pcb_selection(|sel, board| sel.box_select(board, from, to, mode))
                    == Some(true)
                {
                    self.refresh_pcb_highlight();
                }
                self.close_context_menu_on_press()
            }
        }
    }

    /// The canvas captures the left press, so the app-wide listener that
    /// closes an open context menu on it never sees one over the board.
    fn close_context_menu_on_press(&mut self) -> Task<Message> {
        self.update(Message::ContextMenu(ContextMenuMsg::Close))
    }

    /// Esc on a board: drop the selection, the hover and a box in progress.
    pub(crate) fn clear_pcb_selection(&mut self) {
        self.interaction_state.pcb_canvas.cancel_marquee();
        if self.interaction_state.pcb_selection.clear() {
            self.refresh_pcb_highlight();
        }
    }

    /// The board in view changed: repaint the highlight. The index itself
    /// is keyed on the board revision, so it does not depend on this call.
    pub(crate) fn note_pcb_board_changed(&mut self) {
        if self.active_pcb().is_none() {
            self.interaction_state.pcb_selection.sync(None, 0);
        }
        // Build the index now: a refused pad is reported to the Messages
        // panel once per board revision, not only when the pointer moves.
        self.with_pcb_selection(|sel, board| sel.build_index(board));
        self.refresh_pcb_highlight();
    }

    /// Run `f` on the selection state and the active board, after aligning
    /// the state with that board's document and revision. What the index
    /// build left out is reported to the Messages panel.
    fn with_pcb_selection<R>(
        &mut self,
        f: impl FnOnce(&mut PcbSelectionState, &PcbBoard) -> R,
    ) -> Option<R> {
        let docs = &self.document_state;
        let tab = docs.tabs.get(docs.active_tab)?;
        let board = tab.cached_document.as_ref().and_then(TabDocument::as_pcb)?;
        let revision = docs.pcb_revisions.get(&tab.path).copied().unwrap_or(0);
        let selection = &mut self.interaction_state.pcb_selection;
        selection.sync(Some(tab.path.as_path()), revision);
        let out = f(selection, board);
        for line in selection.take_report() {
            crate::diagnostics::log_warning(line);
        }
        Some(out)
    }

    /// Hand the canvas the copper to paint for the current selection.
    fn refresh_pcb_highlight(&mut self) {
        let highlight = self
            .with_pcb_selection(|sel, board| sel.highlight(board))
            .unwrap_or_default();
        self.interaction_state.pcb_canvas.set_highlight(highlight);
    }
}
