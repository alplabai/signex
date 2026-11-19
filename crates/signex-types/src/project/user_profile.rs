//! User setup profiles (#2232, maintainer decision 2026-11-12): named
//! profiles the user saves from a setup dialog. They live in the user's
//! settings, not in a project, and are offered next to the built-ins in every
//! project; a project stores only the id of its default profile
//! (`project_defaults.profile`). The same list holds the user's edits of a
//! built-in tier (#2536, `profile_override.rs`). Format:
//! `docs/PROJECT_SETUP.md`.

use serde::{Deserialize, Serialize};

use super::setup::{DEFAULT_DESIGNATORS, PcbDefaults, SheetDefaults};
use super::setup_profile::SetupProfile;
use crate::designator::DesignatorStandard;

/// Every user profile id starts with this, so it never collides with a
/// built-in token.
pub const USER_PROFILE_ID_PREFIX: &str = "user-";

/// The profile token that means "the project's own defaults" in a command.
pub const PROJECT_PROFILE_TOKEN: &str = "project";

/// Longest a profile name may be, in characters.
pub const MAX_PROFILE_NAME_CHARS: usize = 60;

fn default_designators() -> DesignatorStandard {
    DEFAULT_DESIGNATORS
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// A named profile the user saved: board rules, sheet defaults and the
/// designator standard. With `edited` set it is the user's edit of the
/// built-in tier whose token is its id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: String,
    pub name: String,
    pub pcb: PcbDefaults,
    #[serde(default)]
    pub sheet: SheetDefaults,
    #[serde(default = "default_designators")]
    pub designators: DesignatorStandard,
    /// An edited built-in: the override of the tier `id` names.
    #[serde(default, skip_serializing_if = "is_false")]
    pub edited: bool,
}

impl UserProfile {
    /// Whether the profile can be offered: an id of the user form (or, for
    /// an edited built-in, a tier token), a name, and board rules the
    /// loader accepts.
    pub fn is_usable(&self) -> bool {
        let id_fits = if self.edited {
            SetupProfile::from_token(&self.id).is_some_and(|p| p.fab_template().is_some())
        } else {
            self.id.starts_with(USER_PROFILE_ID_PREFIX)
        };
        id_fits && !self.name.trim().is_empty() && self.pcb.problems().is_empty()
    }

    /// A profile the user saved under a name, not an edited built-in.
    pub fn is_user(&self) -> bool {
        !self.edited && self.id.starts_with(USER_PROFILE_ID_PREFIX)
    }
}

/// A profile an id names: built in (as shipped, or with the user's edits),
/// or one of the user's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProfileRef<'a> {
    BuiltIn(SetupProfile),
    /// A built-in tier the user edited: `values` is the override.
    Edited {
        profile: SetupProfile,
        values: &'a UserProfile,
    },
    User(&'a UserProfile),
}

/// The profile `id` names among the built-ins and `users`, if any. An
/// edited built-in resolves to its override.
pub fn resolve_profile<'a>(id: &str, users: &'a [UserProfile]) -> Option<ProfileRef<'a>> {
    if let Some(builtin) = SetupProfile::from_token(id) {
        return Some(match super::builtin_override(builtin, users) {
            Some(values) => ProfileRef::Edited {
                profile: builtin,
                values,
            },
            None => ProfileRef::BuiltIn(builtin),
        });
    }
    users
        .iter()
        .find(|p| p.is_user() && p.id == id)
        .map(ProfileRef::User)
}

/// The user profile a command names by id or, failing that, by name
/// (case-insensitive). Edited built-ins are not user profiles.
pub fn find_user_profile<'a>(key: &str, users: &'a [UserProfile]) -> Option<&'a UserProfile> {
    let key = key.trim();
    let mut saved = users.iter().filter(|p| p.is_user());
    saved
        .clone()
        .find(|p| p.id == key)
        .or_else(|| saved.find(|p| p.name.trim().eq_ignore_ascii_case(key)))
}

/// Why a profile name is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileNameProblem {
    Empty,
    TooLong,
    /// Another saved profile has this name (a rename).
    Taken,
}

/// Check a profile name as typed; the trimmed name when usable.
pub fn check_profile_name(name: &str) -> Result<&str, ProfileNameProblem> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ProfileNameProblem::Empty);
    }
    if name.chars().count() > MAX_PROFILE_NAME_CHARS {
        return Err(ProfileNameProblem::TooLong);
    }
    Ok(name)
}

/// A fresh id for a profile called `name`: the prefix and a slug of the
/// name, numbered when the slug is taken.
pub fn new_profile_id(name: &str, users: &[UserProfile]) -> String {
    let mut slug = String::new();
    for c in name.trim().chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    let base = format!(
        "{USER_PROFILE_ID_PREFIX}{}",
        if slug.is_empty() { "profile" } else { slug }
    );
    let taken = |id: &str| users.iter().any(|p| p.id == id);
    if !taken(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|id| !taken(id))
        .unwrap_or(base)
}

/// Save `profile` into `users`: a saved profile with the same name (ignoring
/// case) takes the new values and keeps its id; otherwise it is added at the
/// end. Returns the id it is stored under.
pub fn save_user_profile(
    users: &mut Vec<UserProfile>,
    name: &str,
    pcb: PcbDefaults,
    sheet: SheetDefaults,
    designators: DesignatorStandard,
) -> String {
    if let Some(existing) = users
        .iter_mut()
        .find(|p| p.is_user() && p.name.trim().eq_ignore_ascii_case(name.trim()))
    {
        existing.name = name.trim().to_string();
        existing.pcb = pcb;
        existing.sheet = sheet;
        existing.designators = designators;
        return existing.id.clone();
    }
    add_user_profile(users, name, pcb, sheet, designators).id
}

/// Add a new saved profile called `name` with a fresh id, whatever names
/// the list holds; returns it.
pub fn add_user_profile(
    users: &mut Vec<UserProfile>,
    name: &str,
    pcb: PcbDefaults,
    sheet: SheetDefaults,
    designators: DesignatorStandard,
) -> UserProfile {
    let profile = UserProfile {
        id: new_profile_id(name, users),
        name: name.trim().to_string(),
        pcb,
        sheet,
        designators,
        edited: false,
    };
    users.push(profile.clone());
    profile
}

/// Read the user profiles from their settings value: the usable entries, in
/// order, the first of each id kept. A value of another shape reads as none.
pub fn user_profiles_from_value(value: Option<&serde_json::Value>) -> Vec<UserProfile> {
    let Some(entries) = value.and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    let mut out: Vec<UserProfile> = Vec::new();
    for entry in entries {
        let Ok(profile) = serde_json::from_value::<UserProfile>(entry.clone()) else {
            continue;
        };
        if profile.is_usable() && !out.iter().any(|p| p.id == profile.id) {
            out.push(profile);
        }
    }
    out
}
