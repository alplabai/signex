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
use crate::app::new_project::{
    NewProjectMsg, NewProjectState, ProfileStored, import_rule_file, store_profile,
};
use crate::app::state::{LoadedProject, ProjectId};
use crate::app::view::dialogs::new_project::{cards, customize};
use crate::app::{Message, ProjectMsg, Signex};
use crate::styles::Spacing;
use signex_types::project::UserProfile;

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
    journal_lines_draft: Option<&'a str>,
    users: &[UserProfile],
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let sp = Spacing::default();
    let stored;
    let state = match draft {
        Some(draft) => draft,
        None => {
            stored = NewProjectState::of_project(&project.data, users);
            &stored
        }
    };
    let wrap = edited(project.id);
    let unreadable = project.data.has_unreadable_project_defaults().then(|| {
        signex_widgets::text(signex_i18n::t!(
            i18n,
            signex_i18n::ids::DIALOG_NEW_PROJECT_SETUP_UNREADABLE
        ))
        .size(crate::styles::TypeScale::default().caption)
        .color(crate::styles::ti(tokens.text_secondary))
    });
    let body: Element<'a, NewProjectMsg> = column![
        cards::profiles(state, users, i18n, tokens),
        row![
            container(customize::pcb_rules(state, i18n, tokens)).width(Length::FillPortion(1)),
            container(customize::schematic(state, i18n, tokens)).width(Length::FillPortion(1)),
        ]
        .spacing(sp.xl),
    ]
    .spacing(sp.xl + sp.s)
    .boxed();
    let mut col: iced::widget::Column<Element<'a, Message>> = column![].spacing(sp.m);
    if let Some(note) = unreadable {
        col = col.push(note.boxed());
    }
    let body = col
        .push(body.map(wrap).boxed())
        .push(super::agent_journals::view(
            project,
            journal_lines_draft,
            i18n,
            tokens,
        ));
    container(body).padding([sp.xl, sp.xl]).boxed()
}

/// Which stored part an edit of the section touches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SetupPart {
    Pcb,
    Sheet,
    Designators,
    Profile,
}

impl SetupPart {
    fn of(msg: &NewProjectMsg) -> Option<Self> {
        use NewProjectMsg as M;
        Some(match msg {
            M::ProfilePicked(_)
            | M::FabPicked(_)
            | M::RulesFileRead(_)
            | M::LayersStepped(_)
            | M::RuleEdited(..) => Self::Pcb,
            M::SheetSizePicked(_) | M::SheetOrientationPicked(_) | M::TitleBlockPicked(_) => {
                Self::Sheet
            }
            M::DesignatorsPicked(_) => Self::Designators,
            M::UserProfilePicked(_) => Self::Profile,
            _ => return None,
        })
    }
}

impl Signex {
    /// An edit in the Project setup section: only the part it changed is
    /// stored, as one General-history step. Typing in one rule field joins
    /// one step; every pick is its own. A pick that changes nothing (the
    /// card already selected) stores nothing, and a rule value that does not
    /// parse keeps its text and inline reason while the last good value
    /// stays stored, so the other picks are never dropped.
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
            return import_rule_file(
                &self.ui_state.io.picker,
                &self.ui_state.i18n,
                edited(project_id),
            );
        }
        let stored = project.data.project_defaults.clone();
        let stored_designators = project.data.designator_standard();
        let fresh = NewProjectState::of_project(&project.data, &self.ui_state.setup_profiles);
        let part = SetupPart::of(&msg);
        let typing = match &msg {
            NewProjectMsg::RuleEdited(field, _) => Some(*field),
            _ => None,
        };
        let tab = self
            .document_state
            .project_tabs
            .entry(project_id)
            .or_default();
        let draft = tab.setup_draft.get_or_insert(fresh);
        let users = &mut self.ui_state.setup_profiles;
        let picked = matches!(msg, NewProjectMsg::ProfilePicked(_));
        match store_profile(draft, users, &msg) {
            ProfileStored::NotAStore => {}
            ProfileStored::Removed { profile, at } => {
                // The project's own default profile is gone: the project no
                // longer names it, in the same edit.
                self.clear_removed_default_profile(project_id, &profile.id);
                return self.after_profile_store(ProfileStored::Removed { profile, at });
            }
            stored => return self.after_profile_store(stored),
        }
        let before = draft.setup.clone();
        if !draft.apply_setup(msg, users) || draft.setup == before {
            return iced::Task::none();
        }
        let Some(part) = part else {
            return iced::Task::none();
        };
        let mut defaults = stored;
        let mut designators = None;
        match part {
            // Only rules the loader accepts are stored (drill smaller than
            // the via, every value in range); otherwise the last good stored
            // rules stay and the inline reason shows.
            SetupPart::Pcb if !draft.setup.pcb.problems().is_empty() => {
                return iced::Task::none();
            }
            SetupPart::Pcb => {
                defaults.pcb = Some(draft.setup.pcb.clone());
                // Only a card pick names the project's default profile.
                if picked {
                    defaults.profile = Some(draft.setup.profile_id());
                }
            }
            SetupPart::Sheet => defaults.sheet = Some(draft.setup.sheet.clone()),
            // A user profile carries all three parts.
            SetupPart::Profile => {
                defaults = draft.setup.defaults();
                designators = (stored_designators != Some(draft.setup.designators))
                    .then_some(draft.setup.designators);
            }
            SetupPart::Designators => {
                designators = (stored_designators != Some(draft.setup.designators))
                    .then_some(draft.setup.designators);
            }
        }
        self.apply_project_general(
            project_id,
            ProjectGeneralCommand::SetProjectDefaults {
                defaults,
                designators,
                typing,
            },
        );
        iced::Task::none()
    }
}
