//! What keeps an update off a user's own work: files at an import path the
//! previous ledger does not list, part numbers a kept user row already holds,
//! and recorded primitives a kept user row still uses.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use uuid::Uuid;

use super::LedgerState;
use super::ledger::{LEDGER_FILE, Ledger};
use super::{LibraryWriter, SYMBOLS_DIR};
use crate::adapter::LibraryError;
use crate::adapters::local_git::library_row_to_component;
use crate::library_file::LibraryRow;
use crate::primitive::SymbolFile;

fn rel_text(rel: &Path) -> String {
    rel.to_string_lossy().replace('\\', "/")
}

/// Whether the file at `rel` is not the import's: it exists, the previous
/// ledger (if trusted) does not list it, and it differs from `bytes`. A file
/// byte-identical to what the import writes is taken as the import's.
fn is_foreign(dir: &Path, rel: &str, bytes: &[u8], previous: Option<&Ledger>) -> bool {
    let Ok(on_disk) = std::fs::read(dir.join(rel)) else {
        return dir.join(rel).exists();
    };
    on_disk != bytes && !previous.is_some_and(|l| l.files.contains(rel))
}

/// Never overwrite a file the previous import did not write. A symbol
/// envelope whose name is taken moves to `symbols/<file_uuid>.snxsym`, as
/// the editor names a new envelope whose name is taken; any other clash, or
/// one that also clashes there, refuses the update before anything is
/// written. Returns `(from, to)` for each moved envelope.
pub(super) fn relocate_unledgered(
    dir: &Path,
    w: &LibraryWriter,
    files: &mut BTreeMap<PathBuf, Vec<u8>>,
    previous: Option<&Ledger>,
    state: &LedgerState,
) -> Result<Vec<(String, String)>, LibraryError> {
    let why_unlisted = match state {
        LedgerState::Trusted => "the import ledger does not list it".to_string(),
        LedgerState::Missing => format!("there is no import ledger ({LEDGER_FILE})"),
        LedgerState::Rejected(why) => format!("the import ledger was rejected: {why}"),
    };
    let mut moved = Vec::new();
    let clashes: Vec<PathBuf> = files
        .iter()
        .filter(|(rel, bytes)| is_foreign(dir, &rel_text(rel), bytes, previous))
        .map(|(rel, _)| rel.clone())
        .collect();
    for rel in clashes {
        let text = rel_text(&rel);
        let refuse = |why: &str| {
            LibraryError::Conflict(format!(
                "{text} in {} is not a file the previous import wrote ({why_unlisted}){why}; \
                 the update would overwrite it, so nothing was changed. Restore \
                 {LEDGER_FILE} from git if it was lost or damaged, or import into a fresh \
                 folder",
                dir.display()
            ))
        };
        let Some(name) = text.strip_prefix(&format!("{SYMBOLS_DIR}/")) else {
            return Err(refuse(""));
        };
        let Some(envelope) = w.symbol_files.get(name) else {
            return Err(refuse(""));
        };
        if shares_symbols(&dir.join(&rel), envelope) {
            return Err(refuse(" and holds symbols the import writes"));
        }
        let to = PathBuf::from(SYMBOLS_DIR).join(format!("{}.snxsym", envelope.file_uuid));
        if files.contains_key(&to) {
            return Err(refuse(" and its envelope's own name is taken too"));
        }
        let bytes = files.remove(&rel).expect("listed above");
        if is_foreign(dir, &rel_text(&to), &bytes, previous) {
            return Err(refuse(" and neither is the envelope's own name"));
        }
        moved.push((text, rel_text(&to)));
        files.insert(to, bytes);
    }
    Ok(moved)
}

/// Whether the envelope at `path` holds a symbol uuid `envelope` holds:
/// moving the import's envelope next to it would give the library the
/// symbol twice.
fn shares_symbols(path: &Path, envelope: &SymbolFile) -> bool {
    let Some(theirs) = std::fs::read(path)
        .ok()
        .and_then(|b| SymbolFile::from_bytes(&b).ok())
    else {
        return false;
    };
    let ours: HashSet<Uuid> = envelope.symbols.iter().map(|s| s.uuid).collect();
    theirs.symbols.iter().any(|s| ours.contains(&s.uuid))
}

/// Refuse a kept user row whose part number an import row also holds
/// (trimmed, non-blank), naming both rows and the part number.
pub(super) fn check_kept_part_numbers(
    kept: &[Arc<LibraryRow>],
    w: &LibraryWriter,
) -> Result<(), LibraryError> {
    let imported: BTreeMap<&str, Uuid> = w
        .tables
        .values()
        .flat_map(|t| &t.rows)
        .map(|r| (r.internal_pn.as_str().trim(), r.row_id))
        .filter(|(pn, _)| !pn.is_empty())
        .collect();
    for row in kept {
        let pn = row.cells.get("internal_pn").map(|p| p.trim()).unwrap_or("");
        if let Some(import_row) = imported.get(pn).filter(|_| !pn.is_empty()) {
            let user_row = row.cells.get("row_id").map(String::as_str).unwrap_or("?");
            return Err(LibraryError::Conflict(format!(
                "internal_pn {pn:?} is held by the user's row {user_row} and by the import's \
                 row {import_row}; the update would make it ambiguous, so nothing was changed"
            )));
        }
    }
    Ok(())
}

/// Primitive uuids the kept user rows reference.
#[derive(Default)]
pub(super) struct KeptRefs(HashSet<Uuid>);

impl KeptRefs {
    pub(super) fn of(kept: &[Arc<LibraryRow>]) -> Self {
        let mut refs = HashSet::new();
        for row in kept {
            let Ok(r) = library_row_to_component(row) else {
                continue;
            };
            refs.insert(r.symbol_ref.uuid);
            refs.extend(r.alternate_symbols.iter().map(|a| a.symbol_ref.uuid));
            refs.extend(r.footprint_bindings().iter().map(|b| b.footprint.uuid));
            refs.extend(r.sim_ref.map(|s| s.uuid));
        }
        Self(refs)
    }

    /// Whether the recorded file `rel` under `dir` holds a primitive a kept
    /// row uses: a footprint or sim by its file name, a symbol by the uuids
    /// its envelope holds.
    pub(super) fn uses(&self, dir: &Path, rel: &str) -> bool {
        if self.0.is_empty() {
            return false;
        }
        let stem = Path::new(rel)
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| Uuid::parse_str(s).ok());
        if rel.starts_with(&format!("{SYMBOLS_DIR}/")) {
            return std::fs::read(dir.join(rel))
                .ok()
                .and_then(|b| SymbolFile::from_bytes(&b).ok())
                .is_some_and(|f| f.symbols.iter().any(|s| self.0.contains(&s.uuid)));
        }
        stem.is_some_and(|u| self.0.contains(&u))
    }
}
