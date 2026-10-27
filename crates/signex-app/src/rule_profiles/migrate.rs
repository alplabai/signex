//! One-time move of the machine's `prefs.json` ERC rules into the user's
//! default profile (#1085).
//!
//! `prefs.json` keeps its keys: it stays the fallback for a project that
//! stores no rules, so nothing is deleted and a downgrade loses nothing. The
//! default profile is written once; after that the file is the user's own and
//! is never overwritten by a later launch.

use std::path::Path;

use signex_erc::project_rules::seed_from_overrides;
use signex_types::project::RuleProfile;

use super::{USER_DEFAULT_PROFILE_NAME, file_name_for, write_new_profile};

/// What the migration did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileMigration {
    /// The default profile was written from these preferences.
    Migrated,
    /// A default profile already exists; nothing was touched.
    AlreadyDone,
    /// The preferences hold no ERC rules; there was nothing to migrate.
    NothingToMigrate,
}

/// Write the user default profile from the preferences' severity overrides
/// and pin matrix, unless it already exists or there is nothing to carry.
pub fn migrate_prefs_to_default_profile(
    dir: &Path,
    severity: &std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
    pin_matrix: &std::collections::HashMap<(u8, u8), signex_erc::Severity>,
) -> std::io::Result<ProfileMigration> {
    if dir.join(file_name_for(USER_DEFAULT_PROFILE_NAME)).exists() {
        return Ok(ProfileMigration::AlreadyDone);
    }
    if severity.is_empty() && pin_matrix.is_empty() {
        return Ok(ProfileMigration::NothingToMigrate);
    }
    let profile = RuleProfile::new(
        USER_DEFAULT_PROFILE_NAME,
        "1.0",
        seed_from_overrides(severity, pin_matrix),
    );
    std::fs::create_dir_all(dir)?;
    // Reserved exclusively, so a file that appeared since the check above
    // (another launch, a hand-made profile) is never replaced.
    match write_new_profile(dir, &profile) {
        Ok(_) => Ok(ProfileMigration::Migrated),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            Ok(ProfileMigration::AlreadyDone)
        }
        Err(e) => Err(e),
    }
}
