//! Editable built-ins and the saved-profile edits (#2536): an edited
//! built-in tier is kept in the user's profile list as an override with the
//! tier's own id and the `edited` mark; Reset to default drops it. Saved
//! profiles gain rename and duplicate. Projects keep the values they copied;
//! nothing here re-applies a profile to a project. Rules:
//! `docs/PROJECT_SETUP.md`, "Editing a built-in profile".

use super::setup::{PcbDefaults, SheetDefaults};
use super::setup_profile::SetupProfile;
use super::user_profile::{ProfileNameProblem, UserProfile, add_user_profile, check_profile_name};
use crate::designator::DesignatorStandard;

/// The user's override of `profile`, if it is an edited tier.
pub fn builtin_override(profile: SetupProfile, users: &[UserProfile]) -> Option<&UserProfile> {
    profile.fab_template()?;
    users.iter().find(|p| p.edited && p.id == profile.token())
}

/// The profiles the user saved under a name, in order: the list without
/// the edited built-ins.
pub fn saved_profiles(users: &[UserProfile]) -> impl Iterator<Item = &UserProfile> {
    users.iter().filter(|p| p.is_user())
}

impl SetupProfile {
    /// The tier's rules as the user has them: the override's when edited,
    /// else the shipped table on the default layer count. Custom: the
    /// shipped Standard rules on its default count.
    pub fn values(self, users: &[UserProfile]) -> PcbDefaults {
        builtin_override(self, users).map_or_else(
            || self.rules_on(self.default_layers()),
            |edit| edit.pcb.clone(),
        )
    }

    /// Whether the user edited this built-in.
    pub fn is_edited(self, users: &[UserProfile]) -> bool {
        builtin_override(self, users).is_some()
    }
}

/// A change to the profile list, and how to take it back.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileChange {
    /// The entry as now stored.
    pub profile: UserProfile,
    /// The entry of the same id it replaced, if any.
    pub previous: Option<UserProfile>,
}

/// Store `pcb` as the user's edit of the tier `profile`, under the tier's
/// id with the `edited` mark; `name` is the tier's display name. `None` for
/// Custom, which has no shipped values to edit.
pub fn override_builtin(
    users: &mut Vec<UserProfile>,
    profile: SetupProfile,
    name: &str,
    pcb: PcbDefaults,
) -> Option<ProfileChange> {
    profile.fab_template()?;
    let entry = UserProfile {
        id: profile.token().to_string(),
        name: name.trim().to_string(),
        pcb,
        sheet: SheetDefaults::default(),
        designators: super::setup::DEFAULT_DESIGNATORS,
        edited: true,
    };
    let previous = match users.iter().position(|p| p.edited && p.id == entry.id) {
        Some(at) => Some(std::mem::replace(&mut users[at], entry.clone())),
        None => {
            users.push(entry.clone());
            None
        }
    };
    Some(ProfileChange {
        profile: entry,
        previous,
    })
}

/// Reset to default: drop the user's edit of `profile`. Returns the dropped
/// override and where it was, `None` when the tier was not edited.
pub fn reset_builtin(
    users: &mut Vec<UserProfile>,
    profile: SetupProfile,
) -> Option<(UserProfile, usize)> {
    let at = users
        .iter()
        .position(|p| p.edited && p.id == profile.token())?;
    Some((users.remove(at), at))
}

/// Rename the saved profile `id` to `name`. Refused when the name is empty,
/// too long, or another saved profile has it (ignoring case); `Ok(None)`
/// when `id` is not a saved profile.
pub fn rename_user_profile(
    users: &mut [UserProfile],
    id: &str,
    name: &str,
) -> Result<Option<ProfileChange>, ProfileNameProblem> {
    let name = check_profile_name(name)?;
    if users
        .iter()
        .any(|p| p.is_user() && p.id != id && p.name.trim().eq_ignore_ascii_case(name))
    {
        return Err(ProfileNameProblem::Taken);
    }
    let Some(entry) = users.iter_mut().find(|p| p.is_user() && p.id == id) else {
        return Ok(None);
    };
    let previous = entry.clone();
    entry.name = name.to_string();
    Ok(Some(ProfileChange {
        profile: entry.clone(),
        previous: Some(previous),
    }))
}

/// `base`, or `base 2`, `base 3`, ...: the first name no saved profile has
/// (ignoring case).
pub fn unique_profile_name(base: &str, users: &[UserProfile]) -> String {
    let base = base.trim();
    let taken =
        |name: &str| saved_profiles(users).any(|p| p.name.trim().eq_ignore_ascii_case(name));
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|name| !taken(name))
        .unwrap_or_else(|| base.to_string())
}

/// Duplicate: a new saved profile with these values under the first free
/// name from `base`, and a fresh `user-` id.
pub fn duplicate_profile(
    users: &mut Vec<UserProfile>,
    base: &str,
    pcb: PcbDefaults,
    sheet: SheetDefaults,
    designators: DesignatorStandard,
) -> UserProfile {
    let name = unique_profile_name(base, users);
    add_user_profile(users, &name, pcb, sheet, designators)
}
