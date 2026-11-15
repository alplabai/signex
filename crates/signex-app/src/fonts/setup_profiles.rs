//! User setup profiles in `prefs.json` (#2232, 2026-11-12): the profiles a
//! user saves from a setup dialog, offered in every project.
//!
//! A write is one edit (add or replace by id, or remove by id) merged into
//! the list the file holds now, never the whole in-memory list: an entry this
//! build cannot read, or one another running instance saved, is kept.

use super::*;
use signex_types::project::{UserProfile, user_profiles_from_value};

/// The `prefs.json` key of the user setup profiles.
pub const SETUP_PROFILES_KEY: &str = "setup_profiles";

/// The user's setup profiles; prefs written before profiles existed (no key)
/// and a value of another shape read as none.
pub fn read_setup_profiles_from(json: Option<&serde_json::Value>) -> Vec<UserProfile> {
    user_profiles_from_value(json.and_then(|j| j.get(SETUP_PROFILES_KEY)))
}

/// How many stored entries are not offered: unreadable, unusable or a
/// repeated id. They stay in the file.
pub fn unusable_setup_profiles(json: Option<&serde_json::Value>) -> usize {
    let stored = json
        .and_then(|j| j.get(SETUP_PROFILES_KEY))
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    stored.saturating_sub(read_setup_profiles_from(json).len())
}

#[cfg(test)]
fn read_setup_profiles_at(path: &Path) -> Vec<UserProfile> {
    read_setup_profiles_from(read_prefs_json(path).as_ref())
}

/// One change to the stored profiles.
#[derive(Debug, Clone, PartialEq)]
pub enum SetupProfileEdit {
    /// Save the profile. `resaved`: it is a profile this instance already
    /// stored under that id, so the stored entry with the id is replaced.
    /// Otherwise it is new: an id already in the file (an entry this build
    /// cannot read, or a profile another instance saved) is never replaced;
    /// the new profile is numbered past it.
    Upsert { profile: UserProfile, resaved: bool },
    /// Drop the stored entries with this id.
    Remove(String),
}

fn id_of(entry: &serde_json::Value) -> Option<&str> {
    entry.get("id").and_then(serde_json::Value::as_str)
}

/// `id`, or `id-2`, `id-3`, ... : the first that `list` does not hold.
fn free_id(id: &str, list: &[serde_json::Value]) -> String {
    let taken = |candidate: &str| list.iter().any(|e| id_of(e) == Some(candidate));
    if !taken(id) {
        return id.to_string();
    }
    (2..)
        .map(|n| format!("{id}-{n}"))
        .find(|candidate| !taken(candidate))
        .unwrap_or_else(|| id.to_string())
}

/// Merge `edit` into `path`'s `setup_profiles` list, leaving every other
/// entry and key as it is (read-modify-write, atomic). Returns the id the
/// profile is stored under (a new profile's may be numbered past one already
/// in the file; `None` for a removal). A file that cannot be loaded, or a
/// `setup_profiles` value that is not a list, is left untouched and the
/// reason returned.
fn apply_setup_profile_edit_at(
    path: &Path,
    edit: &SetupProfileEdit,
) -> Result<Option<String>, String> {
    let mut outcome: Result<Option<String>, String> = Ok(None);
    try_update_prefs_json(path, SETUP_PROFILES_KEY, |prefs| {
        let mut list = match prefs.get(SETUP_PROFILES_KEY) {
            None => Vec::new(),
            Some(serde_json::Value::Array(list)) => list.clone(),
            Some(_) => {
                outcome = Err(format!(
                    "the `{SETUP_PROFILES_KEY}` setting is not a list this version can \
                     extend; it was left as it is"
                ));
                return;
            }
        };
        match edit {
            SetupProfileEdit::Upsert { profile, resaved } => {
                let mut stored = profile.clone();
                let at = list
                    .iter()
                    .position(|e| id_of(e) == Some(profile.id.as_str()));
                match (at, *resaved) {
                    (Some(at), true) => {
                        let Ok(value) = serde_json::to_value(&stored) else {
                            outcome = Err("the profile could not be serialised".into());
                            return;
                        };
                        list[at] = value;
                    }
                    (Some(_), false) | (None, _) => {
                        stored.id = free_id(&profile.id, &list);
                        let Ok(value) = serde_json::to_value(&stored) else {
                            outcome = Err("the profile could not be serialised".into());
                            return;
                        };
                        list.push(value);
                    }
                }
                outcome = Ok(Some(stored.id));
            }
            SetupProfileEdit::Remove(id) => list.retain(|e| id_of(e) != Some(id.as_str())),
        }
        prefs.insert(
            SETUP_PROFILES_KEY.to_string(),
            serde_json::Value::Array(list),
        );
    })?;
    outcome
}

/// Merge `edit` into the user's `prefs.json`. Blocking: run it through
/// `background_pref_merge`.
pub fn apply_setup_profile_edit(edit: &SetupProfileEdit) -> Result<Option<String>, String> {
    apply_setup_profile_edit_at(&prefs_path(), edit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::designator::DesignatorStandard;
    use signex_types::project::{SetupProfile, SheetDefaults, save_user_profile};

    fn saved(list: &mut Vec<UserProfile>, name: &str) -> UserProfile {
        let id = save_user_profile(
            list,
            name,
            SetupProfile::Hdi.pcb(8),
            SheetDefaults::default(),
            DesignatorStandard::Din40719_2,
        );
        list.iter().find(|p| p.id == id).cloned().expect("saved")
    }

    fn json(path: &Path) -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(path).expect("read")).expect("json")
    }

    #[test]
    fn an_edit_keeps_the_other_keys_and_a_remove_drops_only_its_id() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("prefs.json");
        std::fs::write(
            &path,
            br#"{"ui_font": "Roboto", "pinned_projects": ["a.snxprj"]}"#,
        )
        .expect("write");
        assert!(
            read_setup_profiles_at(&path).is_empty(),
            "no key reads as none"
        );
        let mut list = Vec::new();
        let team = saved(&mut list, "Team");
        apply_setup_profile_edit_at(
            &path,
            &SetupProfileEdit::Upsert {
                profile: team.clone(),
                resaved: false,
            },
        )
        .expect("w");
        assert_eq!(read_setup_profiles_at(&path), std::slice::from_ref(&team));
        assert_eq!(json(&path)["ui_font"], "Roboto");
        assert_eq!(
            json(&path)["pinned_projects"],
            serde_json::json!(["a.snxprj"])
        );
        apply_setup_profile_edit_at(&path, &SetupProfileEdit::Remove(team.id)).expect("w");
        assert!(read_setup_profiles_at(&path).is_empty());
        assert_eq!(json(&path)["ui_font"], "Roboto");
    }

    #[test]
    fn an_entry_this_build_cannot_use_survives_a_save() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("prefs.json");
        let future = serde_json::json!({"id": "user-future", "kind": "newer shape"});
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({"setup_profiles": [future.clone()]})).unwrap(),
        )
        .expect("write");
        let prefs = read_prefs_json(&path);
        assert_eq!(unusable_setup_profiles(prefs.as_ref()), 1);
        let mut list = Vec::new();
        let mine = saved(&mut list, "Mine");
        apply_setup_profile_edit_at(
            &path,
            &SetupProfileEdit::Upsert {
                profile: mine.clone(),
                resaved: false,
            },
        )
        .expect("w");
        let stored = json(&path)["setup_profiles"].clone();
        assert_eq!(stored[0], future, "the unusable entry is kept as it was");
        assert_eq!(read_setup_profiles_at(&path), [mine]);
    }

    /// Two instances, each with its own in-memory list, both keep their save.
    #[test]
    fn saves_from_two_lists_both_persist() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("prefs.json");
        let (mut first, mut second) = (Vec::new(), Vec::new());
        let a = saved(&mut first, "Alpha");
        let b = saved(&mut second, "Beta");
        apply_setup_profile_edit_at(
            &path,
            &SetupProfileEdit::Upsert {
                profile: a.clone(),
                resaved: false,
            },
        )
        .expect("w");
        apply_setup_profile_edit_at(
            &path,
            &SetupProfileEdit::Upsert {
                profile: b.clone(),
                resaved: false,
            },
        )
        .expect("w");
        assert_eq!(read_setup_profiles_at(&path), [a, b]);
    }

    #[test]
    fn a_broken_prefs_file_is_left_alone_and_the_reason_returned() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, b"{ not json").expect("write");
        let result = apply_setup_profile_edit_at(&path, &SetupProfileEdit::Remove("x".into()));
        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).expect("read"), b"{ not json");
    }

    /// A new profile whose id is already in the file (an unreadable entry, or
    /// another instance's profile of the same name) is numbered past it and
    /// replaces nothing; a re-save replaces its own entry.
    #[test]
    fn a_new_profile_never_replaces_an_entry_with_its_id() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("prefs.json");
        let kept = serde_json::json!({"id": "user-team", "kind": "newer shape"});
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({"setup_profiles": [kept.clone()]})).unwrap(),
        )
        .expect("write");
        let mut list = Vec::new();
        let team = saved(&mut list, "Team");
        assert_eq!(team.id, "user-team");
        let edit = SetupProfileEdit::Upsert {
            profile: team.clone(),
            resaved: false,
        };
        let id = apply_setup_profile_edit_at(&path, &edit).expect("w");
        assert_eq!(id.as_deref(), Some("user-team-2"));
        assert_eq!(json(&path)["setup_profiles"][0], kept);
        let mut again = team.clone();
        again.id = "user-team-2".into();
        again.pcb.clearance_mm = 0.3;
        let edit = SetupProfileEdit::Upsert {
            profile: again.clone(),
            resaved: true,
        };
        assert_eq!(
            apply_setup_profile_edit_at(&path, &edit)
                .expect("w")
                .as_deref(),
            Some("user-team-2")
        );
        assert_eq!(read_setup_profiles_at(&path), [again]);
        assert_eq!(json(&path)["setup_profiles"][0], kept);
    }

    /// A `setup_profiles` value of another shape is refused, not replaced.
    #[test]
    fn a_setup_profiles_value_that_is_not_a_list_is_refused_and_kept() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("prefs.json");
        let odd = serde_json::json!({"v2": ["user-x"]});
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({"setup_profiles": odd.clone()})).unwrap(),
        )
        .expect("write");
        let mut list = Vec::new();
        let edit = SetupProfileEdit::Upsert {
            profile: saved(&mut list, "Mine"),
            resaved: false,
        };
        assert!(apply_setup_profile_edit_at(&path, &edit).is_err());
        assert_eq!(json(&path)["setup_profiles"], odd);
    }
}
