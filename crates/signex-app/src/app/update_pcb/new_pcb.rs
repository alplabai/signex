//! New PCB in a project (`new_pcb_document`): one board per project, named
//! after the project, starting from the project defaults (#2232). It opens
//! as an unsaved tab; nothing is written until the user saves.

use std::path::{Path, PathBuf};

use iced::Task;
use signex_engine::pcb::BoardEngine;
use signex_i18n::{ids, t};
use signex_types::pcb::PcbBoard;
use signex_types::project::SetupProfile;

use super::UpdatePcbMsg;
use super::state::{Phase, file_name};
use crate::app::state::{ErrorNotice, ProjectId};
use crate::app::{Message, Signex, TabDocument, TabInfo, TabKind};

/// Copper layers of a new board in a project with no PCB defaults: the
/// Standard profile's.
const FALLBACK_LAYERS: u8 = 4;

impl Signex {
    /// New PCB for `project`, else the project in view: check the board's
    /// path on the worker before anything is made.
    pub(super) fn request_new_pcb(&mut self, project: Option<ProjectId>) -> Task<Message> {
        let Some(project) = project.or_else(|| self.project_in_view()) else {
            self.refuse_new_pcb(self.update_pcb_text(ids::NEW_PCB_NO_PROJECT));
            return Task::none();
        };
        let Some(loaded) = self.document_state.project_by_id(project) else {
            return Task::none();
        };
        if let Some(board) = Self::project_board(loaded) {
            let why = t!(self.ui_state.i18n, ids::NEW_PCB_HAS_BOARD, "board" => file_name(&board));
            self.refuse_new_pcb(why.into_string());
            return Task::none();
        }
        let path = new_board_path(&loaded.path);
        if self.document_state.holds_pcb(&path) {
            self.refuse_new_pcb_exists(&path);
            return Task::none();
        }
        let checked = path.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || checked.try_exists().unwrap_or(true))
                    .await
                    .unwrap_or(true)
            },
            move |exists| {
                Message::from(UpdatePcbMsg::NewPcbPathChecked {
                    project,
                    path,
                    exists,
                })
            },
        )
    }

    /// The path check landed: open the new board as an unsaved tab and name
    /// it in the project (the project is then unsaved too).
    pub(super) fn new_pcb_path_checked(&mut self, project: ProjectId, path: PathBuf, exists: bool) {
        if exists || self.document_state.holds_pcb(&path) {
            self.refuse_new_pcb_exists(&path);
            return;
        }
        let Some(loaded) = self.document_state.project_by_id_mut(project) else {
            return;
        };
        if loaded.data.pcb_file.is_some() {
            return;
        }
        let board = loaded
            .data
            .new_board()
            .unwrap_or_else(|| SetupProfile::Standard.pcb(FALLBACK_LAYERS).new_board());
        loaded.data.pcb_file = Some(file_name(&path));
        let project_path = loaded.path.clone();
        self.document_state.mark_document_dirty(&project_path);
        self.open_new_pcb_tab(path.clone(), board);
        crate::diagnostics::log_info(
            t!(
                self.ui_state.i18n,
                ids::NEW_PCB_CREATED,
                "board" => file_name(&path),
                "project" => file_name(&project_path)
            )
            .into_string(),
        );
        self.refresh_panel_ctx();
        // The Update PCB dialog that offered Create PCB now has its board.
        if let Some(state) = self.ui_state.update_pcb.as_mut()
            && state.project == project
            && matches!(state.phase, Phase::NoBoard)
        {
            state.board = Some(path);
            state.phase = Phase::Opening;
        }
    }

    /// A tab for a board that is not on disk yet: dirty from the start, its
    /// history unknown to any file.
    fn open_new_pcb_tab(&mut self, path: PathBuf, board: PcbBoard) {
        self.park_active_schematic_session();
        let project_id = self.document_state.project_for_path(&path).map(|p| p.id);
        let revision = self.document_state.stamp_pcb_revision(&path);
        self.document_state.tabs.push(TabInfo {
            title: file_name(&path),
            path: path.clone(),
            cached_document: Some(TabDocument::Pcb(BoardEngine::unsaved(board, revision))),
            project_id,
            kind: TabKind::Pcb,
        });
        self.document_state.active_tab = self.document_state.tabs.len() - 1;
        self.apply_loaded_pcb_document(true, true);
        self.document_state.mark_document_dirty(&path);
        self.reconcile_document_ids();
    }

    fn refuse_new_pcb_exists(&mut self, path: &Path) {
        let why = t!(self.ui_state.i18n, ids::NEW_PCB_EXISTS, "file" => file_name(path));
        self.refuse_new_pcb(why.into_string());
    }

    fn refuse_new_pcb(&mut self, why: String) {
        crate::diagnostics::log_warning(why.clone());
        let title = self.update_pcb_text(ids::NEW_PCB_TITLE);
        self.document_state.error_notice = Some(ErrorNotice::new(title, why));
    }
}

/// `<project folder>/<project name>.snxpcb`.
fn new_board_path(project: &Path) -> PathBuf {
    let stem = project
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    project.with_file_name(format!("{stem}.snxpcb"))
}
