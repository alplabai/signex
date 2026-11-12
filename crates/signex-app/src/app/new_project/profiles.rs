//! Save as profile and Remove (#2232, 2026-11-12): the two setup edits that
//! change the user's profile list, shared by the New Project, New PCB and
//! New Schematic dialogs and the Project tab. The list lives in
//! `UiState::setup_profiles`; each edit is merged into `prefs.json` by id off
//! the update thread, so entries this build cannot read are kept.

use iced::Task;
use signex_types::project::{UserProfile, check_profile_name, save_user_profile};

use super::{NewProjectMsg, NewProjectState, SaveProfileProblem};
use crate::app::{Message, ProjectMsg, Signex};
use crate::fonts::SetupProfileEdit;

/// What a profile store did to the user's list.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ProfileStored {
    /// The message is not a profile store; the caller handles it.
    NotAStore,
    /// Refused or a no-op; nothing to write.
    Unchanged,
    Saved {
        profile: UserProfile,
    },
    Removed {
        profile: UserProfile,
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
            ProfileStored::Saved { profile }
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
            ProfileStored::Removed { profile }
        }
        _ => ProfileStored::NotAStore,
    }
}

impl Signex {
    /// After a profile store: merge the edit into `prefs.json` on the
    /// blocking pool; Messages says what happened once the write landed.
    pub(crate) fn after_profile_store(&mut self, stored: ProfileStored) -> Task<Message> {
        use signex_i18n::{ids, t};
        let i18n = &self.ui_state.i18n;
        let (note, edit) = match stored {
            ProfileStored::NotAStore | ProfileStored::Unchanged => return Task::none(),
            ProfileStored::Saved { profile } => (
                t!(i18n, ids::SETUP_PROFILE_SAVED, "name" => profile.name.as_str()),
                SetupProfileEdit::Upsert(profile),
            ),
            ProfileStored::Removed { profile } => {
                let note = t!(i18n, ids::SETUP_PROFILE_REMOVED, "name" => profile.name.as_str());
                self.ui_state
                    .removed_profile_names
                    .insert(profile.id.clone(), profile.name);
                (note, SetupProfileEdit::Remove(profile.id))
            }
        };
        let note = note.into_string();
        crate::fonts::background_pref_merge(
            crate::fonts::SETUP_PROFILES_KEY,
            move || crate::fonts::apply_setup_profile_edit(&edit),
            move |result| {
                Message::Project(ProjectMsg::SetupProfilesWritten {
                    note: note.clone(),
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

    /// The merge of a profile edit landed: say it, or why it was not saved.
    pub(crate) fn setup_profiles_written(&self, note: String, result: Result<(), String>) {
        use signex_i18n::{ids, t};
        match result {
            Ok(()) => crate::diagnostics::log_info(note),
            Err(why) => crate::diagnostics::log_warning(
                t!(
                    self.ui_state.i18n,
                    ids::SETUP_PROFILE_WRITE_FAILED,
                    "why" => why
                )
                .into_string(),
            ),
        }
    }
}
