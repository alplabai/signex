//! The profile-list edits the cards' pencil and menus make (#2536): Edit
//! profile's Save, Reset to default, Duplicate and Rename. Each changes the
//! user's list in memory and returns the change; `after_profile_store`
//! writes it to `prefs.json` off the update thread and takes it back if the
//! write fails. Rules: `docs/PROJECT_SETUP.md`, "Editing a built-in profile".

use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::designator::DesignatorStandard;
use signex_types::project::{
    PcbDefaults, ProfileChange, SetupProfile, SheetDefaults, UserProfile, duplicate_profile,
    override_builtin, rename_user_profile, reset_builtin,
};

use super::profiles::{ProfileStored, StoreNote};
use super::{NewProjectMsg, NewProjectState, ProfileTarget, SaveProfileProblem};

/// The catalog name of a built-in card.
pub(crate) const fn profile_name_id(profile: SetupProfile) -> MessageId {
    match profile {
        SetupProfile::Standard => ids::DIALOG_NEW_PROJECT_PROFILE_STANDARD_NAME,
        SetupProfile::Advanced => ids::DIALOG_NEW_PROJECT_PROFILE_ADVANCED_NAME,
        SetupProfile::Hdi => ids::DIALOG_NEW_PROJECT_PROFILE_HDI_NAME,
        SetupProfile::Custom => ids::DIALOG_NEW_PROJECT_PROFILE_CUSTOM_NAME,
    }
}

fn saved(change: ProfileChange, note: StoreNote) -> ProfileStored {
    ProfileStored::Saved {
        profile: change.profile,
        previous: change.previous,
        note,
    }
}

/// The values `source` would duplicate: a built-in's as the user has them,
/// a saved profile's own.
fn source_values(
    source: &ProfileTarget,
    users: &[UserProfile],
) -> Option<(PcbDefaults, SheetDefaults, DesignatorStandard)> {
    match source {
        ProfileTarget::BuiltIn(profile) => Some((
            profile.values(users),
            SheetDefaults::default(),
            signex_types::project::DEFAULT_DESIGNATORS,
        )),
        ProfileTarget::Saved(id) => users
            .iter()
            .find(|p| p.is_user() && p.id == *id)
            .map(|p| (p.pcb.clone(), p.sheet.clone(), p.designators)),
    }
}

/// Edit profile's Save: the values shown become the profile. A built-in's
/// are kept as its override (same id, edited); a saved profile keeps its id
/// and name. Bad values keep the edit open with the reason.
fn save_edit(draft: &mut NewProjectState, users: &mut Vec<UserProfile>) -> ProfileStored {
    let Some(target) = draft.editing.clone() else {
        return ProfileStored::Unchanged;
    };
    if !draft.rules_valid() || !draft.setup.pcb.problems().is_empty() {
        draft.save_profile_problem = Some(SaveProfileProblem::Rules);
        return ProfileStored::Unchanged;
    }
    draft.editing = None;
    draft.save_profile_problem = None;
    let setup = &draft.setup;
    match target {
        ProfileTarget::BuiltIn(profile) => {
            // The stored name is the tier's id: built-ins show their catalog
            // name, never this one.
            match override_builtin(users, profile, profile.token(), setup.pcb.clone()) {
                Some(change) => saved(change, StoreNote::Edited),
                None => ProfileStored::Unchanged,
            }
        }
        ProfileTarget::Saved(id) => {
            let Some(entry) = users.iter_mut().find(|p| p.is_user() && p.id == id) else {
                return ProfileStored::Unchanged;
            };
            let previous = entry.clone();
            entry.pcb = setup.pcb.clone();
            entry.sheet = setup.sheet.clone();
            entry.designators = setup.designators;
            let profile = entry.clone();
            draft.setup.user_profile = Some(id);
            saved(
                ProfileChange {
                    profile,
                    previous: Some(previous),
                },
                StoreNote::Saved,
            )
        }
    }
}

/// Apply a profile-list edit of the cards to `users`; `NotAStore` for any
/// other message.
pub(crate) fn store_card_edit(
    draft: &mut NewProjectState,
    users: &mut Vec<UserProfile>,
    msg: &NewProjectMsg,
) -> ProfileStored {
    match msg {
        NewProjectMsg::ProfileEditSaved => save_edit(draft, users),
        NewProjectMsg::ProfileResetRequested(profile) => {
            draft.profile_menu = None;
            let Some((dropped, at)) = reset_builtin(users, *profile) else {
                return ProfileStored::Unchanged;
            };
            if draft.editing == Some(ProfileTarget::BuiltIn(*profile)) {
                draft.editing = None;
            }
            // The picked card shows the shipped values again.
            if draft.setup.is_builtin_picked(*profile) {
                draft.pick_profile(*profile, users);
            }
            ProfileStored::Removed {
                profile: dropped,
                at,
            }
        }
        NewProjectMsg::ProfileDuplicateRequested { source, name } => {
            draft.profile_menu = None;
            let Some((pcb, sheet, designators)) = source_values(source, users) else {
                return ProfileStored::Unchanged;
            };
            let profile = duplicate_profile(users, name, pcb, sheet, designators);
            saved(
                ProfileChange {
                    profile,
                    previous: None,
                },
                StoreNote::Duplicated,
            )
        }
        NewProjectMsg::ProfileRenameConfirmed => {
            let Some(mut renaming) = draft.renaming.take() else {
                return ProfileStored::Unchanged;
            };
            match rename_user_profile(users, &renaming.id, &renaming.name) {
                Ok(Some(change)) => saved(change, StoreNote::Renamed),
                Ok(None) => ProfileStored::Unchanged,
                Err(problem) => {
                    renaming.problem = Some(problem);
                    draft.renaming = Some(renaming);
                    ProfileStored::Unchanged
                }
            }
        }
        _ => ProfileStored::NotAStore,
    }
}

/// The Messages line of a stored profile change.
pub(super) fn saved_note(
    i18n: &I18n,
    note: StoreNote,
    profile: &UserProfile,
    previous: Option<&UserProfile>,
) -> Localized {
    let name = profile.name.as_str();
    match note {
        StoreNote::Saved => t!(i18n, ids::SETUP_PROFILE_SAVED, "name" => name),
        StoreNote::Duplicated => t!(i18n, ids::SETUP_PROFILE_DUPLICATED, "name" => name),
        StoreNote::Renamed => t!(
            i18n,
            ids::SETUP_PROFILE_RENAMED,
            "old" => previous.map_or(name, |p| p.name.as_str()),
            "name" => name
        ),
        StoreNote::Edited => t!(
            i18n,
            ids::SETUP_PROFILE_EDITED,
            "name" => builtin_name(i18n, profile).into_string()
        ),
    }
}

/// The Messages line of Reset to default.
pub(super) fn reset_note(i18n: &I18n, profile: &UserProfile) -> Localized {
    t!(
        i18n,
        ids::SETUP_PROFILE_RESET,
        "name" => builtin_name(i18n, profile).into_string()
    )
}

/// The catalog name of the built-in an override edits.
fn builtin_name(i18n: &I18n, profile: &UserProfile) -> Localized {
    let tier = SetupProfile::from_token(&profile.id).unwrap_or(SetupProfile::Standard);
    t!(i18n, profile_name_id(tier))
}
