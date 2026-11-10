//! Cross Probe: show the selection's counterpart on the other side, bringing
//! a hidden tab forward or opening the board or sheet when it is not open.

use std::path::{Path, PathBuf};

use iced::Task;
use pcb_geom::ItemRef;
use signex_i18n::ids;
use signex_types::schematic::{InstancePath, SelectedItem};

use crate::app::project_sheets::project_root_sheet_path;
use crate::app::state::LoadedProject;
use crate::app::state::cross_select::PendingProbe;
use crate::app::{Message, Signex, TabKind};

impl Signex {
    /// The `cross_probe` command: note what the selection in view stands for.
    /// [`Self::run_pending_probe`] shows it after the update, outside any
    /// window's event, so it may switch tabs and open documents.
    pub(super) fn request_cross_probe(&mut self) {
        match self.probe_request() {
            Some(request) => self.interaction_state.cross_select.pending = Some(request),
            None => self.refuse_probe(ids::CROSS_PROBE_NOTHING_SELECTED),
        }
    }

    /// The probe for the board or sheet in view: the main window's, or inside
    /// a window's event that window's (its canvases are in the main slots).
    fn probe_request(&self) -> Option<PendingProbe> {
        let docs = &self.document_state;
        let in_window = self.interaction_state.acting_window.is_some();
        let path = if in_window {
            docs.active_path.clone()?
        } else {
            self.active_tab_path()?
        };
        let (what, to_board) = if docs.pcb_board(&path).is_some() {
            let state = &self.interaction_state.pcb_selection;
            let items: Vec<ItemRef> = if state.document() == Some(path.as_path()) {
                state.selection().items().collect()
            } else {
                Vec::new()
            };
            (self.cross_selection_from_board(&path, &items), false)
        } else if docs.engines.contains_key(&path)
            && docs.active_path.as_deref() == Some(path.as_path())
        {
            let items = &self.interaction_state.canvas.selected;
            (self.cross_selection_from_sheet(&path, items), true)
        } else {
            return None;
        };
        (!what.is_empty()).then_some(PendingProbe {
            what,
            to_board,
            from: path,
            opening: None,
        })
    }

    /// Run the pending Cross Probe, if any.
    pub(super) fn run_pending_probe(&mut self) -> Task<Message> {
        let Some(pending) = self.interaction_state.cross_select.pending.take() else {
            return Task::none();
        };
        if pending.to_board {
            self.probe_board(pending)
        } else {
            self.probe_sheet(pending)
        }
    }

    fn probe_board(&mut self, mut pending: PendingProbe) -> Task<Message> {
        if let Some(task) = self.show_probe_on_board(&pending) {
            return task;
        }
        if let Some(opening) = &pending.opening {
            return self.wait_or_drop(pending.clone(), opening, ids::CROSS_PROBE_NO_MATCH_BOARD);
        }
        let Some(board) = self.project_board_path(&pending.from) else {
            self.refuse_probe(ids::CROSS_PROBE_NO_BOARD);
            return Task::none();
        };
        if self.document_state.tabs.iter().any(|tab| tab.path == board) {
            self.refuse_probe(ids::CROSS_PROBE_NO_MATCH_BOARD);
            return Task::none();
        }
        let task = if self.is_opening(&board) {
            Task::none()
        } else {
            match self.open_path_reported(board.clone()) {
                Ok(task) => task,
                // The open said why in Messages and the error notice.
                Err(_) => return Task::none(),
            }
        };
        pending.opening = Some(board);
        self.interaction_state.cross_select.pending = Some(pending);
        task
    }

    /// Select and frame the probe on a board on screen, else on an open board
    /// tab brought forward. `None` when no open board has a match.
    fn show_probe_on_board(&mut self, pending: &PendingProbe) -> Option<Task<Message>> {
        for (window, path) in self.boards_shown() {
            let items = self.board_items(&path, &pending.what);
            if !items.is_empty() {
                return Some(self.show_on_board(window, &path, &items));
            }
        }
        let hidden = self
            .document_state
            .tabs
            .iter()
            .filter(|tab| matches!(tab.kind, TabKind::Pcb))
            .map(|tab| tab.path.clone())
            .find(|path| !self.board_items(path, &pending.what).is_empty())?;
        self.activate_tab_for_path(&hidden);
        let items = self.board_items(&hidden, &pending.what);
        Some(self.show_on_board(None, &hidden, &items))
    }

    fn show_on_board(
        &mut self,
        window: Option<iced::window::Id>,
        path: &Path,
        items: &[ItemRef],
    ) -> Task<Message> {
        self.set_board_selection(window, path, items);
        self.fit_board_items(window, path, items);
        window.map_or_else(Task::none, iced::window::gain_focus)
    }

    fn probe_sheet(&mut self, mut pending: PendingProbe) -> Task<Message> {
        if let Some(task) = self.show_probe_on_sheet(&pending) {
            return task;
        }
        if let Some(opening) = &pending.opening {
            return self.wait_or_drop(
                pending.clone(),
                opening,
                ids::CROSS_PROBE_NO_MATCH_SCHEMATIC,
            );
        }
        let Some((sheet, placement)) = self.probe_sheet_target(&pending) else {
            self.refuse_probe(ids::CROSS_PROBE_SHEET_UNKNOWN);
            return Task::none();
        };
        // An open sheet is focused at once and recorded at the placement.
        let task = self.open_sheet_with_placement(sheet.clone(), Some(placement));
        if let Some(shown) = self.show_probe_on_sheet(&pending) {
            return Task::batch([task, shown]);
        }
        if !self.is_opening(&sheet) {
            self.refuse_probe(ids::CROSS_PROBE_NO_MATCH_SCHEMATIC);
            return task;
        }
        pending.opening = Some(sheet);
        self.interaction_state.cross_select.pending = Some(pending);
        task
    }

    /// Select and frame the probe on a sheet on screen, else on an open sheet
    /// tab brought forward. `None` when no open sheet has a match.
    fn show_probe_on_sheet(&mut self, pending: &PendingProbe) -> Option<Task<Message>> {
        for (window, path) in self.sheets_shown() {
            let items = self.sheet_items_for(&path, &pending.what);
            if !items.is_empty() {
                return Some(self.show_on_sheet(window, items));
            }
        }
        let hidden = self
            .document_state
            .tabs
            .iter()
            .filter(|tab| matches!(tab.kind, TabKind::Schematic))
            .map(|tab| tab.path.clone())
            .find(|path| !self.sheet_items_for(path, &pending.what).is_empty())?;
        if !self.focus_open_sheet(&hidden) {
            return None;
        }
        let items = self.sheet_items_for(&hidden, &pending.what);
        Some(self.show_on_sheet(None, items))
    }

    fn show_on_sheet(
        &mut self,
        window: Option<iced::window::Id>,
        items: Vec<SelectedItem>,
    ) -> Task<Message> {
        self.set_sheet_selection(window, items);
        self.fit_sheet_selection(window);
        window.map_or_else(Task::none, iced::window::gain_focus)
    }

    /// Keep waiting while `opening` is still being read; once it is done and
    /// nothing matched, say so and drop the probe.
    fn wait_or_drop(
        &mut self,
        pending: PendingProbe,
        opening: &Path,
        no_match: signex_i18n::MessageId,
    ) -> Task<Message> {
        if self.is_opening(opening) {
            self.interaction_state.cross_select.pending = Some(pending);
        } else {
            self.refuse_probe(no_match);
        }
        Task::none()
    }

    /// A read of `path` is in flight.
    fn is_opening(&self, path: &Path) -> bool {
        self.document_state.pending_opens.contains(path)
            || self.ui_state.pending_sheet_opens.contains_key(path)
    }

    /// The project that owns the document at `path`: its tab's, else the one
    /// whose folder or sheet list holds it.
    fn probe_project(&self, path: &Path) -> Option<&LoadedProject> {
        let docs = &self.document_state;
        docs.tabs
            .iter()
            .find(|tab| tab.path == path)
            .and_then(|tab| tab.project_id)
            .and_then(|id| docs.projects.iter().find(|p| p.id == id))
            .or_else(|| docs.document_project(path))
    }

    /// The board file of the project that owns `from`.
    fn project_board_path(&self, from: &Path) -> Option<PathBuf> {
        let project = self.probe_project(from)?;
        let file = project.data.pcb_file.as_deref()?.trim();
        (!file.is_empty()).then(|| project.dir().join(file))
    }

    /// The sheet to open for the probe and the placement to show it through:
    /// the first part's sheet from the project hierarchy, else (nets only)
    /// the sheet of the first member of its nets.
    fn probe_sheet_target(&self, pending: &PendingProbe) -> Option<(PathBuf, InstancePath)> {
        let project = self.probe_project(&pending.from)?;
        let Some(part) = pending.what.parts.first() else {
            return self.sheet_of_nets(project.id, &pending.what.nets);
        };
        let root = project_root_sheet_path(project)?;
        let candidates: Vec<PathBuf> = self
            .hierarchy_describing(&root)
            .map(|(h, _)| {
                h.paths_at(&part.sheet)
                    .into_iter()
                    .map(Path::to_path_buf)
                    .collect()
            })
            .unwrap_or_default();
        // A root-chain link with no hierarchy yet is on the root sheet; of
        // several pages at the root chain, the one holding the unit wins.
        let holds_unit = |path: &PathBuf| {
            self.document_state
                .engines
                .get(path)
                .is_some_and(|e| e.document().symbols.iter().any(|s| s.uuid == part.symbol))
        };
        let sheet = candidates
            .iter()
            .find(|path| holds_unit(path))
            .or_else(|| candidates.first())
            .cloned()
            .or_else(|| part.sheet.is_root().then_some(root))?;
        Some((sheet, part.sheet.clone()))
    }
}
