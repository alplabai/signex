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

mod migrate;
pub use migrate::{ProfileMigration, migrate_prefs_to_default_profile};

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

/// Write `profile` into the library, creating the folder when needed.
/// A profile of the same name is replaced, so only callers that have already
/// checked the file is free (or mean to replace it) use this; "Save as
/// profile" goes through [`save_new_profile`].
pub fn save_profile(dir: &Path, profile: &RuleProfile) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(file_name_for(&profile.name));
    signex_types::atomic_io::atomic_write(&path, &profile.to_json_bytes())?;
    Ok(path)
}

/// Save `erc` as a new profile that never replaces anything: the first of
/// `name`, `name 2`, `name 3`, ... whose file this call could reserve and
/// which is not the shipped profile's name. Returns the profile as written and
/// its path.
///
/// The file is reserved with an exclusive create before it is written, so two
/// concurrent saves of one name cannot pick the same file: the loser sees
/// `AlreadyExists` and moves on to the next name.
pub fn save_new_profile(
    dir: &Path,
    name: &str,
    erc: signex_types::project::ErcRules,
) -> std::io::Result<(RuleProfile, PathBuf)> {
    let name = name.trim();
    std::fs::create_dir_all(dir)?;
    let candidates = std::iter::once(name.to_string())
        .chain((2u32..10_000).map(|n| format!("{name} {n}")))
        .filter(|candidate| !candidate.eq_ignore_ascii_case(BUILTIN_PROFILE_NAME));
    for candidate in candidates {
        let path = dir.join(file_name_for(&candidate));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(placeholder) => {
                drop(placeholder);
                let profile = RuleProfile::new(candidate, "1.0", erc);
                if let Err(e) =
                    signex_types::atomic_io::atomic_write(&path, &profile.to_json_bytes())
                {
                    // Our own empty placeholder; nothing of the user's is lost.
                    std::fs::remove_file(&path).ok();
                    return Err(e);
                }
                return Ok((profile, path));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "no free profile name",
    ))
}

/// What a successful "Save as profile" produced.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedProfile {
    /// Provenance of the file as written, for the project to record.
    pub reference: RuleProfileRef,
    /// One line for the Messages panel.
    pub summary: String,
}

/// [`save_new_profile`] plus the provenance record and the message line.
pub fn save_as_profile(
    dir: &Path,
    name: &str,
    erc: signex_types::project::ErcRules,
) -> Result<SavedProfile, String> {
    let name = name.trim();
    let (profile, path) = save_new_profile(dir, name, erc)
        .map_err(|e| format!("Could not save profile \"{name}\": {e}"))?;
    let note = if profile.name == name {
        String::new()
    } else {
        format!(" (\"{name}\" was already taken)")
    };
    Ok(SavedProfile {
        reference: user_profile_ref(&profile),
        summary: format!("\"{}\"{note} at {}", profile.name, path.display()),
    })
}

/// Why a library file gave no profile.
#[derive(Debug)]
enum FileError {
    /// The file exists but the OS refused to read it (permissions, a
    /// directory named like a profile, ...).
    Unreadable(std::io::Error),
    Invalid(RuleProfileError),
}

/// Every `.snxrules` file in the library with what reading it gave.
fn read_library(dir: &Path) -> Vec<(PathBuf, Result<RuleProfile, FileError>)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == RULE_PROFILE_EXTENSION))
        .map(|p| {
            let parsed = match std::fs::read(&p) {
                Ok(bytes) => RuleProfile::from_json_bytes(&bytes).map_err(FileError::Invalid),
                Err(e) => Err(FileError::Unreadable(e)),
            };
            (p, parsed)
        })
        .collect()
}

/// Every readable profile in the library, sorted by name. A file that does
/// not parse is skipped here (it is still on disk, untouched).
pub fn list_profiles(dir: &Path) -> Vec<RuleProfile> {
    let mut out: Vec<RuleProfile> = read_library(dir)
        .into_iter()
        .filter_map(|(_, parsed)| parsed.ok())
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
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
    let expected_file = file_name_for(&reference.name);
    for (path, parsed) in read_library(dir) {
        match parsed {
            Ok(profile) if profile.name == reference.name => matches.push((path, profile)),
            // A file that would hold this profile but cannot be read is the
            // answer, not "no such profile".
            Err(e) if path.file_name().is_some_and(|f| *f == *expected_file) => {
                unreadable = Some((path, e));
            }
            _ => {}
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
        None => Ok(None),
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
