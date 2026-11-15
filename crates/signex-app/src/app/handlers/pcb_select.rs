//! PCB canvas hover and selection (#2191). Read-only: nothing here edits
//! the board; a drag or Move is handed to `pcb_placement` (#2240). The
//! index and the selection live in `InteractionState::pcb_selection`.

use iced::Task;
use signex_i18n::{ids, t};
use signex_renderer::pcb::{PcbLayerView, PcbSnapshot};
use signex_types::pcb::PcbBoard;

use crate::app::state::PcbRatsnest;
use crate::app::{ContextMenuMsg, Message, Signex, TabDocument};
use crate::canvas::CanvasEvent;
use crate::pcb_canvas::{
    PcbCanvas, PcbCanvasMsg, PcbCarry, PcbSelectionState, board_without, carried_board,
};

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
                self.pcb_paste_followed(at);
                self.pcb_place_followed(at);
                self.board_tool_followed(at);
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
                if self.board_tool_clicked(at)
                    || self.pcb_place_clicked(at)
                    || self.pcb_paste_clicked(at)
                    || self.pcb_move_clicked(at, tolerance_mm)
                    || self.pcb_net_pick_clicked(at, tolerance_mm)
                {
                    return self.close_context_menu_on_press();
                }
                if self.with_pcb_selection(|sel, board| sel.click(board, at, tolerance_mm, mode))
                    == Some(true)
                {
                    self.refresh_pcb_highlight();
                }
                self.close_context_menu_on_press()
            }
            PcbCanvasMsg::BoxSelected {
                from,
                to,
                mode,
                view_rotation_deg,
            } => {
                let changed = self.with_pcb_selection(|sel, board| {
                    sel.box_select(board, from, to, mode, view_rotation_deg)
                });
                if changed == Some(true) {
                    self.refresh_pcb_highlight();
                }
                self.close_context_menu_on_press()
            }
            PcbCanvasMsg::FootprintDragged {
                from,
                to,
                tolerance_mm,
            } => {
                self.pcb_footprints_dragged(from, to, tolerance_mm);
                Task::none()
            }
            PcbCanvasMsg::FootprintDropped { from, to } => {
                self.pcb_footprints_dropped(from, to);
                Task::none()
            }
            PcbCanvasMsg::FootprintDragCancelled => {
                // The press that discarded the drag also closes a menu.
                self.end_pcb_carry();
                self.close_context_menu_on_press()
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
        // A placement tool steps back first: its shape, then the tool.
        if self.step_back_board_tool() {
            return;
        }
        self.interaction_state.pcb_canvas.cancel_marquee();
        // Esc also ends a Properties net pick (#2355).
        self.interaction_state.pcb_primitive.net_pick = None;
        let carried = self.has_own_pcb_carry();
        if carried {
            self.interaction_state.pcb_carry = None;
        }
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
        selection.set_view(
            self.ui_state
                .pcb_layer_views
                .get(&tab.path)
                .unwrap_or(&Default::default()),
        );
        selection.set_filter(&self.ui_state.pcb_pick_filter);
        let out = f(selection, board);
        for line in selection.take_report() {
            crate::diagnostics::log_warning(line);
        }
        Some(out)
    }

    /// Select exactly `items` on the board in view (a DRC violation's
    /// items, #2222); those on a hidden layer are left out and counted in
    /// the Messages panel.
    pub(crate) fn select_pcb_items(&mut self, items: &[pcb_geom::ItemRef]) {
        let Some(done) = self.with_pcb_selection(|sel, board| sel.select_items(board, items))
        else {
            return;
        };
        if done.hidden > 0 {
            crate::diagnostics::log_warning(
                t!(self.ui_state.i18n, ids::DRC_PICK_HIDDEN, "count" => done.hidden).into_string(),
            );
        }
        if done.changed {
            self.refresh_pcb_highlight();
        }
    }

    /// Hand the canvas the copper to paint for the current selection, the
    /// preview of a drag or Move, and what a press grabs to drag (#2240).
    pub(crate) fn refresh_pcb_highlight(&mut self) {
        let carry = self
            .interaction_state
            .pcb_carry
            .clone()
            .filter(|_| self.has_own_pcb_carry());
        let painted = self.with_pcb_selection(|sel, board| sel.paint(board, carry.as_ref()));
        let (highlight, grab) = painted.unwrap_or_default();
        let ghost = self
            .pcb_part_ghost(carry.as_ref())
            .or_else(|| self.board_tool_ghost());
        // A tool waits for clicks: a press never starts a drag.
        let tool_armed = self.own_board_tool().is_some();
        let path = self.active_tab_path().unwrap_or_default();
        let view = self.pcb_layer_view(&path);
        let docs = &self.document_state;
        let board = docs
            .tabs
            .get(docs.active_tab)
            .and_then(|tab| tab.cached_document.as_ref())
            .and_then(TabDocument::as_pcb);
        let interaction = &mut self.interaction_state;
        let canvas = &mut interaction.pcb_canvas;
        canvas.set_highlight(highlight);
        canvas.set_grab(if tool_armed { Vec::new() } else { grab });
        canvas.set_part_ghost(ghost.as_ref());
        let ratsnest = view
            .ratsnest_visible()
            .then_some((&interaction.pcb_ratsnest, path.as_path()));
        match board {
            Some(board) => sync_drag_preview(canvas, board, &view, ratsnest, carry.as_ref()),
            None => canvas.end_drag_preview(),
        }
        // The Properties panel switches between the board and the selection.
        self.sync_pcb_board_summary();
    }

    /// Repaint the selection of every undocked window that shows the board
    /// at `path` over its current geometry: highlight, Move preview and
    /// grab. The window acting now has its state in the main slots and is
    /// painted by [`Self::refresh_pcb_highlight`].
    pub(crate) fn refresh_undocked_pcb_highlights(&mut self, path: &std::path::Path) {
        use crate::app::state::WindowKind;
        let windows: Vec<iced::window::Id> = self
            .interaction_state
            .pcb_selections
            .keys()
            .copied()
            .filter(|id| {
                matches!(
                    self.ui_state.windows.get(id),
                    Some(WindowKind::UndockedTab { path: shown, .. }) if shown == path
                )
            })
            .collect();
        let Some(board) = self.document_state.pcb_board(path) else {
            return;
        };
        let revision = self
            .document_state
            .pcb_revisions
            .get(path)
            .copied()
            .unwrap_or(0);
        let view = self.pcb_layer_view(path);
        let ghost = self
            .interaction_state
            .pcb_carry
            .as_ref()
            .filter(|c| c.path.as_path() == path && c.window.is_some_and(|w| windows.contains(&w)))
            .and_then(|c| Some((c.window?, self.pcb_part_ghost(Some(c))?)));
        let interaction = &mut self.interaction_state;
        for id in windows {
            let carry = interaction
                .pcb_carry
                .as_ref()
                .filter(|c| c.is_for(Some(path), Some(id)));
            let Some(selection) = interaction.pcb_selections.get_mut(&id) else {
                continue;
            };
            selection.sync(Some(path), revision);
            selection.set_view(&view);
            let (highlight, grab) = selection.paint(board, carry);
            if let Some(canvas) = interaction.pcb_canvases.get_mut(&id) {
                canvas.set_highlight(highlight);
                canvas.set_grab(grab);
                let own = ghost.as_ref().filter(|(owner, _)| *owner == id);
                canvas.set_part_ghost(own.map(|(_, snapshot)| snapshot));
                let ratsnest = view
                    .ratsnest_visible()
                    .then_some((&interaction.pcb_ratsnest, path));
                sync_drag_preview(canvas, board, &view, ratsnest, carry);
            }
        }
    }
}

/// Bring `canvas`'s drag preview in line with `carry` on `board` (#2358):
/// while the carry is anchored, the board is drawn without the carried
/// footprints and they are drawn at its offset; otherwise the whole board.
/// `ratsnest` is the cache and the board path when the view shows it.
fn sync_drag_preview(
    canvas: &mut PcbCanvas,
    board: &PcbBoard,
    view: &PcbLayerView,
    ratsnest: Option<(&PcbRatsnest, &std::path::Path)>,
    carry: Option<&PcbCarry>,
) {
    let live = carry.filter(|c| c.anchor.is_some() && !c.footprints.is_empty());
    let Some(carry) = live else {
        canvas.end_drag_preview();
        return;
    };
    if !canvas.previews_drag_of(&carry.footprints) {
        // Once per carry: the board without what it carries.
        let rest = board_without(board, &carry.footprints);
        let base = PcbSnapshot::from_board_with_view(&rest, view);
        let base = match ratsnest {
            Some((cache, path)) => cache.apply_uncached(path, &rest, base),
            None => base,
        };
        canvas.begin_drag_preview(carry.footprints.clone(), base);
    }
    canvas.move_drag_preview(carry.delta, || {
        let moved = carried_board(board, &carry.footprints, carry.delta);
        PcbSnapshot::from_board_with_view(&moved, view)
    });
}
