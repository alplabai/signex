//! Project setup section of the Project tab (#2232): the board rules and
//! sheet defaults the project was created with, edited with the
//! New Project dialog's own cards and fields. An edit is a General-history
//! step on the project's data and marks the `.snxprj` dirty; the file is
//! written by the normal Save, like every other section.

use iced::widget::{container, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_engine::ProjectGeneralCommand;
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;

use super::ProjectTabMsg;
use super::look::{ButtonKind, CardHeader, Glyph, card, text_button, warn_line};
use super::table::{SP, TS, section_with_action};
use crate::app::new_project::{
    NewProjectMsg, NewProjectState, ProfileStored, import_rule_file, store_profile,
};
use crate::app::state::{LoadedProject, ProjectId};
use crate::app::view::dialogs::new_project::{cards, customize, user_cards};
use crate::app::{Message, ProjectMsg, Signex};
use signex_types::project::UserProfile;

fn edited(project_id: ProjectId) -> impl Fn(NewProjectMsg) -> Message + Clone + Send + 'static {
    move |msg| {
        Message::Project(ProjectMsg::Tab(ProjectTabMsg::SetupEdited {
            project_id,
            msg,
        }))
    }
}

/// The section (mockup row 37): the page header with Save as profile, the
/// profile cards, then the PCB rules and Schematic cards side by side.
/// `draft` is the half-edited state, else the stored values are shown.
pub(super) fn view<'a>(
    project: &LoadedProject,
    draft: Option<&NewProjectState>,
    journal_lines_draft: Option<&'a str>,
    users: &[UserProfile],
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let stored;
    let state = match draft {
        Some(draft) => draft,
        None => {
            stored = NewProjectState::of_project(&project.data, users);
            &stored
        }
    };
    let mut body: Vec<Element<'a, Message>> = Vec::new();
    if project.data.has_unreadable_project_defaults() {
        let note = signex_widgets::text(t!(i18n, ids::DIALOG_NEW_PROJECT_SETUP_UNREADABLE))
            .size(TS.caption)
            .boxed();
        body.push(warn_line(Glyph::Warn, note, None, tokens));
    }
    body.push(
        cards::tab_choices(state, users, i18n, tokens)
            .map(edited(project.id))
            .boxed(),
    );
    body.push(rule_cards(project.id, state, i18n, tokens));
    body.push(super::agent_journals::view(
        project,
        journal_lines_draft,
        i18n,
        tokens,
    ));
    section_with_action(
        ids::PROJECT_SECTION_SETUP,
        ids::PROJECT_SETUP_HELP,
        save_action(project.id, state, i18n, tokens),
        body,
        i18n,
        tokens,
    )
}

/// The header's Save as profile button, or its name field while open.
fn save_action<'a>(
    project_id: ProjectId,
    state: &NewProjectState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let wrap = edited(project_id);
    if state.save_profile_name.is_some() {
        return user_cards::save_as(state, i18n, tokens).map(wrap).boxed();
    }
    text_button(
        Some(Glyph::Save),
        t!(i18n, ids::SETUP_PROFILE_SAVE_AS),
        Some(wrap(NewProjectMsg::SaveProfileOpened)),
        ButtonKind::Outline,
        tokens,
    )
}

/// The PCB rules and Schematic cards, side by side.
fn rule_cards<'a>(
    project_id: ProjectId,
    state: &NewProjectState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let titled = |title: Localized, fields: Element<'a, NewProjectMsg>| {
        let header = CardHeader {
            title,
            trailing: Vec::new(),
        };
        container(card(
            Some(header),
            fields.map(edited(project_id)).boxed(),
            tokens,
        ))
        .width(Length::FillPortion(1))
    };
    row![
        titled(
            t!(i18n, ids::PROJECT_SETUP_PCB_RULES),
            customize::pcb_rule_fields(state, true, i18n, tokens),
        ),
        titled(
            t!(i18n, ids::PROJECT_SETUP_SCHEMATIC),
            customize::schematic_fields(state, i18n, tokens),
        ),
    ]
    .spacing(SP.l)
    .align_y(Alignment::Start)
    .boxed()
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
            | M::CardLayersStepped(..)
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
        let fresh = NewProjectState::of_project(&project.data, &self.document_state.setup_profiles);
        let part = SetupPart::of(&msg);
        let typing = match &msg {
            NewProjectMsg::RuleEdited(field, _) => Some(*field),
            _ => None,
        };
        // A tier pick also writes its `fab_*` rules (#2311), even when the
        // values it fills are already shown: an edited fab rule comes back.
        let tier_rules = match &msg {
            NewProjectMsg::FabPicked(tier) => {
                Some((*tier, self.rules_with_tier(project_id, *tier)))
            }
            _ => None,
        };
        let tab = self
            .document_state
            .project_tabs
            .entry(project_id)
            .or_default();
        let draft = tab.setup_draft.get_or_insert(fresh);
        let users = &mut self.document_state.setup_profiles;
        // A card pick, the Fab tier field that mirrors it and a card's layer
        // stepper name the project's default profile.
        let picked = matches!(
            msg,
            NewProjectMsg::ProfilePicked(_)
                | NewProjectMsg::FabPicked(_)
                | NewProjectMsg::CardLayersStepped(..)
        );
        // Edit profile... changes the profile only, never the project
        // (`docs/PROJECT_SETUP.md`): while it is open the section's draft is
        // the profile's, and Save or Cancel shows the project's values again.
        let profile_edit =
            draft.editing.is_some() || matches!(msg, NewProjectMsg::ProfileEditRequested(_));
        let edit_closing = matches!(
            msg,
            NewProjectMsg::ProfileEditSaved | NewProjectMsg::ProfileEditCancelled
        );
        match store_profile(draft, users, &msg) {
            ProfileStored::NotAStore => {}
            stored if edit_closing && draft.editing.is_none() => {
                self.drop_setup_draft(project_id);
                return self.after_profile_store(stored);
            }
            ProfileStored::Removed { profile, at } if profile.edited => {
                // Reset to default: the project keeps the values it copied, so
                // the section shows them again, not the shipped ones.
                self.drop_setup_draft(project_id);
                return self.after_profile_store(ProfileStored::Removed { profile, at });
            }
            ProfileStored::Removed { profile, at } => {
                // The project's own default profile is gone: the project no
                // longer names it, in the same edit.
                self.clear_removed_default_profile(project_id, &profile.id);
                return self.after_profile_store(ProfileStored::Removed { profile, at });
            }
            stored => return self.after_profile_store(stored),
        }
        if profile_edit {
            draft.apply_setup(msg, users);
            if edit_closing && draft.editing.is_none() {
                self.drop_setup_draft(project_id);
            }
            return iced::Task::none();
        }
        let before = draft.setup.clone();
        if !draft.apply_setup(msg, users) || (draft.setup == before && tier_rules.is_none()) {
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
                // The project's dimension defaults (#2868) are not a part
                // of a profile: they stay.
                let dimension = defaults.dimension.take();
                defaults = draft.setup.defaults();
                defaults.dimension = dimension;
                designators = (stored_designators != Some(draft.setup.designators))
                    .then_some(draft.setup.designators);
            }
            SetupPart::Designators => {
                designators = (stored_designators != Some(draft.setup.designators))
                    .then_some(draft.setup.designators);
            }
        }
        let command = match tier_rules {
            Some((_, Ok(rules))) => ProjectGeneralCommand::SetFabTier {
                defaults,
                rules: Box::new(rules),
            },
            Some((tier, Err(why))) => {
                crate::diagnostics::log_warning(format!(
                    "Project setup: the {} fab tier rules were not added: {why}",
                    tier.token()
                ));
                ProjectGeneralCommand::SetProjectDefaults {
                    defaults,
                    designators,
                    typing,
                }
            }
            None => ProjectGeneralCommand::SetProjectDefaults {
                defaults,
                designators,
                typing,
            },
        };
        let rules_before = self.rules_of(project_id);
        self.apply_project_general(project_id, command);
        if self.rules_of(project_id) != rules_before {
            self.invalidate_pending_profile_update();
        }
        iced::Task::none()
    }

    /// The section shows `project_id`'s stored values again.
    fn drop_setup_draft(&mut self, project_id: ProjectId) {
        if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
            tab.setup_draft = None;
        }
    }

    /// `project_id`'s stored rules, `None` when it stores none or is closed.
    pub(crate) fn rules_of(
        &self,
        project_id: ProjectId,
    ) -> Option<signex_types::project::ProjectRules> {
        self.document_state
            .project_by_id(project_id)
            .and_then(|p| p.data.rules.clone())
    }
}
