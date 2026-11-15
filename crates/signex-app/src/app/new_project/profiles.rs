//! Save as profile and Remove (#2232, 2026-11-12): the two setup edits that
//! change the user's profile list, shared by the New Project, New PCB and
//! New Schematic dialogs and the Project tab. The list lives in
//! `UiState::setup_profiles`; each edit is merged into `prefs.json` by id off
//! the update thread, in the order made, so entries this build cannot read
//! are kept. A write that fails undoes the in-memory change.

use iced::Task;
use signex_engine::ProjectGeneralCommand;
use signex_types::project::{UserProfile, check_profile_name, save_user_profile};

use super::{NewProjectMsg, NewProjectState, SaveProfileProblem};
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, Signex};
use crate::fonts::SetupProfileEdit;

/// What a profile store did to the user's list.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ProfileStored {
    /// The message is not a profile store; the caller handles it.
    NotAStore,
    /// Refused or a no-op; nothing to write.
    Unchanged,
    /// `previous`: the profile of the same id it replaced, if any.
    Saved {
        profile: UserProfile,
        previous: Option<UserProfile>,
    },
    /// `at`: where it was in the list.
    Removed { profile: UserProfile, at: usize },
}

/// How to take an in-memory profile change back when its write failed.
#[derive(Debug, Clone, PartialEq)]
pub enum ProfileUndo {
    Saved {
        id: String,
        previous: Option<UserProfile>,
    },
    Removed {
        profile: UserProfile,
        at: usize,
    },
}

/// Apply Save as profile (`SaveProfileConfirmed`) or a confirmed Remove
/// (`UserProfileRemoveConfirmed`) to `users`, with the draft's values. A
/// refused save keeps the name field open with its reason. A removed profile
/// that was picked leaves its values in the draft, now on the Custom card.
pub(crate) fn store_profile(
    draft: &mut NewProjectState,
    users: &mut Vec<UserProfile>,
    msg: &NewProjectMsg,
) -> ProfileStored {
    match msg {
        NewProjectMsg::SaveProfileConfirmed => {
            let typed = draft.save_profile_name.clone().unwrap_or_default();
            let name = match check_profile_name(&typed) {
                Ok(name) => name.to_string(),
                Err(problem) => {
                    draft.save_profile_problem = Some(SaveProfileProblem::Name(problem));
                    return ProfileStored::Unchanged;
                }
            };
            if !draft.rules_valid() || !draft.setup.pcb.problems().is_empty() {
                draft.save_profile_problem = Some(SaveProfileProblem::Rules);
                return ProfileStored::Unchanged;
            }
            let previous = users
                .iter()
                .find(|p| p.name.trim().eq_ignore_ascii_case(&name))
                .cloned();
            let setup = &draft.setup;
            let id = save_user_profile(
                users,
                &name,
                setup.pcb.clone(),
                setup.sheet.clone(),
                setup.designators,
            );
            let Some(profile) = users.iter().find(|p| p.id == id).cloned() else {
                return ProfileStored::Unchanged;
            };
            draft.setup.user_profile = Some(id);
            draft.save_profile_name = None;
            draft.save_profile_problem = None;
            ProfileStored::Saved { profile, previous }
        }
        NewProjectMsg::UserProfileRemoveConfirmed => {
            let Some(id) = draft.remove_confirm.take() else {
                return ProfileStored::Unchanged;
            };
            let Some(at) = users.iter().position(|p| p.id == id) else {
                return ProfileStored::Unchanged;
            };
            let profile = users.remove(at);
            if draft.setup.user_profile.as_deref() == Some(id.as_str()) {
                draft.setup.user_profile = None;
            }
            ProfileStored::Removed { profile, at }
        }
        _ => ProfileStored::NotAStore,
    }
}

impl Signex {
    /// A Remove of `removed` from a surface of `project`: when it is the
    /// project's own default profile, the project stops naming it, as one
    /// undoable project edit.
    pub(crate) fn clear_removed_default_profile(&mut self, project: ProjectId, removed: &str) {
        let Some(loaded) = self.document_state.project_by_id(project) else {
            return;
        };
        if loaded.data.project_defaults.profile.as_deref() != Some(removed) {
            return;
        }
        let mut defaults = loaded.data.project_defaults.clone();
        defaults.profile = None;
        self.apply_project_general(
            project,
            ProjectGeneralCommand::SetProjectDefaults {
                defaults,
                designators: None,
                typing: None,
            },
        );
    }

    /// After a profile store: merge the edit into `prefs.json` on the
    /// blocking pool, in order; Messages says what happened once it landed.
    pub(crate) fn after_profile_store(&mut self, stored: ProfileStored) -> Task<Message> {
        use signex_i18n::{ids, t};
        let i18n = &self.ui_state.i18n;
        let (note, edit, undo) = match stored {
            ProfileStored::NotAStore | ProfileStored::Unchanged => return Task::none(),
            ProfileStored::Saved { profile, previous } => (
                t!(i18n, ids::SETUP_PROFILE_SAVED, "name" => profile.name.as_str()),
                SetupProfileEdit::Upsert {
                    resaved: previous.is_some(),
                    profile: profile.clone(),
                },
                ProfileUndo::Saved {
                    id: profile.id,
                    previous,
                },
            ),
            ProfileStored::Removed { profile, at } => {
                let note = t!(i18n, ids::SETUP_PROFILE_REMOVED, "name" => profile.name.as_str());
                self.ui_state
                    .removed_profile_names
                    .insert(profile.id.clone(), profile.name.clone());
                (
                    note,
                    SetupProfileEdit::Remove(profile.id.clone()),
                    ProfileUndo::Removed { profile, at },
                )
            }
        };
        let note = note.into_string();
        crate::fonts::background_pref_merge(
            crate::fonts::SETUP_PROFILES_KEY,
            move || crate::fonts::apply_setup_profile_edit(&edit),
            move |result| {
                Message::Project(ProjectMsg::SetupProfilesWritten {
                    note: note.clone(),
                    undo: Box::new(undo.clone()),
                    result,
                })
            },
        )
    }

    /// At start-up: say once that `count` stored profiles are not offered.
    /// They stay in the settings file untouched.
    pub(crate) fn note_unusable_setup_profiles(&self, count: usize) {
        if count == 0 {
            return;
        }
        crate::diagnostics::log_warning(
            signex_i18n::t!(
                self.ui_state.i18n,
                signex_i18n::ids::SETUP_PROFILES_SKIPPED,
                "count" => count as u32
            )
            .into_string(),
        );
    }

    /// The merge of a profile edit landed. On success: a new profile the
    /// file numbered past an id it already held takes that id here too, and
    /// Messages says what was saved. On failure: the in-memory change is
    /// taken back and Messages says why.
    pub(crate) fn setup_profiles_written(
        &mut self,
        note: String,
        undo: ProfileUndo,
        result: Result<Option<String>, String>,
    ) {
        use signex_i18n::{ids, t};
        match result {
            Ok(stored_id) => {
                if let (ProfileUndo::Saved { id, .. }, Some(stored)) = (&undo, stored_id)
                    && *id != stored
                {
                    self.rename_profile_id(id, &stored);
                }
                crate::diagnostics::log_info(note);
            }
            Err(why) => {
                self.undo_profile_change(undo);
                crate::diagnostics::log_warning(
                    t!(
                        self.ui_state.i18n,
                        ids::SETUP_PROFILE_WRITE_FAILED,
                        "why" => why
                    )
                    .into_string(),
                );
            }
        }
    }

    fn undo_profile_change(&mut self, undo: ProfileUndo) {
        let users = &mut self.ui_state.setup_profiles;
        match undo {
            ProfileUndo::Saved { id, previous } => {
                match (users.iter().position(|p| p.id == id), previous) {
                    (Some(at), Some(previous)) => users[at] = previous,
                    (Some(at), None) => {
                        users.remove(at);
                        self.for_each_draft(|draft| {
                            if draft.setup.user_profile.as_deref() == Some(id.as_str()) {
                                draft.setup.user_profile = None;
                            }
                        });
                    }
                    (None, _) => {}
                }
            }
            ProfileUndo::Removed { profile, at } => {
                if !users.iter().any(|p| p.id == profile.id) {
                    let at = at.min(users.len());
                    users.insert(at, profile);
                }
            }
        }
    }

    fn rename_profile_id(&mut self, from: &str, to: &str) {
        if let Some(profile) = self
            .ui_state
            .setup_profiles
            .iter_mut()
            .find(|p| p.id == from)
        {
            profile.id = to.to_string();
        }
        self.for_each_draft(|draft| {
            if draft.setup.user_profile.as_deref() == Some(from) {
                draft.setup.user_profile = Some(to.to_string());
            }
        });
    }

    /// Every open setup draft: New Project, New PCB / New Schematic, and the
    /// Project tabs' setup sections.
    fn for_each_draft(&mut self, mut edit: impl FnMut(&mut NewProjectState)) {
        if let Some(draft) = self.ui_state.new_project.as_mut() {
            edit(draft);
        }
        if let Some(state) = self.ui_state.document_setup.as_mut() {
            edit(&mut state.draft);
        }
        for tab in self.document_state.project_tabs.values_mut() {
            if let Some(draft) = tab.setup_draft.as_mut() {
                edit(draft);
            }
        }
    }
}
