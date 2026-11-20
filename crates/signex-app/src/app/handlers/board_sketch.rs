//! Board sketch mode (#2234): enter and leave it, route the sketch editor's
//! messages to the working copy, and commit the copy to the board.
//!
//! The drawing is the footprint editor's sketch tool, hosted on the
//! session's own editor (`state/board_sketch.rs`). A commit bakes the copy
//! with `signex_bake::bake_board` and runs one `SetBoardSketch` board
//! command, so undo, dirty and save are a board edit's. A refused bake
//! leaves the board as it was and the mode on. See `docs/PCB_SKETCH.md`.

use signex_i18n::RenderTranslatable;
use std::path::Path;

use iced::Task;
use signex_engine::pcb::BoardCommand;
use signex_i18n::{Translatable, ids, t};

use crate::app::state::{BoardSketchSession, ErrorNotice};
use crate::app::{BoardSketchMsg, FootprintDocument, Message, PcbMsg, Signex};
use crate::library::editor::footprint::sketch_redundancy_task::dispatch_redundancy;
use crate::library::editor::footprint::sketch_solve_task::dispatch_pending;
use crate::library::editor::footprint::state::SketchOverlay;
use crate::library::editor::footprint::{SketchTool, ToolPending, apply_footprint_primitive_edit};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

impl Signex {
    pub(crate) fn handle_board_sketch(&mut self, event: BoardSketchMsg) -> Task<Message> {
        match event {
            // The toggle while "Discard changes?" is open answers Keep
            // editing, as Esc does; it never applies the sketch.
            BoardSketchMsg::ModeToggled
                if self
                    .board_sketch_in_view()
                    .is_some_and(|s| s.editor.state.sketch_exit.confirming_discard) =>
            {
                return self.handle_board_sketch(BoardSketchMsg::DiscardCancelled);
            }
            BoardSketchMsg::ModeToggled if self.board_sketch_in_view().is_some() => {
                self.leave_board_sketch();
            }
            BoardSketchMsg::ModeToggled => self.enter_board_sketch(),
            BoardSketchMsg::BakeRequested => {
                self.leave_board_sketch();
            }
            BoardSketchMsg::BakePreviewToggled => {
                if let Some(session) = self.interaction_state.board_sketch.as_mut() {
                    let exit = &mut session.editor.state.sketch_exit;
                    exit.preview = !exit.preview;
                }
                self.refresh_board_sketch_preview();
            }
            BoardSketchMsg::DiscardRequested => self.request_board_sketch_discard(),
            BoardSketchMsg::DiscardConfirmed => self.interaction_state.board_sketch = None,
            BoardSketchMsg::DiscardCancelled => {
                if let Some(session) = self.interaction_state.board_sketch.as_mut() {
                    session.editor.state.sketch_exit.confirming_discard = false;
                }
            }
        }
        self.refresh_panel_ctx();
        Task::none()
    }

    /// Discard changes: leave at once when the board already has what the
    /// copy holds, else ask first (#2518). Nothing is written either way.
    fn request_board_sketch_discard(&mut self) {
        let Some(session) = self.interaction_state.board_sketch.as_mut() else {
            return;
        };
        if session.has_changes() {
            session.editor.state.sketch_exit.confirming_discard = true;
        } else {
            self.interaction_state.board_sketch = None;
        }
    }

    /// Re-bake the open bake preview of the board sketch against its
    /// board's nets; a closed preview drops its result.
    fn refresh_board_sketch_preview(&mut self) {
        let Some(session) = self.interaction_state.board_sketch.as_mut() else {
            return;
        };
        let nets = self
            .document_state
            .pcb_board(&session.board_path)
            .map(|board| board.nets.as_slice())
            .unwrap_or_default();
        session.refresh_preview(nets);
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
    /// Messages names the board. A close, save or quit the user asked for
    /// applies them first ([`Self::settle_board_sketch_of`]); what is left
    /// is a discard or a read the user chose.
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

    /// The board whose sketch has changes the board has not taken: unsaved
    /// work of that board for the quit prompt and the title count.
    pub(crate) fn board_sketch_owed(&self) -> Option<std::path::PathBuf> {
        let session = self.interaction_state.board_sketch.as_ref()?;
        session.has_changes().then(|| session.board_path.clone())
    }

    /// Apply the sketch's changes to its board before a save, close or quit
    /// reads the board at `path` (any board: `None`), as title-block drafts
    /// are committed. The mode stays on. `false`: the bake refused, the copy
    /// is kept, the notice says why and the caller holds.
    pub(crate) fn settle_board_sketch_of(&mut self, path: Option<&Path>) -> bool {
        let Some(board) = self
            .board_sketch_owed()
            .filter(|board| path.is_none_or(|p| p == board))
        else {
            return true;
        };
        // No board left to take it (cannot be reached through the UI): a
        // copy that can never commit must not hold every quit.
        if self.document_state.pcb_board(&board).is_none() {
            self.end_board_sketch_of(&board);
            return true;
        }
        if self.commit_board_sketch() {
            return true;
        }
        self.hold_for_board_sketch(&board);
        false
    }

    /// Leave the sketch mode of the board at `path`, applying its changes,
    /// before the board moves to its own window, where the mode is not
    /// drawn. `false`: the bake refused and the caller holds.
    pub(crate) fn leave_board_sketch_of(&mut self, path: &Path) -> bool {
        let on = self
            .interaction_state
            .board_sketch
            .as_ref()
            .is_some_and(|session| session.board_path == path);
        if !on || self.leave_board_sketch() {
            return true;
        }
        self.hold_for_board_sketch(path);
        false
    }

    /// The board's file moved from `from` to `to` (a rename): its sketch
    /// mode follows it, so a later save or quit still commits the copy.
    pub(crate) fn rekey_board_sketch(&mut self, from: &Path, to: &Path) {
        if let Some(session) = self
            .interaction_state
            .board_sketch
            .as_mut()
            .filter(|session| session.board_path == from)
        {
            session.board_path = to.to_path_buf();
            session.editor.path = to.to_path_buf();
        }
    }

    /// A save, close, quit or undock waits for the sketch: say so in
    /// Messages and on the notice card, with the bake's reason.
    fn hold_for_board_sketch(&mut self, board: &Path) {
        let name = board
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        // Messages stays en-US whatever the UI language.
        let held = Translatable::new(ids::BOARD_SKETCH_HELD).arg("board", name.as_str());
        crate::diagnostics::log_warning(held.render_en_us());
        let detail = self.board_sketch_held_detail(name);
        self.document_state.error_notice = Some(ErrorNotice::new(ids::BOARD_SKETCH_TITLE, detail));
    }

    /// Why a board named `name` holds: the hold and the bake's refusal, which
    /// nests as a message so the card renders it in the view.
    fn board_sketch_held_detail(&self, name: String) -> Translatable {
        let refusal = self
            .interaction_state
            .board_sketch
            .as_ref()
            .and_then(|session| session.refusal.clone());
        match refusal {
            Some(refusal) => Translatable::new(ids::NOTICE_BOARD_SKETCH_HELD_WHY)
                .arg("board", name)
                .arg("refusal", refusal),
            None => Translatable::new(ids::BOARD_SKETCH_HELD).arg("board", name),
        }
    }

    /// Why the board at `board` holds a close, in en-US: a caller's refusal.
    pub(crate) fn board_sketch_held_reason(&self, board: &Path) -> String {
        let name = board
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.board_sketch_held_detail(name).render_en_us()
    }

    /// Whether the board at `path` is shown in its own window.
    fn board_is_undocked(&self, path: &Path) -> bool {
        self.ui_state.windows.values().any(|kind| {
            matches!(kind, crate::app::state::WindowKind::UndockedTab { path: p, .. } if p == path)
        })
    }

    /// Start sketch mode on the board in view. Another board's sketch mode
    /// is left first; if its commit is refused, this one does not start. A
    /// session whose board is no longer open ends. A board in its own window
    /// does not start it: the mode is drawn in the main window only.
    fn enter_board_sketch(&mut self) {
        let Some(path) = self.active_tab_path() else {
            return;
        };
        if self.board_is_undocked(&path) {
            crate::diagnostics::log_warning(
                t!(self.ui_state.i18n, ids::BOARD_SKETCH_UNDOCKED).into_string(),
            );
            return;
        }
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
        let Some(board) = self.document_state.pcb_board(&path) else {
            return true;
        };
        let stored = board
            .sketch
            .as_ref()
            .map(|stored| stored.sketch.clone())
            .unwrap_or_default();
        if let Some(session) = self.interaction_state.board_sketch.as_mut() {
            session.mark_committed(stored, board);
        }
        true
    }

    /// The board did not take the sketch: say why on the notice card, in
    /// Messages and in the mode's bar. The board and the copy are unchanged.
    fn refuse_board_sketch(&mut self, reason: String) {
        let detail = Translatable::new(ids::BOARD_SKETCH_REFUSED).arg("reason", reason);
        crate::diagnostics::log_warning(detail.render_en_us());
        // The bar keeps the message, so it follows a language switch.
        if let Some(session) = self.interaction_state.board_sketch.as_mut() {
            session.refusal = Some(detail.clone());
        }
        self.document_state.error_notice = Some(ErrorNotice::new(ids::BOARD_SKETCH_TITLE, detail));
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
        self.refresh_board_sketch_preview();
        self.refresh_panel_ctx();
        Task::batch([solve, redundancy])
    }

    /// What Esc does on a board in sketch mode: answer an open discard
    /// question with Keep editing, cancel the tool, gesture, menu, canvas
    /// overlay or parameters table in flight, then Discard changes, which
    /// asks first when there are changes. Esc never applies the sketch to
    /// the board (#2518).
    pub(crate) fn board_sketch_escape(&self) -> Option<Message> {
        let session = self.board_sketch_in_view()?;
        let state = &session.editor.state;
        let sketch = |event| Some(Message::Pcb(PcbMsg::Sketch(event)));
        if state.sketch_exit.confirming_discard {
            return sketch(BoardSketchMsg::DiscardCancelled);
        }
        let busy = state.active_tool != SketchTool::Select
            || !matches!(state.tool_pending, ToolPending::Idle)
            || state.context_menu.is_some()
            || state.active_bar_menu.is_some()
            || state.parameters_table.is_some()
            || state.sketch_overlay != SketchOverlay::default();
        if busy {
            return Some(Message::Library(LibraryMessage::PrimitiveEditorEvent {
                path: session.board_path.clone(),
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ToolEscape),
            }));
        }
        sketch(BoardSketchMsg::DiscardRequested)
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
            self.refresh_board_sketch_preview();
            self.refresh_panel_ctx();
        }
        Some(moved)
    }
}

#[cfg(test)]
#[path = "board_sketch_tests.rs"]
mod tests;
