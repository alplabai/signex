//! Folding the machine's old `prefs.json` ERC rules into the `User Default`
//! profile (#1085, #1608).
//!
//! Until #1608 the live source of the machine's rules was `prefs.json`
//! (`erc_severity`, `pin_matrix`); the `User Default` profile a v0.15 build
//! made from them once was a copy that went stale as the preferences kept
//! changing. From now on the profile is the only store, so the preferences'
//! rules are folded into it once more: they win over what the profile holds
//! for every rule this build knows, and anything in the profile this build
//! does not know is kept. A marker in `prefs.json` (`erc_rules_in_profile`)
//! records that the fold happened; it is written before the profile is, so a
//! preferences file that cannot be written never causes a second fold over the
//! profile's later edits.
//!
//! The old keys stay in `prefs.json`, unread once the marker is set. A build
//! from before #1608 reads them and does not see edits made after the
//! upgrade.
//!
//! Two narrow cases leave the marker set without the fold having happened, and
//! the preferences' newer rules are then not merged: a second Signex instance
//! created `User Default` between this one's check and its exclusive create
//! (`AlreadyExists`; the other instance's file stands), and a crash between
//! the marker write and the profile write. Both lose nothing that was in the
//! profile; only the stale preferences' values are not applied.
//!
//! When the marker cannot be written the migration is refused and the app
//! keeps the machine's rules read-only for the session
//! (`UiState::machine_rules_locked`), so nothing edited in that session can be
//! overwritten by the fold of a later launch.

use std::path::Path;

use signex_erc::project_rules::profile::adopt_profile;
use signex_erc::project_rules::seed_from_overrides;
use signex_types::project::RuleProfile;

use super::{USER_DEFAULT_PROFILE_NAME, file_name_for, write_new_profile};

/// What the migration did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileMigration {
    /// The default profile was written from these preferences.
    Migrated,
    /// An existing default profile took the preferences' rules.
    Folded,
    /// Nothing to do: already folded, or the profile already holds the rules.
    AlreadyDone,
    /// The preferences hold no ERC rules; there was nothing to migrate.
    NothingToMigrate,
}

/// The ERC rules `prefs.json` still carries, and whether they were folded.
pub struct PrefsRules<'a> {
    pub severity: &'a std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
    pub pin_matrix: &'a std::collections::HashMap<(u8, u8), signex_erc::Severity>,
    /// The `erc_rules_in_profile` marker is set.
    pub folded: bool,
}

/// Fold the preferences' rules into the user default profile, once.
///
/// `set_marker(true)` is called before anything is written and must succeed
/// (else nothing is touched); `set_marker(false)` undoes it when the profile
/// could not be written, so the next launch tries again.
pub fn migrate_prefs_to_default_profile(
    dir: &Path,
    prefs: &PrefsRules<'_>,
    set_marker: &dyn Fn(bool) -> Result<(), String>,
) -> std::io::Result<ProfileMigration> {
    if prefs.folded {
        return Ok(ProfileMigration::AlreadyDone);
    }
    if prefs.severity.is_empty() && prefs.pin_matrix.is_empty() {
        return Ok(ProfileMigration::NothingToMigrate);
    }
    let path = dir.join(file_name_for(USER_DEFAULT_PROFILE_NAME));
    // A zero-byte file is the placeholder of a migration that crashed before
    // its write landed; it holds nothing of the user's, so finish the job
    // instead of treating it as done forever.
    if path
        .symlink_metadata()
        .is_ok_and(|m| m.is_file() && m.len() == 0)
    {
        std::fs::remove_file(&path)?;
    }
    set_marker(true).map_err(std::io::Error::other)?;
    let outcome = fold(dir, &path, prefs);
    if outcome.is_err() {
        // Best effort: if this fails too, the next launch simply stops at
        // the marker and the profile keeps whatever it holds.
        set_marker(false).ok();
    }
    outcome
}

fn fold(dir: &Path, path: &Path, prefs: &PrefsRules<'_>) -> std::io::Result<ProfileMigration> {
    let seeded = seed_from_overrides(prefs.severity, prefs.pin_matrix);
    if !path.exists() {
        std::fs::create_dir_all(dir)?;
        let profile = RuleProfile::new(USER_DEFAULT_PROFILE_NAME, "1.0", seeded);
        // Reserved exclusively, so a file that appeared since the check above
        // (another launch, a hand-made profile) is never replaced.
        return match write_new_profile(dir, &profile) {
            Ok(_) => Ok(ProfileMigration::Migrated),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                Ok(ProfileMigration::AlreadyDone)
            }
            Err(e) => Err(e),
        };
    }
    // An unreadable profile is the user's: report it, never replace it.
    let mut profile = RuleProfile::from_json_bytes(&std::fs::read(path)?).map_err(|e| {
        std::io::Error::other(format!(
            "{} could not be read, so the preferences' ERC rules were not merged into it: {e}",
            path.display()
        ))
    })?;
    let merged = adopt_profile(&profile.erc, &seeded);
    if merged == profile.erc {
        return Ok(ProfileMigration::AlreadyDone);
    }
    profile.erc = merged;
    signex_types::atomic_io::atomic_write(path, &profile.to_json_bytes())?;
    Ok(ProfileMigration::Folded)
}
