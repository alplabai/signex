//! Where a replayed document is shown, whether its targets are still there,
//! and holding the person's selection while Replay runs.

use std::path::{Path, PathBuf};

use pcb_geom::{ClickMode, ItemRef, Selection};
use signex_types::schematic::SelectedItem;

use crate::app::state::WindowKind;
use crate::app::{Signex, Tool};
use crate::panels::command_log::RecordedTargets;
use crate::pcb_canvas::PcbSelectionState;

/// The main window's selection, taken while Replay runs.
pub(super) struct HeldSelection {
    document: Option<PathBuf>,
    sheet: Vec<SelectedItem>,
    board: Selection,
}

/// A tab by path and kind: a project's Project and Rules tabs share a path.
pub(super) type TabKey = (PathBuf, std::mem::Discriminant<crate::app::TabKind>);

/// Where the targets' document is shown.
pub(super) enum Shown {
    Main,
    Window(iced::window::Id),
    /// In a tab behind the one in view, which is `back`.
    Behind {
        back: Option<TabKey>,
    },
}

pub(super) fn board_selection(items: &[ItemRef]) -> Selection {
    let mut selection = Selection::default();
    let _changed = selection.apply_marquee(items, ClickMode::Replace);
    selection
}

impl Signex {
    /// Whether a tool is in the middle of something in the main window: a
    /// placement, a wire, a Move or a board tool. Switching tabs or running
    /// a selection command now would end or redirect it.
    pub(super) fn tool_in_progress(&self) -> bool {
        let state = &self.interaction_state;
        !matches!(state.current_tool, Tool::Select)
            || state.wire_drawing
            || state.placement.is_some()
            || state.placement_pause.is_some()
            || state.pcb_carry.is_some()
            || state.pcb_tool.is_some()
            || self.ui_state.move_pick.is_some()
    }

    /// Where `document` is open, `None` when it is not.
    pub(super) fn where_shown(&self, document: &Path) -> Option<Shown> {
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
    pub(super) fn missing_targets(
        &self,
        document: &Path,
        targets: &RecordedTargets,
    ) -> Vec<String> {
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

    pub(super) fn still_on_sheet(
        &self,
        document: &Path,
        items: Vec<SelectedItem>,
    ) -> Vec<SelectedItem> {
        let Some(engine) = self.document_state.engines.get(document) else {
            return Vec::new();
        };
        items
            .into_iter()
            .filter(|item| engine.holds_selected_item(item))
            .collect()
    }

    /// Take the main window's selection, leaving it empty.
    pub(super) fn take_main_selection(&mut self) -> HeldSelection {
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

    pub(super) fn put_main_targets(&mut self, targets: &RecordedTargets) {
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

    pub(super) fn clear_main_selection(&mut self) {
        self.interaction_state.active_canvas_mut().selected.clear();
        self.interaction_state
            .pcb_selection
            .put_selection(Selection::default());
    }

    /// Put a held selection back, without what the command removed. A
    /// command that moved the view leaves it: the ids belong to the document
    /// they were picked in.
    pub(super) fn give_back_main_selection(&mut self, held: HeldSelection) {
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
    pub(super) fn return_to_tab(&mut self, key: Option<&TabKey>) {
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
}
