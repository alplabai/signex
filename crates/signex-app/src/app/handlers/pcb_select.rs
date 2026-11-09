//! PCB canvas hover and selection (#2191). Read-only: nothing here edits
//! the board; a drag or Move is handed to `pcb_placement` (#2240). The
//! index and the selection live in `InteractionState::pcb_selection`.

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
                self.pcb_move_followed(at);
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
                if self.pcb_move_clicked(at, tolerance_mm) {
                    return self.close_context_menu_on_press();
                }
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
            PcbCanvasMsg::FootprintDragged { from, to } => {
                self.pcb_footprints_dragged(from, to);
                Task::none()
            }
            PcbCanvasMsg::FootprintDropped { from, to } => {
                self.pcb_footprints_dropped(from, to);
                Task::none()
            }
            PcbCanvasMsg::FootprintDragCancelled => {
                self.end_pcb_carry();
                Task::none()
            }
        }
    }

    /// The canvas captures the left press, so the app-wide listener that
    /// closes an open context menu on it never sees one over the board.
    fn close_context_menu_on_press(&mut self) -> Task<Message> {
        self.update(Message::ContextMenu(ContextMenuMsg::Close))
    }

    /// Esc on a board: drop the selection, the hover, a box in progress and
    /// a drag or Move in progress.
    pub(crate) fn clear_pcb_selection(&mut self) {
        self.interaction_state.pcb_canvas.cancel_marquee();
        let carried = self.interaction_state.pcb_carry.take().is_some();
        if self.interaction_state.pcb_selection.clear() || carried {
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
    pub(crate) fn with_pcb_selection<R>(
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

    /// Hand the canvas the copper to paint for the current selection, the
    /// preview of a drag or Move, and what a press grabs to drag (#2240).
    pub(crate) fn refresh_pcb_highlight(&mut self) {
        let carry = self.interaction_state.pcb_carry.clone();
        let painted = self.with_pcb_selection(|sel, board| {
            let mut highlight = sel.highlight(board);
            let selected = sel.selection().footprints();
            let grab = sel.footprint_shapes(board, &selected);
            if let Some(carry) = carry.filter(|c| c.anchor.is_some()) {
                let by = pcb_geom::PointNm::from_mm(carry.delta, "carry");
                highlight.ghost = match by {
                    Ok(by) => sel
                        .footprint_shapes(board, &carry.footprints)
                        .into_iter()
                        .map(|shape| shape.translated(by))
                        .collect(),
                    Err(_) => Vec::new(),
                };
            }
            (highlight, grab)
        });
        let (highlight, grab) = painted.unwrap_or_default();
        let canvas = &mut self.interaction_state.pcb_canvas;
        canvas.set_highlight(highlight);
        canvas.set_grab(grab);
    }
}
