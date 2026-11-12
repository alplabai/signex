//! Save as profile and Remove (#2232, 2026-11-12): the two setup edits that
//! change the user's profile list, shared by the New Project, New PCB and
//! New Schematic dialogs and the Project tab. The list lives in
//! `UiState::setup_profiles` and is written to `prefs.json` off the update
//! thread.

use iced::Task;
use signex_types::project::{UserProfile, check_profile_name, save_user_profile};

use super::{NewProjectMsg, NewProjectState};
use crate::app::{Message, Signex};

/// What a profile store did to the user's list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProfileStored {
    /// The message is not a profile store; the caller handles it.
    NotAStore,
    /// Refused or a no-op; nothing to write.
    Unchanged,
    Saved {
        name: String,
    },
    Removed {
        name: String,
    },
}

/// Apply Save as profile (`SaveProfileConfirmed`) or Remove
/// (`UserProfileRemoved`) to `users`, with the draft's values. A refused name
/// stays in the field with its reason. A removed profile that was picked
/// leaves its values in the draft, now on the Custom card.
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
                    draft.save_profile_problem = Some(problem);
                    return ProfileStored::Unchanged;
                }
            };
            if !draft.rules_valid() || !draft.setup.pcb.problems().is_empty() {
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
            draft.setup.user_profile = Some(id);
            draft.save_profile_name = None;
            draft.save_profile_problem = None;
            ProfileStored::Saved { name }
        }
        NewProjectMsg::UserProfileRemoved(id) => {
            let Some(at) = users.iter().position(|p| p.id == *id) else {
                return ProfileStored::Unchanged;
            };
            let removed = users.remove(at);
            if draft.setup.user_profile.as_deref() == Some(id.as_str()) {
                draft.setup.user_profile = None;
            }
            ProfileStored::Removed { name: removed.name }
        }
        _ => ProfileStored::NotAStore,
    }
}

impl Signex {
    /// After a profile store: say what happened in Messages and write the
    /// list to `prefs.json` on the blocking pool.
    pub(crate) fn after_profile_store(&self, stored: ProfileStored) -> Task<Message> {
        use signex_i18n::{ids, t};
        let i18n = &self.ui_state.i18n;
        let note = match stored {
            ProfileStored::NotAStore | ProfileStored::Unchanged => return Task::none(),
            ProfileStored::Saved { name } => t!(i18n, ids::SETUP_PROFILE_SAVED, "name" => name),
            ProfileStored::Removed { name } => {
                t!(i18n, ids::SETUP_PROFILE_REMOVED, "name" => name)
            }
        };
        crate::diagnostics::log_info(note.into_string());
        self.persist_setup_profiles()
    }

    /// The `Task` that writes the user's setup profiles to `prefs.json`.
    pub(crate) fn persist_setup_profiles(&self) -> Task<Message> {
        let profiles = self.ui_state.setup_profiles.clone();
        crate::fonts::background_pref_write(crate::fonts::SETUP_PROFILES_KEY, move || {
            crate::fonts::write_setup_profiles(&profiles)
        })
    }
}
