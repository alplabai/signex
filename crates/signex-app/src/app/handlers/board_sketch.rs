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
use crate::library::editor::footprint::SketchOverlay;
use crate::library::editor::footprint::sketch_redundancy_task::dispatch_redundancy;
use crate::library::editor::footprint::sketch_solve_task::dispatch_pending;
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
    /// board's nets and components; a closed preview drops its result.
    pub(super) fn refresh_board_sketch_preview(&mut self) {
        self.sync_board_sketch_fab();
        let Some(session) = self.interaction_state.board_sketch.as_mut() else {
            return;
        };
        let board = self.document_state.pcb_board(&session.board_path);
        session.refresh_preview(board);
    }

    /// The board target rows of the selected entity of the board sketch in
    /// view (decision 2); none without one.
    pub(crate) fn board_sketch_target(&self) -> crate::panels::BoardTargetProps {
        self.board_sketch_in_view()
            .and_then(|session| {
                let board = self.document_state.pcb_board(&session.board_path)?;
                Some(session.target_props(board))
            })
            .unwrap_or_default()
    }

    /// Copy the router tool diameter and the min slot of the board's project
    /// into the sketch session, for its inside-corner and min slot checks
    /// (mockup row 179d): on entry, on every sketch edit and when a fab tier
    /// or profile sets them. Memory only.
    pub(crate) fn sync_board_sketch_fab(&mut self) {
        let Some(path) = self
            .interaction_state
            .board_sketch
            .as_ref()
            .map(|session| session.board_path.clone())
        else {
            return;
        };
        let ds = &self.document_state;
        let project = ds
            .document_project(&path)
            .or_else(|| ds.project_for_path(&path));
        let diameter = project.and_then(|project| project.data.router_tool_diameter());
        let min_slot = project
            .and_then(|project| project.data.rules.as_ref())
            .and_then(|rules| rules.min_slot());
        if let Some(session) = self.interaction_state.board_sketch.as_mut() {
            let exit = &mut session.editor.state.sketch_exit;
            exit.router_tool_diameter = diameter;
            exit.min_slot = min_slot;
        }
    }

    /// Bring the sketch's origin marker and readouts in line with its board:
    /// on entry and whenever the board or its view changes.
    pub(crate) fn sync_board_sketch_origin(&mut self) {
        let Some(path) = self
            .interaction_state
            .board_sketch
            .as_ref()
            .map(|session| session.board_path.clone())
        else {
            return;
        };
        let Some(origin) = self
            .document_state
            .pcb_board(&path)
            .map(|board| board.options.origin)
        else {
            return;
        };
        let hidden = !self.pcb_layer_view(&path).origin_marker_visible();
        if let Some(session) = self.interaction_state.board_sketch.as_mut() {
            session.follow_board_origin(origin, hidden);
        }
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

    /// [`Self::board_sketch_editor`], to edit: `None` while the Import DXF
    /// card holds the sketch.
    pub(crate) fn board_sketch_editor_mut(&mut self) -> Option<&mut FootprintDocument> {
        self.board_sketch_in_view_mut()
            .filter(|session| session.dxf_dialog.is_none())
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
        self.sync_board_sketch_fab();
        self.sync_board_sketch_origin();
        // The board's own selection is hidden in the mode: no key may act
        // on it unseen.
        self.clear_pcb_selection();
        // The hint is said once a session: Messages never repeats it
        // (mockup row 187).
        if !self.interaction_state.board_sketch_hint_said {
            self.interaction_state.board_sketch_hint_said = true;
            crate::diagnostics::log_info(
                t!(self.ui_state.i18n, ids::BOARD_SKETCH_ENTERED).into_string(),
            );
        }
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
        let (command, locked) = match board_sketch_command(sketch, board) {
            Ok(made) => made,
            Err(error) => {
                self.refuse_board_sketch(error.to_string());
                return false;
            }
        };
        // Messages stays en-US whatever the UI language.
        for reference in locked {
            let locked = Translatable::new(ids::BOARD_SKETCH_ANCHOR_LOCKED)
                .arg("reference", reference.as_str());
            crate::diagnostics::log_warning(locked.render_en_us());
        }
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
        // The Import DXF card holds the sketch; only the answer of a solve
        // already running still lands, or that solve would never finish.
        let held = self
            .interaction_state
            .board_sketch
            .as_ref()
            .is_some_and(|session| session.dxf_dialog.is_some());
        let answer = matches!(
            msg,
            FootprintEditorMsg::SketchSolved { .. } | FootprintEditorMsg::RedundancyChecked { .. }
        );
        if held && !answer {
            return Task::none();
        }
        if matches!(msg, FootprintEditorMsg::SketchImportDxfRequested) {
            return self.request_sketch_dxf_file();
        }
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
        if session.dxf_dialog.is_some() {
            return Some(super::pcb_dxf_sketch::dialog_cancelled());
        }
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
    /// mode is left. Neither while the Import DXF card holds the sketch.
    pub(crate) fn board_sketch_history(&self, path: &Path) -> Option<(bool, bool)> {
        let session = self.interaction_state.board_sketch.as_ref()?;
        let history = &session.editor.history;
        let free = session.dxf_dialog.is_none();
        (session.board_path == path)
            .then(|| (free && history.can_undo(), free && history.can_redo()))
    }

    /// Step the working copy's history when the board at `path` is in
    /// sketch mode; `None` when it is not. `Some(false)` while the Import
    /// DXF card holds the sketch, so the step cannot fall to the board.
    pub(crate) fn step_board_sketch_history(&mut self, path: &Path, undo: bool) -> Option<bool> {
        let session = self
            .interaction_state
            .board_sketch
            .as_mut()
            .filter(|session| session.board_path == path)?;
        if session.dxf_dialog.is_some() {
            return Some(false);
        }
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

/// The board command that commits `sketch` to `board`, with the designators
/// of the locked components an anchor leaves in place. A sketch that has
/// owned no pour, mechanical drawing or anchor, before or now, takes the
/// plain sketch command as it always did (decision 2, 2026-11-20).
fn board_sketch_command(
    sketch: signex_sketch::SketchData,
    board: &signex_types::pcb::PcbBoard,
) -> Result<(BoardCommand, Vec<String>), signex_bake::BoardBakeError> {
    let owned_before = board
        .sketch
        .as_ref()
        .is_some_and(|stored| stored.sketch.owns_board_items());
    if !(sketch.owns_board_items() || owned_before) {
        let baked = signex_bake::bake_board(&sketch, &board.nets)?;
        return Ok((BoardCommand::set_board_sketch(sketch, baked), Vec::new()));
    }
    let (baked, items) = signex_bake::board::bake_board_with_items(&sketch, board)?;
    let locked = items
        .locked_anchors(&board.footprints)
        .into_iter()
        .map(|anchor| anchor.reference.clone())
        .collect();
    let command = BoardCommand::set_board_sketch_with_items(sketch, baked, items);
    Ok((command, locked))
}

#[cfg(test)]
#[path = "board_sketch_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "board_sketch_parameter_tests.rs"]
mod parameter_tests;

#[cfg(test)]
#[path = "board_sketch_pattern_tests.rs"]
mod pattern_tests;
