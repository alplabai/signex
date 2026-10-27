//! Backing up, restoring and discarding the user's shortcuts file.
//!
//! Backups live beside the file as `<name>.bak`, `<name>.bak.2`, … and are
//! never overwritten: a new backup (or a restore's moved-aside file) takes
//! the first free slot.

use super::{ProfileLoadError, ShortcutProfileSet, config_path, save_profile_set_at};
use std::path::{Path, PathBuf};

/// Copy the existing shortcuts file aside before a save that would
/// otherwise overwrite profiles this process failed to load (#595).
///
/// Three outcomes, all load-bearing:
///
/// * `path` does not exist — `Ok(None)`, there is nothing to preserve.
/// * otherwise — copy `path` to the first free sibling name from
///   [`free_aside_path`] (`.bak`, `.bak.2`, …) and return
///   `Ok(Some(bak_path))`. An existing backup is never overwritten, and
///   it no longer suppresses a new one either (#1378): the old
///   "any `.bak` means skip" rule lost the current file whenever an
///   older backup was lying around.
///
/// Both the source and the candidate names are probed with
/// `symlink_metadata`, so a stat error other than `NotFound` is an `Err`
/// rather than "absent" (`Path::exists()` answers `false` for a name it
/// merely cannot stat).
///
/// An `Err` means the copy could not be made, and the caller must then
/// refuse to save: overwriting after a failed backup is the data loss
/// this function exists to prevent, not a fallback.
pub fn back_up_profile_file_at(path: &Path) -> Result<Option<PathBuf>, ProfileLoadError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ProfileLoadError::Io(error)),
    }
    let bak = free_aside_path(path).map_err(ProfileLoadError::Io)?;
    std::fs::copy(path, &bak).map_err(ProfileLoadError::Io)?;
    Ok(Some(bak))
}

/// [`back_up_profile_file_at`] against the resolved user config path,
/// mirroring the [`save_profile_set`] / [`save_profile_set_at`] pair.
pub fn back_up_profile_file() -> Result<Option<PathBuf>, ProfileLoadError> {
    let Some(path) = config_path() else {
        return Err(ProfileLoadError::NoConfigDir);
    };
    back_up_profile_file_at(&path)
}

/// The newest backup beside the resolved shortcuts file — what the UI
/// offers to Restore from or Discard, and the one the last backup's status
/// line named (#1378). `None` when there is no config directory or no
/// backup; a stat failure is logged and also reads as `None`, because this
/// only decides whether to *show* two controls (the destructive paths work
/// from the path they are handed and go through `symlink_metadata`).
pub fn existing_backup_profiles_path() -> Option<PathBuf> {
    let path = config_path()?;
    match newest_backup_at(&path) {
        Ok(newest) => newest,
        Err(error) => {
            tracing::warn!(
                target: "signex::keymap",
                "cannot look for a shortcuts backup ({error}); looked beside {}",
                path.display()
            );
            None
        }
    }
}

/// Sibling name for backup `slot` of `path`: slot 1 is `<name>.bak`, then
/// `<name>.bak.2`, `.bak.3`, … Appended to the WHOLE file name through
/// [`std::ffi::OsString`], so the `.toml` extension survives and a
/// non-UTF-8 directory name is not mangled.
fn slot_path(path: &Path, slot: u32) -> Option<PathBuf> {
    let mut name = path.file_name()?.to_os_string();
    name.push(".bak");
    if slot > 1 {
        name.push(format!(".{slot}"));
    }
    Some(path.with_file_name(name))
}

/// The highest-numbered backup slot that exists beside `path`. New backups
/// always take the first free slot and discard removes only this one, so
/// the highest slot is the most recent.
///
/// Every slot is probed, not just the contiguous run, so a gap cannot hide
/// a newer backup. A stat error other than `NotFound` is an `Err`.
fn newest_backup_at(path: &Path) -> Result<Option<PathBuf>, std::io::Error> {
    let mut newest = None;
    for slot in 1..=MAX_ASIDE_SLOTS {
        let Some(candidate) = slot_path(path, slot) else {
            return Ok(None);
        };
        match std::fs::symlink_metadata(&candidate) {
            Ok(_) => newest = Some(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(newest)
}

/// How many aside slots to try before giving up, matching
/// `fonts::prefs_file`'s ladder. A user who has restored a hundred times
/// has a problem no rename will fix.
const MAX_ASIDE_SLOTS: u32 = 100;

/// First free sibling name to move the live shortcuts file aside to
/// before a restore overwrites it.
///
/// Slot 1 is the plain `.bak`; the ladder numbers from `.bak.2`, so this
/// covers exactly [`MAX_ASIDE_SLOTS`] names and the exhaustion message
/// below stays true. A slot counts as free only when
/// `symlink_metadata` answers `NotFound`; any other error aborts
/// carrying that errno rather than laddering past it, because
/// `Path::exists()` answers `false` for a name it merely cannot stat and
/// `std::fs::rename` silently replaces its destination — the file that
/// vanished would be the user's earlier backup.
fn free_aside_path(path: &Path) -> Result<PathBuf, std::io::Error> {
    for slot in 1..=MAX_ASIDE_SLOTS {
        let Some(candidate) = slot_path(path, slot) else {
            return Err(std::io::Error::other(format!(
                "shortcuts file has no file name to back up: {}",
                path.display()
            )));
        };
        match std::fs::symlink_metadata(&candidate) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(candidate),
            Err(error) => return Err(error),
            Ok(_) => {}
        }
    }
    Err(std::io::Error::other(format!(
        "all {MAX_ASIDE_SLOTS} .bak slots are taken beside {}",
        path.display()
    )))
}

/// Move the live shortcuts file aside to a free slot, then write `set`
/// in its place. Returns where the previous file went, or `Ok(None)`
/// when there was no live file to preserve.
///
/// The move is what makes a restore safe to offer at any time. The
/// `.bak` is never cleaned up, so it outlives the failure that produced
/// it: months later the user may have built a whole new set of profiles
/// and still have that ancient backup sitting there. Writing straight
/// over the live file would throw the new work away — the same shape as
/// the `prefs.json` clobber in #594.
pub fn restore_profiles_at(
    path: &Path,
    set: &ShortcutProfileSet,
) -> Result<Option<PathBuf>, ProfileLoadError> {
    let aside = match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(ProfileLoadError::Io(error)),
        Ok(_) => {
            let target = free_aside_path(path).map_err(ProfileLoadError::Io)?;
            std::fs::rename(path, &target).map_err(ProfileLoadError::Io)?;
            Some(target)
        }
    };
    save_profile_set_at(path, set)?;
    Ok(aside)
}

/// Delete a profile backup. Only ever called from an explicit user
/// action — a successful save must never remove it, because that is
/// exactly the moment the user is most likely to want the old profiles
/// back. `Ok(false)` when there was nothing to delete.
pub fn discard_profile_backup_at(bak: &Path) -> Result<bool, ProfileLoadError> {
    match std::fs::remove_file(bak) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(ProfileLoadError::Io(error)),
    }
}

#[cfg(test)]
mod tests;
