//! Which project and board Update PCB acts on, from the document in view or
//! a project the Projects tree names, and the Design menu row's label.

use std::path::{Path, PathBuf};

use crate::app::state::{LoadedProject, ProjectId};
use crate::app::{Signex, TabKind};

use super::state::file_name;

/// The Design menu row's target: a schematic updates its project's board, a
/// board imports from its project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PcbSyncLabel {
    /// A schematic of a project; the board's file name when it has one.
    UpdateBoard(Option<String>),
    /// A board of a project; the project's file name.
    ImportFrom(String),
}

impl Signex {
    /// The document Update PCB starts from: inside a window's event the
    /// window's own (its documents are swapped into the main slots), else
    /// the main window's active tab.
    pub(super) fn update_pcb_source(&self) -> Option<PathBuf> {
        if self.interaction_state.acting_window.is_some() {
            self.document_state.active_path.clone()
        } else {
            self.active_tab_path()
        }
    }

    /// The project that owns the document at `path`: its tab's, else the
    /// one whose folder or sheet list holds it.
    pub(super) fn owning_project(&self, path: &Path) -> Option<&LoadedProject> {
        let docs = &self.document_state;
        docs.tabs
            .iter()
            .find(|tab| tab.path == path)
            .and_then(|tab| tab.project_id)
            .and_then(|id| docs.project_by_id(id))
            .or_else(|| docs.document_project(path))
            .or_else(|| docs.project_for_path(path))
    }

    /// The board file a project names, joined to its folder.
    pub(super) fn project_board(project: &LoadedProject) -> Option<PathBuf> {
        let file = project.data.pcb_file.as_deref()?.trim();
        (!file.is_empty()).then(|| project.dir().join(file))
    }

    /// The project of the document in view, for New PCB with no project
    /// named: the document's owner, else the active project.
    pub(super) fn project_in_view(&self) -> Option<ProjectId> {
        self.update_pcb_source()
            .and_then(|path| self.owning_project(&path).map(|p| p.id))
            .or(self.document_state.active_project)
    }

    /// The Design menu row's label target for `window` (`None`: the main
    /// window): the schematic or board that window shows, when a project
    /// owns it.
    pub(crate) fn pcb_sync_label(&self, window: Option<iced::window::Id>) -> Option<PcbSyncLabel> {
        let path = match window {
            Some(id) if self.ui_state.main_window_id != Some(id) => {
                self.undocked_document_path(id)?
            }
            _ => self.active_tab_path()?,
        };
        let project = self.owning_project(&path)?;
        let is_board = self
            .document_state
            .tabs
            .iter()
            .any(|tab| tab.path == path && matches!(tab.kind, TabKind::Pcb));
        if is_board {
            return Some(PcbSyncLabel::ImportFrom(file_name(&project.path)));
        }
        let is_sheet = self.document_state.engines.contains_key(&path);
        is_sheet.then(|| {
            PcbSyncLabel::UpdateBoard(Self::project_board(project).as_deref().map(file_name))
        })
    }

    /// Projects tree, project row: New PCB (`new_pcb`) or Update PCB Document
    /// for the project at `tree_path[0]`.
    pub(crate) fn pcb_sync_from_tree(
        &mut self,
        tree_path: &[usize],
        new_pcb: bool,
    ) -> iced::Task<crate::app::Message> {
        let Some(project) = tree_path
            .first()
            .and_then(|idx| self.document_state.projects.get(*idx))
            .map(|p| p.id)
        else {
            return iced::Task::none();
        };
        self.dispatch_update_pcb_message(if new_pcb {
            super::UpdatePcbMsg::NewPcbRequested(Some(project))
        } else {
            super::UpdatePcbMsg::RequestedForProject(project)
        })
    }

    /// The document an undocked tab window shows.
    fn undocked_document_path(&self, window: iced::window::Id) -> Option<PathBuf> {
        match self.ui_state.windows.get(&window)? {
            crate::app::state::WindowKind::UndockedTab { path, .. } => Some(path.clone()),
            _ => None,
        }
    }
}
