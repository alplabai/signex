//! User setup profiles in `prefs.json` (#2232, 2026-11-12): the profiles a
//! user saves from a setup dialog, offered in every project.

use super::*;
use signex_types::project::{UserProfile, user_profiles_from_value};

/// The `prefs.json` key of the user setup profiles.
pub const SETUP_PROFILES_KEY: &str = "setup_profiles";

/// The user's setup profiles; prefs written before profiles existed (no key)
/// and a value of another shape read as none.
pub fn read_setup_profiles_from(json: Option<&serde_json::Value>) -> Vec<UserProfile> {
    user_profiles_from_value(json.and_then(|j| j.get(SETUP_PROFILES_KEY)))
}

#[cfg(test)]
fn read_setup_profiles_at(path: &Path) -> Vec<UserProfile> {
    read_setup_profiles_from(read_prefs_json(path).as_ref())
}

/// Write the profiles into `path`'s `setup_profiles` key, leaving every
/// other key as it is (read-modify-write, atomic).
pub(super) fn write_setup_profiles_at(path: &Path, profiles: &[UserProfile]) {
    update_prefs_json(
        path,
        SETUP_PROFILES_KEY,
        |prefs| match serde_json::to_value(profiles) {
            Ok(value) => {
                prefs.insert(SETUP_PROFILES_KEY.to_string(), value);
            }
            Err(error) => tracing::error!(
                target: "signex::prefs",
                error = %error,
                "the setup profiles could not be serialised; they were not saved"
            ),
        },
    )
}

/// Persist the user's setup profiles to `prefs.json`. Blocking: run it
/// through `background_pref_write`.
pub fn write_setup_profiles(profiles: &[UserProfile]) {
    write_setup_profiles_at(&prefs_path(), profiles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::designator::DesignatorStandard;
    use signex_types::project::{SetupProfile, SheetDefaults, save_user_profile};

    #[test]
    fn profiles_round_trip_beside_the_other_keys() {
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
        let mut profiles = Vec::new();
        save_user_profile(
            &mut profiles,
            "Team",
            SetupProfile::Hdi.pcb(8),
            SheetDefaults::default(),
            DesignatorStandard::Din40719_2,
        );

        write_setup_profiles_at(&path, &profiles);

        assert_eq!(read_setup_profiles_at(&path), profiles);
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("read")).expect("json");
        assert_eq!(json["ui_font"], "Roboto");
        assert_eq!(json["pinned_projects"], serde_json::json!(["a.snxprj"]));

        write_setup_profiles_at(&path, &[]);
        assert!(read_setup_profiles_at(&path).is_empty());
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("read")).expect("json");
        assert_eq!(json["ui_font"], "Roboto");
    }

    #[test]
    fn a_broken_prefs_file_is_left_alone() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, b"{ not json").expect("write");
        write_setup_profiles_at(&path, &[]);
        assert_eq!(std::fs::read(&path).expect("read"), b"{ not json");
    }
}
