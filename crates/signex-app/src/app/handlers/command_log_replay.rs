//! Command Log Replay on recorded targets (Caner, 2026-11-22). A command that
//! acts on the selection records what it acted on; Replay runs it on exactly
//! those items and puts the person's selection back. Rules:
//! `docs/COMMAND_LOG.md`, "Replay".

use std::path::{Path, PathBuf};

use iced::Task;
use pcb_geom::{ClickMode, ItemRef, Selection};
use signex_i18n::{RenderTranslatable, Translatable, ids};
use signex_types::schematic::SelectedItem;

use crate::app::command::CommandArgs;
use crate::app::core::command_log::RecordedTargets;
use crate::app::state::WindowKind;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::panels::command_log::{LogRow, LogSource};
use crate::pcb_canvas::PcbSelectionState;

/// How Replay treats a command with no operands of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReplayScope {
    /// Finishes inside its dispatch, on the selection: runs on the targets.
    OnTargets,
    /// Reads the selection but goes on afterwards (a tool, a dialog, a later
    /// paste): the selection could not be put back, so it is refused.
    LiveSelection,
    /// Its effect is the selection itself: runs as it is.
    SetsSelection,
    /// Has no target: runs as it is, never seeing the current selection.
    NoTarget,
}

/// The scope of the command `id`.
pub(super) fn replay_scope(id: &str) -> ReplayScope {
    match id {
        "delete_selection"
        | "cut"
        | "copy"
        | "rotate_clockwise"
        | "rotate_counterclockwise"
        | "rotate_selection"
        | "rotate_selection_cw"
        | "mirror_x"
        | "mirror_y"
        | "flip_selected_x"
        | "flip_selected_y"
        | "flip_selection"
        | "bring_drawings_to_front"
        | "send_drawings_to_back"
        | "bring_drawings_forward"
        | "send_drawings_backward" => ReplayScope::OnTargets,
        "select_all" | "unselect_all" => ReplayScope::SetsSelection,
        "duplicate"
        | "move_selection"
        | "move_object"
        | "edit_object_properties"
        | "edit_selected_object_properties"
        | "find_similar_objects"
        | "select_connection"
        | "select_expand_connection"
        | "select_node_or_connection_item"
        | "toggle_selection"
        | "cross_probe"
        | "zoom_to_selection_area"
        | "remove_last_vertex"
        | "change_footprint"
        | "change_component"
        | "swap_pins"
        | "swap_gates"
        | "save_board_reuse_block"
        | "place_board_reuse_block"
        | "repour_selected"
        | "lock_board_items"
        | "unlock_board_items"
        | "stitch_vias"
        | "add_teardrops"
        | "remove_teardrops"
        | "gloss_selected_routes"
        | "board_shape_from_selection"
        | "keepout_from_selection"
        | "arrange_in_box"
        | "tune_net_length"
        | "tune_diff_pair"
        | "create_union"
        | "add_to_union"
        | "remove_from_union"
        | "break_union"
        | "sync_sheet_entries"
        | "sync_sheet_pins"
        | "open_child_sheet"
        | "create_sheet_from_sheet_symbol"
        | "open_datasheet"
        | "edit_library_symbol"
        | "edit_selected_symbol_in_symbol_editor"
        | "edit_blanket"
        | "edit_no_erc_rules" => ReplayScope::LiveSelection,
        _ => ReplayScope::NoTarget,
    }
}

/// The main window's selection, taken while Replay runs.
struct HeldSelection {
    document: Option<PathBuf>,
    sheet: Vec<SelectedItem>,
    board: Selection,
}

/// Where the targets' document is shown.
enum Shown {
    Main,
    Window(iced::window::Id),
    /// In a tab behind the one in view, which is `back`.
    Behind {
        back: Option<TabKey>,
    },
}

/// A tab by path and kind: a project's Project and Rules tabs share a path.
type TabKey = (PathBuf, std::mem::Discriminant<crate::app::TabKind>);

fn board_selection(items: &[ItemRef]) -> Selection {
    let mut selection = Selection::default();
    let _changed = selection.apply_marquee(items, ClickMode::Replace);
    selection
}

impl Signex {
    /// What `command`, about to run, acts on: `None` unless it is a person's
    /// outermost command that acts on a selection and has one.
    pub(crate) fn command_targets(
        &self,
        command: &AppCommandId,
        has_operands: bool,
        window: Option<iced::window::Id>,
    ) -> Option<RecordedTargets> {
        let is_person = matches!(self.dispatch_source(window), LogSource::Gui { .. });
        if has_operands
            || !is_person
            || self.ui_state.command_log.nesting != 0
            || replay_scope(command.as_str()) != ReplayScope::OnTargets
        {
            return None;
        }
        let undocked = window.filter(|id| self.is_document_window(*id));
        let interaction = &self.interaction_state;
        let board = match undocked {
            Some(id) => self
                .undocked_pcb_path(id)
                .and_then(|_| interaction.pcb_selections.get(&id)),
            None => self.has_active_pcb().then_some(&interaction.pcb_selection),
        };
        if let Some(board) = board {
            let items: Vec<ItemRef> = board.selection().items().collect();
            return (!items.is_empty()).then_some(RecordedTargets::Board(items));
        }
        let (canvas, engine) = match undocked {
            Some(id) => (
                interaction.canvas_for_window(id),
                self.document_state.engine_for_window(id, &self.ui_state),
            ),
            None => (
                interaction.active_canvas(),
                self.document_state.active_engine(),
            ),
        };
        engine?;
        (!canvas.selected.is_empty()).then(|| RecordedTargets::Sheet(canvas.selected.clone()))
    }

    /// Replay a row that has no operands of its own.
    pub(super) fn replay_without_operands(
        &mut self,
        row: &LogRow,
        command: &AppCommandId,
    ) -> Task<Message> {
        match replay_scope(command.as_str()) {
            ReplayScope::SetsSelection => {
                self.dispatch_command_reporting(command, CommandArgs::none(), None)
                    .1
            }
            ReplayScope::NoTarget => {
                let held = self.take_main_selection();
                let (_, task) = self.dispatch_command_reporting(command, CommandArgs::none(), None);
                self.give_back_main_selection(held);
                task
            }
            ReplayScope::LiveSelection => {
                let reason = Translatable::new(ids::COMMAND_LOG_REPLAY_LIVE_SELECTION)
                    .arg("command", row.label.clone());
                self.refuse_replay(row, reason);
                Task::none()
            }
            ReplayScope::OnTargets => {
                let targets = self.ui_state.command_log.log.targets_of(row.seq).cloned();
                match (targets, row.document.clone()) {
                    (Some(targets), Some(document)) => {
                        self.replay_on_targets(row, command, &targets, &document)
                    }
                    _ => {
                        self.refuse_replay(
                            row,
                            Translatable::new(ids::COMMAND_LOG_REPLAY_NO_TARGETS),
                        );
                        Task::none()
                    }
                }
            }
        }
    }

    /// Run `command` on `targets` in `document`, or refuse with what is
    /// missing and change nothing.
    fn replay_on_targets(
        &mut self,
        row: &LogRow,
        command: &AppCommandId,
        targets: &RecordedTargets,
        document: &Path,
    ) -> Task<Message> {
        let name = document_name(document);
        let Some(shown) = self.where_shown(document) else {
            let reason =
                Translatable::new(ids::COMMAND_LOG_REPLAY_DOCUMENT_CLOSED).arg("document", name);
            self.refuse_replay(row, reason);
            return Task::none();
        };
        let missing = self.missing_targets(document, targets);
        if !missing.is_empty() {
            let reason = Translatable::new(ids::COMMAND_LOG_REPLAY_TARGET_GONE)
                .arg("count", missing.len())
                .arg("document", name)
                .arg("missing", missing.join(", "));
            self.refuse_replay(row, reason);
            return Task::none();
        }
        match shown {
            Shown::Window(window) => self.replay_in_window(command, targets, document, window),
            Shown::Main => self.replay_in_main(command, targets, None),
            Shown::Behind { back } => {
                let held = self.take_main_selection();
                if !self.focus_open_sheet(document) {
                    self.give_back_main_selection(held);
                    let reason = Translatable::new(ids::COMMAND_LOG_REPLAY_DOCUMENT_BUSY)
                        .arg("document", name);
                    self.refuse_replay(row, reason);
                    return Task::none();
                }
                let task = self.replay_in_main(command, targets, Some(&back));
                self.give_back_main_selection(held);
                task
            }
        }
    }

    /// Run `command` on `targets` in the main window, then return to the tab
    /// `back` when Replay brought another one forward. A selection left on
    /// the targets' tab goes with it; on the tab in view the caller's or
    /// this function's held selection is put back.
    fn replay_in_main(
        &mut self,
        command: &AppCommandId,
        targets: &RecordedTargets,
        back: Option<&Option<TabKey>>,
    ) -> Task<Message> {
        let held = back.is_none().then(|| self.take_main_selection());
        self.put_main_targets(targets);
        let (_, task) = self.dispatch_command_reporting(command, CommandArgs::none(), None);
        self.clear_main_selection();
        if let Some(back) = back {
            self.return_to_tab(back.as_ref());
        }
        if let Some(held) = held {
            self.give_back_main_selection(held);
        }
        task
    }

    /// Run `command` on `targets` in the undocked `window`, and put its own
    /// selection back.
    fn replay_in_window(
        &mut self,
        command: &AppCommandId,
        targets: &RecordedTargets,
        document: &Path,
        window: iced::window::Id,
    ) -> Task<Message> {
        let revision = self.board_revision(document);
        let interaction = &mut self.interaction_state;
        let (held_sheet, held_board) = match targets {
            RecordedTargets::Sheet(items) => {
                let canvas = interaction.canvases.get_mut(&window);
                let held =
                    canvas.map(|canvas| std::mem::replace(&mut canvas.selected, items.clone()));
                (held, None)
            }
            RecordedTargets::Board(items) => {
                let selection = interaction.pcb_selections.entry(window).or_default();
                selection.sync(Some(document), revision);
                let held = selection.selection().clone();
                selection.put_selection(board_selection(items));
                (None, Some(held))
            }
        };
        let (_, task) = self.dispatch_command_reporting(command, CommandArgs::none(), Some(window));
        if let Some(held) = held_sheet {
            let kept = self.still_on_sheet(document, held);
            if let Some(canvas) = self.interaction_state.canvases.get_mut(&window) {
                canvas.selected = kept;
                canvas.clear_overlay_cache();
            }
        }
        if let Some(held) = held_board {
            let revision = self.board_revision(document);
            if let Some(selection) = self.interaction_state.pcb_selections.get_mut(&window) {
                selection.sync(Some(document), revision);
                selection.put_selection(held);
            }
            self.refresh_undocked_pcb_highlights(document);
        }
        self.refresh_window_panel_ctxs(None);
        task
    }

    /// Where `document` is open, `None` when it is not.
    fn where_shown(&self, document: &Path) -> Option<Shown> {
        let docs = &self.document_state;
        let active = docs.tabs.get(docs.active_tab);
        if active.is_some_and(|tab| tab.path == document) {
            return Some(Shown::Main);
        }
        let window = self.ui_state.windows.iter().find_map(|(id, kind)| {
            matches!(kind, WindowKind::UndockedTab { path, .. } if path == document).then_some(*id)
        });
        if let Some(window) = window {
            return Some(Shown::Window(window));
        }
        docs.tabs
            .iter()
            .any(|tab| tab.path == document)
            .then(|| Shown::Behind {
                back: active.map(|tab| (tab.path.clone(), std::mem::discriminant(&tab.kind))),
            })
    }

    /// The names of the targets no longer in `document`.
    fn missing_targets(&self, document: &Path, targets: &RecordedTargets) -> Vec<String> {
        let names = targets.names();
        let present: Vec<bool> = match targets {
            RecordedTargets::Sheet(items) => {
                let engine = self.document_state.engines.get(document);
                items
                    .iter()
                    .map(|item| engine.is_some_and(|e| e.holds_selected_item(item)))
                    .collect()
            }
            RecordedTargets::Board(items) => {
                // A fresh state: an item on a hidden layer is still there.
                let mut probe = PcbSelectionState::default();
                if let Some(board) = self.document_state.pcb_board(document) {
                    probe.select_items(board, items);
                }
                items
                    .iter()
                    .map(|item| probe.selection().contains(*item))
                    .collect()
            }
        };
        names
            .into_iter()
            .zip(present)
            .filter_map(|(name, is_there)| (!is_there).then_some(name))
            .collect()
    }

    fn still_on_sheet(&self, document: &Path, items: Vec<SelectedItem>) -> Vec<SelectedItem> {
        let Some(engine) = self.document_state.engines.get(document) else {
            return Vec::new();
        };
        items
            .into_iter()
            .filter(|item| engine.holds_selected_item(item))
            .collect()
    }

    /// Take the main window's selection, leaving it empty.
    fn take_main_selection(&mut self) -> HeldSelection {
        let document = self.active_tab_path();
        let sheet = std::mem::take(&mut self.interaction_state.active_canvas_mut().selected);
        let board = self.interaction_state.pcb_selection.selection().clone();
        self.interaction_state
            .pcb_selection
            .put_selection(Selection::default());
        HeldSelection {
            document,
            sheet,
            board,
        }
    }

    fn put_main_targets(&mut self, targets: &RecordedTargets) {
        match targets {
            RecordedTargets::Sheet(items) => {
                self.interaction_state.active_canvas_mut().selected = items.clone();
            }
            RecordedTargets::Board(items) => {
                let selection = board_selection(items);
                let _on_board = self.with_pcb_selection(|state, _| state.put_selection(selection));
            }
        }
    }

    fn clear_main_selection(&mut self) {
        self.interaction_state.active_canvas_mut().selected.clear();
        self.interaction_state
            .pcb_selection
            .put_selection(Selection::default());
    }

    /// Put a held selection back, without what the command removed. A
    /// command that moved the view leaves it: the ids belong to the document
    /// they were picked in.
    fn give_back_main_selection(&mut self, held: HeldSelection) {
        if self.active_tab_path() == held.document {
            let kept = match held.document.as_deref() {
                Some(document) => self.still_on_sheet(document, held.sheet),
                None => Vec::new(),
            };
            self.interaction_state.active_canvas_mut().selected = kept;
            let board = held.board;
            let _on_board = self.with_pcb_selection(|state, _| state.put_selection(board));
        }
        self.interaction_state
            .active_canvas_mut()
            .clear_overlay_cache();
        if self.has_active_pcb() {
            self.refresh_pcb_highlight();
        }
        self.update_selection_info();
    }

    /// Bring the tab `key` forward again.
    fn return_to_tab(&mut self, key: Option<&TabKey>) {
        let Some((path, kind)) = key else {
            return;
        };
        let docs = &self.document_state;
        let at = docs
            .tabs
            .iter()
            .position(|tab| tab.path == *path && std::mem::discriminant(&tab.kind) == *kind);
        if let Some(index) = at
            && index != docs.active_tab
        {
            self.park_active_schematic_session();
            self.document_state.active_tab = index;
            self.sync_active_tab();
        }
    }

    /// Refuse a Replay: a new failed row, and a Messages line because a
    /// paused log or a filter can hide that row.
    fn refuse_replay(&mut self, row: &LogRow, reason: Translatable) {
        let reason = reason.render_en_us();
        crate::diagnostics::log_warning(&reason);
        self.record_refused_replay(row, reason);
    }
}

fn document_name(document: &Path) -> String {
    document.file_name().map_or_else(
        || document.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}
