//! Board sketch mode (#2234): enter and leave it, route the sketch editor's
//! messages to the working copy, and commit the copy to the board.
//!
//! The drawing is the footprint editor's sketch tool, hosted on the
//! session's own editor (`state/board_sketch.rs`). A commit bakes the copy
//! with `signex_bake::bake_board` and runs one `SetBoardSketch` board
//! command, so undo, dirty and save are a board edit's. A refused bake
//! leaves the board as it was and the mode on. See `docs/PCB_SKETCH.md`.

use std::path::Path;

use iced::Task;
use signex_engine::pcb::BoardCommand;
use signex_i18n::{ids, t};

use crate::app::state::{BoardSketchSession, ErrorNotice};
use crate::app::{BoardSketchMsg, FootprintDocument, Message, PcbMsg, Signex};
use crate::library::editor::footprint::sketch_redundancy_task::dispatch_redundancy;
use crate::library::editor::footprint::sketch_solve_task::dispatch_pending;
use crate::library::editor::footprint::state::{SketchTool, ToolPending};
use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

impl Signex {
    pub(crate) fn handle_board_sketch(&mut self, event: BoardSketchMsg) -> Task<Message> {
        match event {
            BoardSketchMsg::ModeToggled if self.board_sketch_in_view().is_some() => {
                self.leave_board_sketch();
            }
            BoardSketchMsg::ModeToggled => self.enter_board_sketch(),
            BoardSketchMsg::LeaveRequested => {
                self.leave_board_sketch();
            }
            BoardSketchMsg::ApplyRequested => {
                let changed = self
                    .interaction_state
                    .board_sketch
                    .as_ref()
                    .is_some_and(BoardSketchSession::has_changes);
                if changed {
                    self.commit_board_sketch();
                }
            }
            BoardSketchMsg::DiscardRequested => self.interaction_state.board_sketch = None,
        }
        self.refresh_panel_ctx();
        Task::none()
    }

    /// The sketch mode of the board the main window shows, if it is on.
    pub(crate) fn board_sketch_in_view(&self) -> Option<&BoardSketchSession> {
        let session = self.interaction_state.board_sketch.as_ref()?;
        let shown = self.active_tab_path()?;
        (shown == session.board_path && self.has_active_pcb()).then_some(session)
    }

    /// [`Self::board_sketch_in_view`], mutable.
    pub(crate) fn board_sketch_in_view_mut(&mut self) -> Option<&mut BoardSketchSession> {
        let shown = self.active_tab_path()?;
        if !self.has_active_pcb() {
            return None;
        }
        self.interaction_state
            .board_sketch
            .as_mut()
            .filter(|session| session.board_path == shown)
    }

    /// The sketch editor of the board in view: the Properties panel, the
    /// Active Bar and the footprint dock handlers act on it as on a
    /// footprint editor tab.
    pub(crate) fn board_sketch_editor(&self) -> Option<&FootprintDocument> {
        self.board_sketch_in_view().map(|session| &*session.editor)
    }

    /// [`Self::board_sketch_editor`], mutable.
    pub(crate) fn board_sketch_editor_mut(&mut self) -> Option<&mut FootprintDocument> {
        self.board_sketch_in_view_mut()
            .map(|session| &mut *session.editor)
    }

    /// The board at `path` closes, is parked, or is replaced by a copy read
    /// from disk: its sketch mode ends, so a later reopen starts fresh and no
    /// old copy can be committed. Uncommitted changes are dropped and
    /// Messages names the board (the close prompt is about saving the file,
    /// which never holds an uncommitted sketch).
    pub(crate) fn end_board_sketch_of(&mut self, path: &Path) {
        let Some(session) = self.interaction_state.board_sketch.as_ref() else {
            return;
        };
        if session.board_path != path {
            return;
        }
        if session.has_changes() {
            let board = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            crate::diagnostics::log_warning(
                t!(self.ui_state.i18n, ids::BOARD_SKETCH_DROPPED, "board" => board.as_str())
                    .into_string(),
            );
        }
        self.interaction_state.board_sketch = None;
    }

    /// Start sketch mode on the board in view. Another board's sketch mode
    /// is left first; if its commit is refused, this one does not start. A
    /// session whose board is no longer open (renamed by Save As) ends.
    fn enter_board_sketch(&mut self) {
        let Some(path) = self.active_tab_path() else {
            return;
        };
        if let Some(stale) = self
            .interaction_state
            .board_sketch
            .as_ref()
            .map(|session| session.board_path.clone())
            .filter(|other| self.document_state.pcb_board(other).is_none())
        {
            self.end_board_sketch_of(&stale);
        }
        let other = self
            .interaction_state
            .board_sketch
            .as_ref()
            .is_some_and(|session| session.board_path != path);
        if other && !self.leave_board_sketch() {
            return;
        }
        let Some(board) = self.active_pcb() else {
            return;
        };
        let session = BoardSketchSession::open(&path, board);
        self.interaction_state.board_sketch = Some(session);
        // The board's own selection is hidden in the mode: no key may act
        // on it unseen.
        self.clear_pcb_selection();
        crate::diagnostics::log_info(
            t!(self.ui_state.i18n, ids::BOARD_SKETCH_ENTERED).into_string(),
        );
    }

    /// Commit any change, then leave. Whether the mode is off afterwards.
    fn leave_board_sketch(&mut self) -> bool {
        let Some(session) = self.interaction_state.board_sketch.as_ref() else {
            return true;
        };
        if session.has_changes() && !self.commit_board_sketch() {
            return false;
        }
        self.interaction_state.board_sketch = None;
        true
    }

    /// Bake the working copy and run it as one board command. Whether the
    /// board took it; a refusal is told on the notice card and in Messages.
    fn commit_board_sketch(&mut self) -> bool {
        let Some(session) = self.interaction_state.board_sketch.as_ref() else {
            return false;
        };
        let path = session.board_path.clone();
        let sketch = session.working_sketch();
        let Some(board) = self.document_state.pcb_board(&path) else {
            let reason = crate::app::state::pcb_edit::BoardEditError::NotLoaded(path).to_string();
            self.refuse_board_sketch(reason);
            return false;
        };
        let baked = match signex_bake::bake_board(&sketch, &board.nets) {
            Ok(baked) => baked,
            Err(error) => {
                self.refuse_board_sketch(error.to_string());
                return false;
            }
        };
        let command = BoardCommand::set_board_sketch(sketch, baked);
        if let Err(error) = self.execute_board_commands(&path, vec![command], None) {
            self.refuse_board_sketch(error.to_string());
            return false;
        }
        // The stored copy carries the bake's ownership record; keep editing
        // from it so a later delete of the outline still clears the board's.
        let stored = self
            .document_state
            .pcb_board(&path)
            .and_then(|board| board.sketch.as_ref())
            .map(|stored| stored.sketch.clone())
            .unwrap_or_default();
        if let Some(session) = self.interaction_state.board_sketch.as_mut() {
            session.mark_committed(stored);
        }
        true
    }

    /// The board did not take the sketch: say why on the notice card, in
    /// Messages and in the mode's bar. The board and the copy are unchanged.
    fn refuse_board_sketch(&mut self, reason: String) {
        let i18n = &self.ui_state.i18n;
        let title = t!(i18n, ids::BOARD_SKETCH_TITLE).into_string();
        let detail = t!(i18n, ids::BOARD_SKETCH_REFUSED, "reason" => reason.as_str()).into_string();
        crate::diagnostics::log_warning(detail.clone());
        self.document_state.error_notice = Some(ErrorNotice::new(title, detail.clone()));
        if let Some(session) = self.interaction_state.board_sketch.as_mut() {
            session.refusal = Some(detail);
        }
    }

    /// Whether a sketch editor message for `path` is the board sketch's: it
    /// names the board, or it is the answer of the session's own solve.
    pub(crate) fn board_sketch_takes(&self, path: &Path, msg: &FootprintEditorMsg) -> bool {
        let Some(session) = self.interaction_state.board_sketch.as_ref() else {
            return false;
        };
        match msg {
            FootprintEditorMsg::SketchSolved { instance, .. }
            | FootprintEditorMsg::RedundancyChecked { instance, .. } => {
                session.editor.state.sketch_solve.instance() == *instance
            }
            _ => session.board_path == path,
        }
    }

    /// Apply one sketch editor message to the working copy, as the
    /// footprint editor applies it to a footprint tab.
    pub(crate) fn apply_board_sketch_edit(&mut self, msg: FootprintEditorMsg) -> Task<Message> {
        let Some(session) = self.interaction_state.board_sketch.as_mut() else {
            return Task::none();
        };
        let path = session.board_path.clone();
        apply_footprint_primitive_edit(&mut session.editor, msg);
        let solve = dispatch_pending(&mut session.editor, &path).map(Message::Library);
        let redundancy = dispatch_redundancy(&mut session.editor, &path).map(Message::Library);
        self.refresh_panel_ctx();
        Task::batch([solve, redundancy])
    }

    /// What Esc does on a board in sketch mode: cancel the tool, gesture or
    /// menu in flight first, then leave the mode.
    pub(crate) fn board_sketch_escape(&self) -> Option<Message> {
        let session = self.board_sketch_in_view()?;
        let state = &session.editor.state;
        let busy = state.active_tool != SketchTool::Select
            || !matches!(state.tool_pending, ToolPending::Idle)
            || state.context_menu.is_some()
            || state.active_bar_menu.is_some();
        Some(if busy {
            Message::Library(LibraryMessage::PrimitiveEditorEvent {
                path: session.board_path.clone(),
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ToolEscape),
            })
        } else {
            Message::Pcb(PcbMsg::Sketch(BoardSketchMsg::LeaveRequested))
        })
    }

    /// `(can_undo, can_redo)` of the working copy when the board at `path`
    /// is in sketch mode: Undo steps the sketch, not the board, until the
    /// mode is left.
    pub(crate) fn board_sketch_history(&self, path: &Path) -> Option<(bool, bool)> {
        let session = self.interaction_state.board_sketch.as_ref()?;
        let history = &session.editor.history;
        (session.board_path == path).then(|| (history.can_undo(), history.can_redo()))
    }

    /// Step the working copy's history when the board at `path` is in
    /// sketch mode; `None` when it is not.
    pub(crate) fn step_board_sketch_history(&mut self, path: &Path, undo: bool) -> Option<bool> {
        let session = self
            .interaction_state
            .board_sketch
            .as_mut()
            .filter(|session| session.board_path == path)?;
        let moved = if undo {
            session.editor.undo()
        } else {
            session.editor.redo()
        };
        if moved {
            session.editor.canvas_cache.clear();
            self.refresh_panel_ctx();
        }
        Some(moved)
    }
}
