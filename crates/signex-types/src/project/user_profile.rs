//! User setup profiles (#2232, maintainer decision 2026-11-12): named
//! profiles the user saves from a setup dialog. They live in the user's
//! settings, not in a project, and are offered next to the built-ins in every
//! project; a project stores only the id of its default profile
//! (`project_defaults.profile`). Format: `docs/PROJECT_SETUP.md`.

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

/// A named profile the user saved: board rules, sheet defaults and the
/// designator standard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: String,
    pub name: String,
    pub pcb: PcbDefaults,
    #[serde(default)]
    pub sheet: SheetDefaults,
    #[serde(default = "default_designators")]
    pub designators: DesignatorStandard,
}

impl UserProfile {
    /// Whether the profile can be offered: an id of the user form, a name,
    /// and board rules the loader accepts.
    pub fn is_usable(&self) -> bool {
        self.id.starts_with(USER_PROFILE_ID_PREFIX)
            && !self.name.trim().is_empty()
            && self.pcb.problems().is_empty()
    }
}

/// A profile an id names: built in, or one of the user's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProfileRef<'a> {
    BuiltIn(SetupProfile),
    User(&'a UserProfile),
}

/// The profile `id` names among the built-ins and `users`, if any.
pub fn resolve_profile<'a>(id: &str, users: &'a [UserProfile]) -> Option<ProfileRef<'a>> {
    if let Some(builtin) = SetupProfile::from_token(id) {
        return Some(ProfileRef::BuiltIn(builtin));
    }
    users.iter().find(|p| p.id == id).map(ProfileRef::User)
}

/// The user profile a command names by id or, failing that, by name
/// (case-insensitive).
pub fn find_user_profile<'a>(key: &str, users: &'a [UserProfile]) -> Option<&'a UserProfile> {
    let key = key.trim();
    users.iter().find(|p| p.id == key).or_else(|| {
        users
            .iter()
            .find(|p| p.name.trim().eq_ignore_ascii_case(key))
    })
}

/// Why a profile name is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileNameProblem {
    Empty,
    TooLong,
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

/// Save `profile` into `users`: a profile with the same name (ignoring case)
/// takes the new values and keeps its id; otherwise it is added at the end.
/// Returns the id it is stored under.
pub fn save_user_profile(
    users: &mut Vec<UserProfile>,
    name: &str,
    pcb: PcbDefaults,
    sheet: SheetDefaults,
    designators: DesignatorStandard,
) -> String {
    if let Some(existing) = users
        .iter_mut()
        .find(|p| p.name.trim().eq_ignore_ascii_case(name.trim()))
    {
        existing.name = name.trim().to_string();
        existing.pcb = pcb;
        existing.sheet = sheet;
        existing.designators = designators;
        return existing.id.clone();
    }
    let id = new_profile_id(name, users);
    users.push(UserProfile {
        id: id.clone(),
        name: name.trim().to_string(),
        pcb,
        sheet,
        designators,
    });
    id
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
