//! The user's rule-profile library on disk (#1085): `.snxrules` files under
//! the app config directory.
//!
//! Every function here does blocking file IO, so the app calls them only from
//! a `Task` (`spawn_blocking`), never from `update`. The directory is a
//! parameter, not read from the environment, so tests point it at a tempdir.
//! The rule vocabulary and the diff live in `signex_erc::project_rules::profile`.

use std::path::{Path, PathBuf};

use signex_erc::project_rules::profile::{
    BUILTIN_PROFILE_NAME, SOURCE_BUILTIN, builtin_profile, profile_ref,
};
use signex_types::project::{
    RULE_PROFILE_EXTENSION, RuleProfile, RuleProfileError, RuleProfileRef,
};

mod manage;
mod migrate;
mod seed;
pub use manage::*;
pub use migrate::{PrefsRules, ProfileMigration, migrate_prefs_to_default_profile};
pub use seed::rules_for_new_project;

#[cfg(test)]
mod manage_tests;
#[cfg(test)]
mod tests;

/// A profile found for a provenance record, with the `source` to record when
/// a project adopts it (`"builtin"` or `"user"`).
pub type ResolvedProfile = (RuleProfile, &'static str);

/// Name of the profile the machine's old `prefs.json` rules migrate into.
pub const USER_DEFAULT_PROFILE_NAME: &str = "User Default";

/// Folder of the user's profile library, or `None` when the platform has no
/// config directory.
pub fn profiles_dir() -> Option<PathBuf> {
    Some(crate::config_root::config_root()?.join("rule-profiles"))
}

/// File name a profile of `name` is stored under: lower-case, anything that
/// is not a letter or digit collapsed to `-`.
pub fn file_name_for(name: &str) -> String {
    let mut stem = String::new();
    for ch in name.trim().chars() {
        if ch.is_alphanumeric() {
            stem.extend(ch.to_lowercase());
        } else if !stem.ends_with('-') {
            stem.push('-');
        }
    }
    let stem = stem.trim_matches('-');
    let stem = if stem.is_empty() { "profile" } else { stem };
    format!("{stem}.{RULE_PROFILE_EXTENSION}")
}

/// Write `profile` to its file only if that file does not exist yet:
/// reserve the name with an exclusive create, then fill it atomically.
/// `AlreadyExists` means the file is someone else's and was not touched.
///
/// A crash between the reservation and the write leaves a zero-byte file;
/// [`read_library`] ignores those, so it is never reported as a corrupt
/// profile (it only keeps its file name from being reused).
pub(super) fn write_new_profile(dir: &Path, profile: &RuleProfile) -> std::io::Result<PathBuf> {
    let path = dir.join(file_name_for(&profile.name));
    drop(
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?,
    );
    if let Err(e) = signex_types::atomic_io::atomic_write(&path, &profile.to_json_bytes()) {
        // Our own empty placeholder; nothing of the user's is lost.
        std::fs::remove_file(&path).ok();
        return Err(e);
    }
    Ok(path)
}

/// What a successful "Save as profile" produced.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedProfile {
    /// Provenance of the file as written, for the project to record.
    pub reference: RuleProfileRef,
    /// One line for the Messages panel.
    pub summary: String,
}

/// Why a library file gave no profile.
#[derive(Debug)]
pub(super) enum FileError {
    /// The file exists but the OS refused to read it (permissions, a
    /// directory named like a profile, ...).
    Unreadable(std::io::Error),
    Invalid(RuleProfileError),
}

/// Every `.snxrules` file in the library with what reading it gave. A
/// zero-byte file is a reservation a crashed save left behind, not a profile,
/// and is left out.
pub(super) fn read_library(dir: &Path) -> Vec<(PathBuf, Result<RuleProfile, FileError>)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == RULE_PROFILE_EXTENSION))
        .filter_map(|p| {
            let parsed = match std::fs::read(&p) {
                Ok(bytes) if bytes.is_empty() => return None,
                Ok(bytes) => RuleProfile::from_json_bytes(&bytes).map_err(FileError::Invalid),
                Err(e) => Err(FileError::Unreadable(e)),
            };
            Some((p, parsed))
        })
        .collect()
}

/// The local profile a project's provenance record names: the shipped one
/// for `source == "builtin"`, otherwise the library's profile of that name.
/// `Ok(None)` when no such profile exists on this machine.
pub fn resolve_profile(
    dir: Option<&Path>,
    reference: &RuleProfileRef,
) -> Result<Option<ResolvedProfile>, String> {
    use signex_erc::project_rules::profile::SOURCE_USER;
    if reference.source == SOURCE_BUILTIN {
        return Ok(
            (reference.name == BUILTIN_PROFILE_NAME).then(|| (builtin_profile(), SOURCE_BUILTIN))
        );
    }
    let Some(dir) = dir else {
        return Err("this platform has no config folder for rule profiles".into());
    };
    let mut matches: Vec<(PathBuf, RuleProfile)> = Vec::new();
    let mut unreadable = None;
    let mut other_unreadable: Vec<PathBuf> = Vec::new();
    let expected_file = file_name_for(&reference.name);
    for (path, parsed) in read_library(dir) {
        match parsed {
            Ok(profile) if profile.name == reference.name => matches.push((path, profile)),
            // A file that would hold this profile but cannot be read is the
            // answer, not "no such profile".
            Err(e) if path.file_name().is_some_and(|f| *f == *expected_file) => {
                unreadable = Some((path, e));
            }
            // Under another file name it might still hold this profile.
            Err(_) => other_unreadable.push(path),
            Ok(_) => {}
        }
    }
    if matches.len() > 1 {
        matches.sort_by(|a, b| a.0.cmp(&b.0));
        let files: Vec<String> = matches
            .iter()
            .map(|(p, _)| p.display().to_string())
            .collect();
        return Err(format!(
            "the profile name \"{}\" is ambiguous: {} all carry it. Rename or remove all but one.",
            reference.name,
            files.join(", ")
        ));
    }
    if let Some((_, profile)) = matches.pop() {
        return Ok(Some((profile, SOURCE_USER)));
    }
    match unreadable {
        None if other_unreadable.is_empty() => Ok(None),
        None => {
            other_unreadable.sort();
            let files: Vec<String> = other_unreadable
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            Err(format!(
                "no readable profile is named \"{}\", but {} could not be read and may hold it",
                reference.name,
                files.join(", ")
            ))
        }
        Some((path, FileError::Invalid(RuleProfileError::UnsupportedSchema(v)))) => Err(format!(
            "the profile file {} was written by a newer Signex (schema {v})",
            path.display()
        )),
        Some((path, FileError::Invalid(e))) => Err(format!(
            "the profile file {} could not be read: {e}",
            path.display()
        )),
        Some((path, FileError::Unreadable(e))) => Err(format!(
            "the profile file {} could not be opened: {e}",
            path.display()
        )),
    }
}

/// Provenance for a profile saved to or read from the library.
pub fn user_profile_ref(profile: &RuleProfile) -> RuleProfileRef {
    profile_ref(profile, signex_erc::project_rules::profile::SOURCE_USER)
}
