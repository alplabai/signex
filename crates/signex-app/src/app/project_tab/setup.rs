//! Project setup section of the Project tab (#2232): the board rules and
//! sheet defaults the project was created with, edited with the
//! New Project dialog's own cards and fields. An edit is a General-history
//! step on the project's data and marks the `.snxprj` dirty; the file is
//! written by the normal Save, like every other section.

use iced::widget::{column, container, row};
use iced::{Element, Length, Widget as _};
use signex_engine::ProjectGeneralCommand;
use signex_i18n::I18n;
use signex_types::theme::ThemeTokens;

use super::ProjectTabMsg;
use crate::app::new_project::{NewProjectMsg, NewProjectState, import_rule_file};
use crate::app::state::{LoadedProject, ProjectId};
use crate::app::view::dialogs::new_project::{cards, customize};
use crate::app::{Message, ProjectMsg, Signex};
use crate::styles::Spacing;

fn edited(project_id: ProjectId) -> impl Fn(NewProjectMsg) -> Message + Clone + Send + 'static {
    move |msg| {
        Message::Project(ProjectMsg::Tab(ProjectTabMsg::SetupEdited {
            project_id,
            msg,
        }))
    }
}

/// The section: profile cards, then the rules and the sheet
/// defaults side by side. `draft` is the half-edited state, else the stored
/// values are shown.
pub(super) fn view<'a>(
    project: &LoadedProject,
    draft: Option<&NewProjectState>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let sp = Spacing::default();
    let stored;
    let state = match draft {
        Some(draft) => draft,
        None => {
            stored = NewProjectState::of_project(&project.data);
            &stored
        }
    };
    let wrap = edited(project.id);
    let body: Element<'a, NewProjectMsg> = column![
        cards::profiles(state, i18n, tokens),
        row![
            container(customize::pcb_rules(state, i18n, tokens)).width(Length::FillPortion(1)),
            container(customize::schematic(state, i18n, tokens)).width(Length::FillPortion(1)),
        ]
        .spacing(sp.xl),
    ]
    .spacing(sp.xl + sp.s)
    .boxed();
    container(body.map(wrap)).padding([sp.xl, sp.xl]).boxed()
}

impl Signex {
    /// An edit in the Project setup section. A valid draft is stored in the
    /// project's data as one General-history step; a draft with a bad value
    /// keeps its text and its inline reason and stores nothing.
    pub(crate) fn edit_project_setup(
        &mut self,
        project_id: ProjectId,
        msg: NewProjectMsg,
    ) -> iced::Task<Message> {
        let Some(project) = self.document_state.project_by_id(project_id) else {
            crate::diagnostics::log_warning(
                "Project tab: the project was closed; the edit was dropped",
            );
            return iced::Task::none();
        };
        if matches!(msg, NewProjectMsg::ImportRequested) {
            return import_rule_file(&self.ui_state.io.picker, edited(project_id));
        }
        let stored_designators = project.data.designator_standard();
        let fresh = NewProjectState::of_project(&project.data);
        let tab = self
            .document_state
            .project_tabs
            .entry(project_id)
            .or_default();
        let draft = tab.setup_draft.get_or_insert(fresh);
        if !draft.apply_setup(msg) || !draft.rules_valid() {
            return iced::Task::none();
        }
        let defaults = draft.setup.defaults();
        let designators = draft.setup.designators;
        self.apply_project_setup_change(project_id, defaults, designators, stored_designators);
        iced::Task::none()
    }

    fn apply_project_setup_change(
        &mut self,
        project_id: ProjectId,
        defaults: signex_types::project::ProjectDefaults,
        designators: signex_types::designator::DesignatorStandard,
        stored_designators: Option<signex_types::designator::DesignatorStandard>,
    ) {
        self.apply_project_general(
            project_id,
            ProjectGeneralCommand::SetProjectDefaults(defaults),
        );
        if stored_designators != Some(designators) {
            self.apply_project_general(
                project_id,
                ProjectGeneralCommand::SetDesignatorStandard(Some(designators)),
            );
        }
    }
}
