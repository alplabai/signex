//! Reading and writing the user's shortcut file, backups and import/export.

use super::schema::{TomlShortcutConfig, TomlShortcutProfile};
use super::{ProfileLoadError, ShortcutProfile, ShortcutProfileKind, ShortcutProfileSet};
use std::path::{Path, PathBuf};

const USER_SHORTCUTS_FILE_NAME: &str = "keyboard_shortcuts.toml";

pub fn config_path() -> Option<PathBuf> {
    #[cfg(test)]
    if let Some(root) = crate::config_root::isolated::current() {
        return Some(root.join(USER_SHORTCUTS_FILE_NAME));
    }
    crate::config_root::config_root().map(|root| root.join(USER_SHORTCUTS_FILE_NAME))
}

pub fn config_path_for_dir(base: &Path) -> PathBuf {
    crate::config_root::config_root_for_dir(base).join(USER_SHORTCUTS_FILE_NAME)
}

pub fn load_profile_set() -> Result<ShortcutProfileSet, ProfileLoadError> {
    let Some(path) = config_path() else {
        return ShortcutProfileSet::built_ins();
    };
    load_profile_set_at(&path)
}

pub fn load_profile_set_at(path: &Path) -> Result<ShortcutProfileSet, ProfileLoadError> {
    let mut set = ShortcutProfileSet::built_ins()?;
    if !path.exists() {
        return Ok(set);
    }

    let source = std::fs::read_to_string(path).map_err(ProfileLoadError::Io)?;
    let config = TomlShortcutConfig::parse(&source)?;
    config.apply_to(&mut set)?;
    Ok(set)
}

pub fn save_profile_set(set: &ShortcutProfileSet) -> Result<(), ProfileLoadError> {
    let Some(path) = config_path() else {
        return Err(ProfileLoadError::NoConfigDir);
    };
    save_profile_set_at(&path, set)
}

/// Crash-safe: [`signex_types::atomic_io::atomic_write`] writes to a temp
/// sibling, fsyncs it and renames over the destination, so a crash mid-save
/// leaves the user's previously saved keymap profiles intact rather than a
/// truncated file. It also creates the parent directory, so no separate
/// `create_dir_all` here.
pub fn save_profile_set_at(path: &Path, set: &ShortcutProfileSet) -> Result<(), ProfileLoadError> {
    let source = export_custom_profiles(set)?;
    signex_types::atomic_io::atomic_write(path, source.as_bytes()).map_err(ProfileLoadError::Io)
}

/// What a restore recovered out of a backup, and what it could not.
pub struct RestoredProfiles {
    /// The profile set the backup describes, built-ins included.
    pub set: ShortcutProfileSet,
    /// `Some(id)` when the backup's `active_profile` named a profile
    /// that is not in it. The custom profiles are all present and this
    /// is the one thing that could not be honoured, so the set comes
    /// back with the built-in default active rather than failing the
    /// whole restore over a dangling pointer. That is the single most
    /// likely reason the file failed to load in the first place.
    pub active_profile_reset: Option<String>,
}

/// Read the profiles out of a backup file, through the normal loader.
///
/// Deliberately NOT "copy the `.bak` back over the live file". The
/// backup holds the file that failed to load; putting it back would
/// reproduce the same failure on the next launch. Parsing it instead
/// means an unrepairable backup fails here, cleanly, with both files
/// untouched — and a backup whose only fault was a dangling
/// `active_profile` is recovered in full.
pub fn read_backup_profiles_at(bak: &Path) -> Result<RestoredProfiles, ProfileLoadError> {
    let source = std::fs::read_to_string(bak).map_err(ProfileLoadError::Io)?;
    let mut set = ShortcutProfileSet::built_ins()?;
    let active_profile_reset = TomlShortcutConfig::parse(&source)?.apply_to_recovering(&mut set)?;
    Ok(RestoredProfiles {
        set,
        active_profile_reset,
    })
}

pub fn import_custom_profile(source: &str) -> Result<ShortcutProfile, ProfileLoadError> {
    let profile = TomlShortcutProfile::parse(source)?.into_profile()?;
    if profile.kind != ShortcutProfileKind::Custom {
        return Err(ProfileLoadError::BuiltInMutation(profile.id));
    }
    Ok(profile)
}

pub fn export_custom_profile(profile: &ShortcutProfile) -> Result<String, ProfileLoadError> {
    if profile.kind != ShortcutProfileKind::Custom {
        return Err(ProfileLoadError::BuiltInMutation(profile.id.clone()));
    }
    let document = TomlShortcutProfile::from_profile(profile)?;
    toml::to_string_pretty(&document).map_err(ProfileLoadError::TomlSerialize)
}

pub fn export_custom_profiles(set: &ShortcutProfileSet) -> Result<String, ProfileLoadError> {
    let document = TomlShortcutConfig::from_profile_set(set)?;
    toml::to_string_pretty(&document).map_err(ProfileLoadError::TomlSerialize)
}
