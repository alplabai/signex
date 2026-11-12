//! Opening the New PCB / New Schematic dialog, its edits, Create and Cancel.

use iced::Task;
use signex_i18n::{ids, t};
use signex_types::project::{ProjectSetup, ResolvedSetup};

use super::{DocumentSetupMsg, DocumentSetupState, SetupKind};
use crate::app::new_project::{NewProjectMsg, NewProjectState, ProfileStored, store_profile};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

impl Signex {
    /// The project's setup as a dialog or command starts from it. A default
    /// profile the user no longer has is said in Messages; the stored values
    /// are used.
    pub(crate) fn resolved_project_setup(&self, project: ProjectId) -> Option<ResolvedSetup> {
        let loaded = self.document_state.project_by_id(project)?;
        let resolved = ProjectSetup::for_project(
            &loaded.data.project_defaults,
            loaded.data.designator_standard(),
            &self.ui_state.setup_profiles,
        );
        if let Some(missing) = &resolved.missing_profile {
            crate::diagnostics::log_warning(
                t!(
                    self.ui_state.i18n,
                    ids::SETUP_PROFILE_MISSING,
                    "profile" => missing.as_str(),
                    "project" => loaded.data.name.as_str()
                )
                .into_string(),
            );
        }
        Some(resolved)
    }

    /// New PCB or New Schematic for `project`, else the project in view:
    /// open the setup dialog, prefilled from the project. Nothing is made or
    /// written. A project that already has a board refuses New PCB here.
    pub(crate) fn open_document_setup(
        &mut self,
        kind: SetupKind,
        project: Option<ProjectId>,
    ) -> Task<Message> {
        self.interaction_state.context_menu = None;
        let Some(project) = project.or_else(|| self.project_in_view()) else {
            match kind {
                SetupKind::Pcb => self
                    .refuse_new_pcb(t!(self.ui_state.i18n, ids::NEW_PCB_NO_PROJECT).into_string()),
                SetupKind::Sheet => crate::diagnostics::log_warning(
                    t!(self.ui_state.i18n, ids::SETUP_SHEET_NO_PROJECT).into_string(),
                ),
            }
            return Task::none();
        };
        let Some(loaded) = self.document_state.project_by_id(project) else {
            return Task::none();
        };
        if kind == SetupKind::Pcb {
            // A board already open (or opening) refuses before the dialog;
            // a file on disk is checked after Create, off the update thread.
            let named = Self::project_board(loaded);
            let path = named
                .clone()
                .unwrap_or_else(|| crate::app::update_pcb::new_board_path(&loaded.path));
            if self.document_state.holds_pcb(&path) || self.is_board_opening(&path) {
                self.refuse_new_pcb_taken(&path, named.is_some());
                return Task::none();
            }
        }
        let project_name = loaded.data.name.clone();
        let Some(resolved) = self.resolved_project_setup(project) else {
            return Task::none();
        };
        let mut draft = NewProjectState::of_setup(resolved.setup);
        draft.customize_open = true;
        self.ui_state.document_setup = Some(DocumentSetupState {
            kind,
            project,
            project_name,
            draft,
        });
        Task::none()
    }

    pub(crate) fn dispatch_document_setup_message(
        &mut self,
        msg: DocumentSetupMsg,
    ) -> Task<Message> {
        match msg {
            DocumentSetupMsg::OpenRequested { kind, project } => {
                self.open_document_setup(kind, project)
            }
            DocumentSetupMsg::CancelRequested
            | DocumentSetupMsg::Setup(NewProjectMsg::CancelRequested) => {
                self.ui_state.document_setup = None;
                Task::none()
            }
            DocumentSetupMsg::SubmitRequested
            | DocumentSetupMsg::Setup(NewProjectMsg::SubmitRequested) => {
                self.create_from_document_setup()
            }
            DocumentSetupMsg::Setup(NewProjectMsg::ImportRequested) => {
                crate::app::new_project::import_rule_file(
                    &self.ui_state.io.picker,
                    &self.ui_state.i18n,
                    |msg| Message::from(DocumentSetupMsg::Setup(msg)),
                )
            }
            DocumentSetupMsg::Setup(edit) => {
                let ui = &mut self.ui_state;
                let Some(state) = ui.document_setup.as_mut() else {
                    return Task::none();
                };
                match store_profile(&mut state.draft, &mut ui.setup_profiles, &edit) {
                    ProfileStored::NotAStore => {
                        state.draft.apply_setup(edit, &ui.setup_profiles);
                        Task::none()
                    }
                    stored => self.after_profile_store(stored),
                }
            }
        }
    }

    /// Create: close the dialog and make the document with the values shown.
    /// Ignored while a value is not valid.
    fn create_from_document_setup(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.document_setup.as_ref() else {
            return Task::none();
        };
        if !state.can_create() {
            return Task::none();
        }
        let Some(state) = self.ui_state.document_setup.take() else {
            return Task::none();
        };
        match state.kind {
            SetupKind::Pcb => {
                self.request_new_pcb(Some(state.project), Some(state.draft.setup.pcb))
            }
            SetupKind::Sheet => {
                let choice = state.sheet_choice();
                self.pick_new_schematic_path(state.project, choice)
            }
        }
    }
}
