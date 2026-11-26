//! The writer's record of what it wrote (`.signex-import-ledger.json` in the
//! library root), so an update deletes only its own earlier output
//! (`docs/LIBRARY_CONVERTER_CONTRACT.md` §5).
//!
//! The file is read as untrusted input: anything wrong with it (not JSON,
//! another format token or library id, a row id that is not a UUID, a file
//! name the writer would not produce) rejects the whole record, and an
//! update then deletes nothing.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{FOOTPRINTS_DIR, SIMS_DIR, SYMBOLS_DIR};

/// File name of the ledger, in the library root.
pub const LEDGER_FILE: &str = ".signex-import-ledger.json";
const LEDGER_FORMAT: &str = "signex-import-ledger/1";

/// Ids and files one write or update produced.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    format: String,
    library_id: Uuid,
    /// Part rows written, by `row_id`.
    pub rows: BTreeSet<Uuid>,
    /// Primitive files written, relative to the root with `/`. STEP files
    /// are never listed: an update never deletes them.
    pub files: BTreeSet<String>,
}

/// What an update found in the ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerRead {
    /// Read and valid: its rows and files may be deleted.
    Trusted(Ledger),
    /// No ledger: nothing is deleted.
    Missing,
    /// Unusable, with why: nothing is deleted.
    Rejected(String),
}

impl Ledger {
    pub(super) fn new(library_id: Uuid) -> Self {
        Self {
            format: LEDGER_FORMAT.into(),
            library_id,
            ..Self::default()
        }
    }

    /// The ledger's bytes: pretty JSON, sorted, ending in one `\n`.
    pub(super) fn to_bytes(&self) -> Vec<u8> {
        let mut text = serde_json::to_string_pretty(self).expect("ledger serialises");
        text.push('\n');
        text.into_bytes()
    }
}

/// Read the ledger of the library at `root`, written for `library_id`.
pub(super) fn read(root: &Path, library_id: Uuid) -> LedgerRead {
    let path = root.join(LEDGER_FILE);
    match std::fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return LedgerRead::Missing,
        Err(e) => return LedgerRead::Rejected(format!("{LEDGER_FILE}: {e}")),
        Ok(meta) if !meta.file_type().is_file() => {
            return LedgerRead::Rejected(format!("{LEDGER_FILE} is not a regular file"));
        }
        Ok(_) => {}
    }
    let ledger: Ledger = match std::fs::read(&path)
        .map_err(|e| e.to_string())
        .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|e| e.to_string()))
    {
        Ok(ledger) => ledger,
        Err(e) => return LedgerRead::Rejected(format!("{LEDGER_FILE}: {e}")),
    };
    if ledger.format != LEDGER_FORMAT {
        return LedgerRead::Rejected(format!("{LEDGER_FILE}: format {:?}", ledger.format));
    }
    if ledger.library_id != library_id {
        return LedgerRead::Rejected(format!(
            "{LEDGER_FILE} is for library {}, not {library_id}",
            ledger.library_id
        ));
    }
    if let Some(bad) = ledger.files.iter().find(|f| !is_writer_file_name(f)) {
        return LedgerRead::Rejected(format!(
            "{LEDGER_FILE} lists {bad:?}, which is not a file the writer produces"
        ));
    }
    LedgerRead::Trusted(ledger)
}

/// Whether `rel` is a name the writer produces for a primitive:
/// `symbols/<slug or uuid>.snxsym`, `footprints/<uuid>.snxfpt`,
/// `sims/<uuid>.snxsim`. One folder, one plain file name, nothing else.
pub(super) fn is_writer_file_name(rel: &str) -> bool {
    let Some((folder, name)) = rel.split_once('/') else {
        return false;
    };
    let Some((stem, ext)) = name.rsplit_once('.') else {
        return false;
    };
    let canonical_uuid = |s: &str| Uuid::parse_str(s).is_ok_and(|u| u.to_string() == s);
    let slug = |s: &str| {
        !s.is_empty()
            && !s.starts_with('-')
            && !s.ends_with('-')
            && !s.contains("--")
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    };
    match (folder, ext) {
        (SYMBOLS_DIR, "snxsym") => slug(stem) || canonical_uuid(stem),
        (FOOTPRINTS_DIR, "snxfpt") | (SIMS_DIR, "snxsim") => canonical_uuid(stem),
        _ => false,
    }
}

/// `rel` as a path under `root`, when it is a regular file (not a link)
/// whose real location is inside `root`. Anything else is not deleted.
pub(super) fn deletable(root: &Path, rel: &str) -> Option<PathBuf> {
    if !is_writer_file_name(rel) {
        return None;
    }
    let rel_path = Path::new(rel);
    if rel_path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return None;
    }
    let real_root = root.canonicalize().ok()?;
    let folder = root.join(rel_path.parent()?);
    // The folder itself must be a real folder, not a link to elsewhere.
    if !std::fs::symlink_metadata(&folder)
        .ok()?
        .file_type()
        .is_dir()
    {
        return None;
    }
    let path = root.join(rel_path);
    if !std::fs::symlink_metadata(&path).ok()?.file_type().is_file() {
        return None;
    }
    path.canonicalize()
        .ok()
        .filter(|real| real.starts_with(&real_root))
}
