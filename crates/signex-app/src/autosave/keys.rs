//! The names a recovery copy of one document may sit under.
//!
//! The current key is the app-wide path comparison key
//! ([`path_key_with`]) under the case rule of the document's volume, so two
//! spellings of one file on a case-insensitive volume share a copy and on a
//! case-sensitive volume stay distinct. Dot segments resolve lexically,
//! never by a disk lookup: `w/link/../x` and `w/x` share a key even when
//! `link` is a symlink (the key must be computable without IO, as
//! `KeyMap::lexical_key` is).
//!
//! The case rule is probed, and the probe has to give the same answer
//! before and after the document's folder is renamed or moved. It is
//! therefore asked of the nearest folder that exists, and an answer that
//! cannot be had means "keep case": an undecided probe never merges two
//! spellings into one key. Because a probe can still change its mind, a lookup
//! never trusts the current rule alone: [`all_keys`] lists every form a copy
//! may have been filed under, with either rule, and the older builds' forms.

use std::path::{Path, PathBuf};

use crate::app::{CaseFold, path_key_with, probe_directory_case};

use super::sidecar::{SIDECAR_EXTENSION, bytes_hash};

/// One way a key has been built.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum KeyForm {
    /// The current key under a case rule (`true`: the volume ignores case).
    Keyed(bool),
    /// Builds from #1530 to #1540: the dot-segment normalised text as spelled.
    AsGiven,
    /// Builds before #1510: the raw text as spelled.
    Raw,
    /// The case-folded spelling of the path, as-given hashed (the carry of #1530).
    FoldedAsGiven,
    /// The case-folded spelling of the path, raw hashed.
    FoldedRaw,
}

impl KeyForm {
    /// Whether a copy under this form may belong to a spelling other than the
    /// one asked about, because the key merges spellings that differ in case.
    pub(super) fn merges_spellings(self) -> bool {
        matches!(
            self,
            Self::Keyed(true) | Self::FoldedAsGiven | Self::FoldedRaw
        )
    }
}

/// Whether the volume of `document` ignores case, for keying. Asked of the
/// nearest existing folder (the document's own folder may be mid-rename), and
/// `false` when the probe cannot decide. Disk IO: recovery tasks only.
pub(super) fn stable_ignores_case(document: &Path) -> bool {
    let mut dir = document.parent();
    while let Some(d) = dir {
        let probed = if d.as_os_str().is_empty() {
            Path::new(".")
        } else {
            d
        };
        if probed.is_dir() {
            return matches!(probe_directory_case(probed), Ok(Some(true)));
        }
        dir = d.parent();
    }
    false
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "document".to_string())
}

fn named(recovery_dir: &Path, name: &str, hash: u64) -> PathBuf {
    recovery_dir.join(format!("{name}-{hash:016x}.{SIDECAR_EXTENSION}"))
}

fn as_given_hash(path: &Path) -> u64 {
    let path = crate::app::project_sheets::normalize_clamped(path);
    bytes_hash(path.to_string_lossy().as_bytes())
}

fn raw_hash(path: &Path) -> u64 {
    bytes_hash(path.to_string_lossy().as_bytes())
}

/// The recovery file of `document` under one key form.
pub(super) fn key_for(recovery_dir: &Path, document: &Path, form: KeyForm) -> PathBuf {
    let folded = || PathBuf::from(path_key_with(document, CaseFold::from_probe(true)));
    match form {
        KeyForm::Keyed(ignores_case) => {
            let key = path_key_with(document, CaseFold::from_probe(ignores_case));
            let name = key
                .rsplit('/')
                .next()
                .filter(|n| !n.is_empty())
                .unwrap_or("document");
            named(recovery_dir, name, bytes_hash(key.as_bytes()))
        }
        KeyForm::AsGiven => named(
            recovery_dir,
            &file_name_of(document),
            as_given_hash(document),
        ),
        KeyForm::Raw => named(recovery_dir, &file_name_of(document), raw_hash(document)),
        KeyForm::FoldedAsGiven => {
            let f = folded();
            named(recovery_dir, &file_name_of(&f), as_given_hash(&f))
        }
        KeyForm::FoldedRaw => {
            let f = folded();
            named(recovery_dir, &file_name_of(&f), raw_hash(&f))
        }
    }
}

/// Every place a copy of `document` may sit, the current form (`ignores_case`)
/// first, without repeats. Pure text: no probe, no disk.
pub(super) fn all_keys(
    recovery_dir: &Path,
    document: &Path,
    ignores_case: bool,
) -> Vec<(KeyForm, PathBuf)> {
    let forms = [
        KeyForm::Keyed(ignores_case),
        KeyForm::Keyed(!ignores_case),
        KeyForm::AsGiven,
        KeyForm::Raw,
        KeyForm::FoldedAsGiven,
        KeyForm::FoldedRaw,
    ];
    let mut keys: Vec<(KeyForm, PathBuf)> = Vec::new();
    for form in forms {
        let key = key_for(recovery_dir, document, form);
        if keys.iter().all(|(_, k)| *k != key) {
            keys.push((form, key));
        }
    }
    keys
}

/// Whether some EXISTING path other than `document` spells it up to case, in
/// any component, so a copy under a case-merging key could belong to that file
/// instead. Walks the path one component at a time, listing each folder for
/// entries whose name matches ignoring case. A folder that cannot be listed
/// for any reason but being absent counts as a variant (the safe side).
pub(super) fn has_case_variant(document: &Path) -> bool {
    use std::path::Component;
    let lower = |s: &std::ffi::OsStr| s.to_string_lossy().to_lowercase();
    // Every existing spelling of the prefix walked so far, and whether one of
    // them is the document's own spelling.
    let mut spellings: Vec<PathBuf> = vec![PathBuf::new()];
    let mut own = PathBuf::new();
    for component in document.components() {
        let name = match component {
            Component::Normal(name) => name,
            other => {
                for s in &mut spellings {
                    s.push(other.as_os_str());
                }
                own.push(other.as_os_str());
                continue;
            }
        };
        own.push(name);
        let mut next: Vec<PathBuf> = Vec::new();
        for prefix in &spellings {
            let dir = if prefix.as_os_str().is_empty() {
                Path::new(".")
            } else {
                prefix.as_path()
            };
            match std::fs::read_dir(dir) {
                Ok(entries) => {
                    let wanted = lower(name);
                    next.extend(
                        entries
                            .flatten()
                            .filter(|e| lower(&e.file_name()) == wanted)
                            .map(|e| prefix.join(e.file_name())),
                    );
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return true,
            }
        }
        spellings = next;
        if spellings.is_empty() {
            return false;
        }
    }
    spellings.iter().any(|s| *s != own)
}
