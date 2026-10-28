//! Managing the profile library from the app (#1608): list, create, rename,
//! duplicate, delete, edit, and "Save as profile" with an explicit overwrite.
//!
//! Same rules as the rest of `rule_profiles`: blocking file IO, called only
//! from a `Task`, the directory is a parameter. No function here replaces a
//! profile unless the caller said so (`overwrite`, an in-place edit of the
//! profile being edited); every write is atomic, and a file this module cannot
//! read as a profile is never overwritten, because it may hold the user's
//! work.

use std::path::{Path, PathBuf};

use signex_erc::project_rules::profile::{BUILTIN_PROFILE_NAME, adopt_profile, builtin_profile};
use signex_types::project::{ErcRules, RuleProfile};

use super::{
    FileError, USER_DEFAULT_PROFILE_NAME, file_name_for, read_library, user_profile_ref,
    write_new_profile,
};

/// Longest profile name the dialogs accept.
pub const MAX_NAME_CHARS: usize = 64;
/// Longest profile version string the dialogs accept.
pub const MAX_VERSION_CHARS: usize = 32;
/// Version a fresh profile starts at.
pub const DEFAULT_VERSION: &str = "1.0";

/// Whether `name` is one of the two names the library keeps for itself: the
/// shipped profile and the machine's own rule set.
pub fn is_reserved_name(name: &str) -> bool {
    let name = name.trim();
    name.eq_ignore_ascii_case(BUILTIN_PROFILE_NAME)
        || name.eq_ignore_ascii_case(USER_DEFAULT_PROFILE_NAME)
}

/// Check the inputs of the Save-as and New dialogs. Returns the trimmed name
/// and version, or the sentence to show the user.
pub fn validate_profile_input(name: &str, version: &str) -> Result<(String, String), String> {
    let name = name.trim();
    let version = version.trim();
    if name.is_empty() {
        return Err("Enter a name for the profile.".into());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(format!(
            "The name is too long (at most {MAX_NAME_CHARS} characters)."
        ));
    }
    if name.chars().any(char::is_control) {
        return Err("The name cannot contain control characters.".into());
    }
    if is_reserved_name(name) {
        return Err(format!(
            "\"{name}\" is reserved for a profile Signex manages itself; choose another name."
        ));
    }
    if version.is_empty() {
        return Err("Enter a version for the profile.".into());
    }
    if version.chars().count() > MAX_VERSION_CHARS {
        return Err(format!(
            "The version is too long (at most {MAX_VERSION_CHARS} characters)."
        ));
    }
    if version.chars().any(char::is_control) {
        return Err("The version cannot contain control characters.".into());
    }
    Ok((name.to_string(), version.to_string()))
}

/// Where a listed profile comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileKind {
    /// Shipped with the app; read-only.
    Builtin,
    /// The machine's own rule set: what the ERC dialog edits with no project
    /// open. Editable, but neither renamed nor deleted.
    UserDefault,
    /// Any other `.snxrules` file in the library.
    User,
}

/// One profile in the picker.
#[derive(Debug, Clone, PartialEq)]
pub struct ListedProfile {
    pub profile: RuleProfile,
    pub kind: ProfileKind,
}

/// Everything the Preferences profile editor shows: the profiles, and a
/// sentence for each library file that could not be used.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProfileListing {
    pub profiles: Vec<ListedProfile>,
    pub problems: Vec<String>,
}

fn is_user_default(name: &str) -> bool {
    name.eq_ignore_ascii_case(USER_DEFAULT_PROFILE_NAME)
}

/// The shipped profile, then `User Default`, then the rest by name. A file
/// that cannot be read, or repeats a name, is reported and left on disk.
pub fn list_library(dir: Option<&Path>) -> ProfileListing {
    let mut listing = ProfileListing {
        profiles: vec![ListedProfile {
            profile: builtin_profile(),
            kind: ProfileKind::Builtin,
        }],
        problems: Vec::new(),
    };
    let Some(dir) = dir else {
        listing
            .problems
            .push("This platform has no config folder for rule profiles.".into());
        return listing;
    };
    let mut library = Vec::new();
    for (path, parsed) in read_library(dir) {
        match parsed {
            Ok(profile) if profile.name.eq_ignore_ascii_case(BUILTIN_PROFILE_NAME) => {
                listing.problems.push(format!(
                    "{} is named \"{}\", the shipped profile's name, and is not listed.",
                    path.display(),
                    profile.name
                ));
            }
            Ok(profile) => library.push((path, profile)),
            Err(e) => listing.problems.push(describe_unusable(&path, &e)),
        }
    }
    library.sort_by(|a, b| {
        let key = |p: &RuleProfile| (!is_user_default(&p.name), p.name.to_lowercase());
        key(&a.1).cmp(&key(&b.1)).then(a.0.cmp(&b.0))
    });
    for (path, profile) in library {
        let taken = listing
            .profiles
            .iter()
            .any(|l| l.profile.name.eq_ignore_ascii_case(&profile.name));
        if taken {
            listing.problems.push(format!(
                "{} repeats the name \"{}\" and is not listed. Rename or remove one of the files.",
                path.display(),
                profile.name
            ));
            continue;
        }
        let kind = if is_user_default(&profile.name) {
            ProfileKind::UserDefault
        } else {
            ProfileKind::User
        };
        listing.profiles.push(ListedProfile { profile, kind });
    }
    listing
}

fn describe_unusable(path: &Path, error: &FileError) -> String {
    match error {
        FileError::Invalid(signex_types::project::RuleProfileError::UnsupportedSchema(v)) => {
            format!(
                "{} was written by a newer Signex (schema {v}) and is not listed.",
                path.display()
            )
        }
        FileError::Invalid(e) => format!("{} could not be read ({e}).", path.display()),
        FileError::Unreadable(e) => format!("{} could not be opened ({e}).", path.display()),
    }
}

/// Where a profile of a given name lives, or would live.
enum Slot {
    /// A library file already holds a profile of this name.
    Holds(PathBuf),
    /// No profile has this name and its file name is free.
    Free,
}

/// Find the file of the profile called `name` (case-insensitive), or report
/// that its file name is free. An unreadable file in the way is an error: it
/// may hold a profile of that name.
fn locate(dir: &Path, name: &str) -> Result<Slot, String> {
    let mut holders = Vec::new();
    for (path, parsed) in read_library(dir) {
        if let Ok(profile) = parsed
            && profile.name.eq_ignore_ascii_case(name)
        {
            holders.push(path);
        }
    }
    if holders.len() > 1 {
        holders.sort();
        let files: Vec<String> = holders.iter().map(|p| p.display().to_string()).collect();
        return Err(format!(
            "the profile name \"{name}\" is ambiguous: {} all carry it. Rename or remove all but one.",
            files.join(", ")
        ));
    }
    if let Some(path) = holders.pop() {
        return Ok(Slot::Holds(path));
    }
    let path = dir.join(file_name_for(name));
    if path.symlink_metadata().is_ok() {
        return Err(format!(
            "{} already exists but does not hold a profile named \"{name}\" that could be read; \
             it was left alone. Choose another name.",
            path.display()
        ));
    }
    Ok(Slot::Free)
}

fn io_error(what: &str, name: &str, error: &std::io::Error) -> String {
    format!("Could not {what} profile \"{name}\": {error}")
}

/// What "Save as profile" did.
#[derive(Debug, Clone, PartialEq)]
pub enum SaveOutcome {
    /// The profile is on disk.
    Saved(super::SavedProfile),
    /// A profile of this name exists and `overwrite` was not set; nothing was
    /// written.
    Exists,
}

/// Save `erc` as the profile `name`/`version`. Never replaces an existing
/// profile unless `overwrite` is set, which the caller sets only after the
/// user confirmed. Inputs are re-validated here, so no caller can write a
/// name the dialogs would refuse.
pub fn save_named_profile(
    dir: &Path,
    name: &str,
    version: &str,
    erc: ErcRules,
    overwrite: bool,
) -> Result<SaveOutcome, String> {
    let (name, version) = validate_profile_input(name, version)?;
    std::fs::create_dir_all(dir).map_err(|e| io_error("save", &name, &e))?;
    let profile = RuleProfile::new(name.clone(), version, erc);
    let path = match locate(dir, &name)? {
        Slot::Holds(_) if !overwrite => return Ok(SaveOutcome::Exists),
        Slot::Holds(path) => {
            signex_types::atomic_io::atomic_write(&path, &profile.to_json_bytes())
                .map_err(|e| io_error("overwrite", &name, &e))?;
            path
        }
        Slot::Free => match write_new_profile(dir, &profile) {
            Ok(path) => path,
            // Someone created the file between the check and the write.
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                return Ok(SaveOutcome::Exists);
            }
            Err(e) => return Err(io_error("save", &name, &e)),
        },
    };
    Ok(SaveOutcome::Saved(super::SavedProfile {
        reference: user_profile_ref(&profile),
        summary: format!("\"{}\" at {}", profile.name, path.display()),
    }))
}

/// A new profile named `name` that starts as a copy of `source`'s rules and
/// version. Fails when the name is taken; never replaces.
pub fn create_profile_from(dir: &Path, source: &RuleProfile, name: &str) -> Result<String, String> {
    match save_named_profile(dir, name, &source.version, source.erc.clone(), false)? {
        SaveOutcome::Saved(saved) => Ok(saved.summary),
        SaveOutcome::Exists => Err(format!(
            "A profile named \"{}\" already exists.",
            name.trim()
        )),
    }
}

/// Rename the profile `old` to `new`: the new file is written first and the
/// old one removed after, so a failure in between leaves both, never neither.
/// The shipped profile and `User Default` cannot be renamed.
pub fn rename_profile(dir: &Path, old: &str, new: &str) -> Result<(), String> {
    if is_reserved_name(old) {
        return Err(format!("\"{old}\" cannot be renamed."));
    }
    let (new, _) = validate_profile_input(new, DEFAULT_VERSION)?;
    let Slot::Holds(old_path) = locate(dir, old)? else {
        return Err(format!("There is no profile named \"{old}\"."));
    };
    let bytes = std::fs::read(&old_path).map_err(|e| io_error("read", old, &e))?;
    let mut profile = RuleProfile::from_json_bytes(&bytes)
        .map_err(|e| format!("Could not read profile \"{old}\": {e}"))?;
    profile.name = new.clone();
    if old.eq_ignore_ascii_case(&new) {
        // Only the capitalisation changes: same file, rewritten in place.
        return signex_types::atomic_io::atomic_write(&old_path, &profile.to_json_bytes())
            .map_err(|e| io_error("rename", old, &e));
    }
    if let Slot::Holds(_) = locate(dir, &new)? {
        return Err(format!("A profile named \"{new}\" already exists."));
    }
    write_new_profile(dir, &profile).map_err(|e| io_error("rename", old, &e))?;
    std::fs::remove_file(&old_path).map_err(|e| {
        format!(
            "Profile \"{old}\" was copied to \"{new}\" but the old file {} could not be removed: {e}",
            old_path.display()
        )
    })
}

/// Delete the profile `name`'s file. The shipped profile and `User Default`
/// cannot be deleted.
pub fn delete_profile(dir: &Path, name: &str) -> Result<(), String> {
    if is_reserved_name(name) {
        return Err(format!("\"{name}\" cannot be deleted."));
    }
    let Slot::Holds(path) = locate(dir, name)? else {
        return Err(format!("There is no profile named \"{name}\"."));
    };
    std::fs::remove_file(&path).map_err(|e| io_error("delete", name, &e))
}

/// One change to a profile's rules, applied to the file as it is on disk
/// when the write runs, not to a copy the screen held earlier, so an edit made
/// elsewhere in between (another window's Save as, a hand edit) is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleEdit {
    /// One rule's severity.
    Severity(signex_erc::RuleKind, signex_erc::Severity),
    /// Cycle one pin-matrix cell (and its mirror).
    PinCell(u8, u8),
    /// Every rule back to the shipped profile's. Rule ids and keys this build
    /// does not know are kept.
    ResetToShipped,
}

impl RuleEdit {
    pub fn apply(self, erc: &mut ErcRules) {
        use signex_erc::project_rules::{cycle_pin_cell, set_severity};
        match self {
            RuleEdit::Severity(rule, severity) => set_severity(erc, rule, severity),
            RuleEdit::PinCell(row, col) => cycle_pin_cell(erc, row, col),
            RuleEdit::ResetToShipped => {
                *erc = adopt_profile(erc, &builtin_profile().erc);
            }
        }
    }
}

/// Re-read the file of the library profile `name`, apply `edit`, write it back
/// and return what the file now holds. Fails if the file is gone (a deleted
/// profile is not silently recreated), cannot be read (it is left alone), or
/// the profile is the shipped one.
pub fn edit_profile_in_place(
    dir: &Path,
    name: &str,
    edit: RuleEdit,
) -> Result<RuleProfile, String> {
    if name.eq_ignore_ascii_case(BUILTIN_PROFILE_NAME) {
        return Err("The shipped profile cannot be edited; duplicate it first.".into());
    }
    let Slot::Holds(path) = locate(dir, name)? else {
        return Err(format!(
            "Profile \"{name}\" is no longer in the library, so the edit was not saved."
        ));
    };
    let bytes = std::fs::read(&path).map_err(|e| io_error("read", name, &e))?;
    let mut profile = RuleProfile::from_json_bytes(&bytes)
        .map_err(|e| format!("Could not read profile \"{name}\": {e}"))?;
    edit.apply(&mut profile.erc);
    signex_types::atomic_io::atomic_write(&path, &profile.to_json_bytes())
        .map_err(|e| io_error("save", name, &e))?;
    Ok(profile)
}

/// The machine's rule set, if it has been created.
pub fn load_user_default(dir: &Path) -> Result<Option<RuleProfile>, String> {
    let reference = signex_types::project::RuleProfileRef {
        name: USER_DEFAULT_PROFILE_NAME.into(),
        source: signex_erc::project_rules::profile::SOURCE_USER.into(),
        ..Default::default()
    };
    Ok(super::resolve_profile(Some(dir), &reference)?.map(|(profile, _)| profile))
}

/// Store `erc` as the machine's rule set, creating the profile on first use.
/// What the existing file holds that this build does not know (rule ids and
/// keys from a newer build) and its version are kept. A file that exists but
/// cannot be read is not replaced: the error says so and the edit stays in
/// memory only.
pub fn write_user_default(dir: &Path, erc: ErcRules) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| io_error("save", USER_DEFAULT_PROFILE_NAME, &e))?;
    match locate(dir, USER_DEFAULT_PROFILE_NAME)? {
        Slot::Free => {
            let profile = RuleProfile::new(USER_DEFAULT_PROFILE_NAME, DEFAULT_VERSION, erc);
            write_new_profile(dir, &profile)
                .map(drop)
                .map_err(|e| io_error("save", USER_DEFAULT_PROFILE_NAME, &e))
        }
        Slot::Holds(path) => {
            let bytes = std::fs::read(&path)
                .map_err(|e| io_error("read", USER_DEFAULT_PROFILE_NAME, &e))?;
            let mut profile = RuleProfile::from_json_bytes(&bytes).map_err(|e| {
                format!("Could not read profile \"{USER_DEFAULT_PROFILE_NAME}\": {e}")
            })?;
            profile.erc = adopt_profile(&profile.erc, &erc);
            signex_types::atomic_io::atomic_write(&path, &profile.to_json_bytes())
                .map_err(|e| io_error("save", USER_DEFAULT_PROFILE_NAME, &e))
        }
    }
}
