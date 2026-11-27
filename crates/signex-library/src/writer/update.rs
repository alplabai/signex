//! [`LibraryWriter::write_update`]: overwrite a library the same converter
//! wrote before, per `docs/LIBRARY_CONVERTER_CONTRACT.md` §5.
//!
//! The library is a **snapshot** of its source, but only for what the
//! writer wrote itself. The writer records the rows and primitive files it
//! wrote in its ledger (`ledger.rs`); an update deletes the recorded ones it
//! no longer has, and overwrites the ones it has again, user edits
//! included. Rows, primitives and files a user added are not in the ledger
//! and survive byte for byte. A missing or rejected ledger deletes nothing.
//! An import file is never written over a file the previous ledger does not
//! list: a symbol envelope moves to `<file_uuid>.snxsym`, any other clash
//! refuses the update. A recorded file a kept user row still uses is kept,
//! and a kept user row whose part number an import row holds refuses the
//! update; both refusals happen before anything is written.
//! STEP files are never deleted. The writer never commits, so only what was
//! committed survives in git history; refusing a library with uncommitted
//! changes is the app's job before it runs an update (#3114).
//!
//! - A row whose content is unchanged keeps its stored cells byte for byte.
//! - A changed row keeps its lifecycle `state` and `created`, gets the old
//!   version with the patch number bumped, and the writer's update time.
//! - A surviving row keeps its cached prices unless the writer gives its own.
//! - The manifest's `[mode]`, `[workflow]` and `[users]` stay as they are
//!   (library administration, not converter output); the rest comes from the
//!   writer. Keys this build does not model survive, as with every save.
//! - A stored row whose `row_id` does not read refuses the update: it could
//!   be neither matched nor kept.
//!
//! The library's write lock is held from reading the `.snxlib` to the new
//! ledger, so a running Signex cannot write in between. Files are replaced
//! one by one with `atomic_write`, not as one directory swap: primitives
//! first, then the `.snxlib`, then deletions, then the ledger, so the
//! `.snxlib` never names a file that is not there yet and a re-run after a
//! failure still knows what the previous import wrote. A failure part way leaves old and new files
//! mixed; the error says so, and running the update again completes it.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use uuid::Uuid;

use super::ledger::{self, LEDGER_FILE, Ledger, LedgerRead};
use super::render::{hashed, header, render};
use super::update_guard;
use super::{LibraryWriter, TableDraft};
use crate::adapter::LibraryError;
use crate::adapters::local_git::{component_to_library_row, library_row_to_component};
use crate::library_file::{LibraryFile, LibraryRow, LibraryTable};
use crate::pricing::RowPricing;
use crate::tables::ALTERNATE_SYMBOLS_COLUMN;

/// What [`LibraryWriter::write_update`] did.
/// How an update read the writer's ledger.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum LedgerState {
    /// Read and valid: what it recorded and the writer no longer has was
    /// deleted.
    Trusted,
    /// There was none: nothing was deleted.
    #[default]
    Missing,
    /// Unusable, with why: nothing was deleted.
    Rejected(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpdateSummary {
    /// The `.snxlib` that was updated.
    pub snxlib: PathBuf,
    pub rows_added: usize,
    pub rows_changed: usize,
    pub rows_unchanged: usize,
    /// Rows the ledger recorded that the writer no longer has: deleted.
    pub rows_removed: usize,
    /// Rows not in the ledger (a user's) that the writer does not have: kept.
    pub rows_kept: usize,
    /// What the ledger allowed: nothing is deleted unless it is trusted.
    pub ledger: LedgerState,
    /// Recorded files the writer no longer has, kept because a kept user
    /// row still uses a primitive in them.
    pub files_kept_in_use: Vec<String>,
    /// Symbol envelopes written under `<file_uuid>.snxsym` because a file
    /// the previous import did not write held their usual name: `(name,
    /// written as)`.
    pub files_moved: Vec<(String, String)>,
    /// Primitive and model files written because their bytes changed.
    pub files_written: usize,
    /// Primitive and model files deleted because the writer no longer has them.
    pub files_deleted: usize,
}

pub(super) fn write_update(w: &LibraryWriter, dir: &Path) -> Result<UpdateSummary, LibraryError> {
    let snxlib = single_snxlib(dir)?;
    let _write_lock = crate::adapters::local_git::lock_library_file(&snxlib)?;
    let text = std::fs::read_to_string(&snxlib)?;
    let mut existing = LibraryFile::parse(&text)?;
    if existing.manifest.library_id != w.manifest.library_id {
        return Err(LibraryError::Conflict(format!(
            "{} is library {}, not {}; --update only overwrites the same library",
            snxlib.display(),
            existing.manifest.library_id,
            w.manifest.library_id
        )));
    }
    let mut files = render(w)?;
    files.remove(Path::new(&w.snxlib_file_name()));

    let mut summary = UpdateSummary {
        snxlib: snxlib.clone(),
        ..UpdateSummary::default()
    };
    let (mut old_rows, unreadable_ids) = stored_rows(&existing);
    if unreadable_ids > 0 {
        return Err(LibraryError::InvalidData(format!(
            "{} holds {unreadable_ids} row(s) whose row_id is not a UUID; an update would \
             drop them, so nothing was changed. Fix or remove those rows first",
            snxlib.display()
        )));
    }
    let old_pricing = stored_pricing(&existing);
    let previous = match ledger::read(dir, w.manifest.library_id) {
        LedgerRead::Trusted(ledger) => {
            summary.ledger = LedgerState::Trusted;
            Some(ledger)
        }
        LedgerRead::Missing => None,
        LedgerRead::Rejected(why) => {
            tracing::warn!(target: "signex::library", "{why}; the update deletes nothing");
            summary.ledger = LedgerState::Rejected(why);
            None
        }
    };
    let now = w.update_time.unwrap_or_else(chrono::Utc::now);
    let mut tables = BTreeMap::new();
    for (name, draft) in &w.tables {
        let old_table = existing.tables.get(name);
        let table = merged_table(
            draft,
            old_table,
            &mut old_rows,
            &old_pricing,
            now,
            &mut summary,
        )?;
        tables.insert(name.clone(), table);
    }
    let kept_rows = keep_unrecorded_rows(
        &existing,
        &old_rows,
        previous.as_ref(),
        &mut tables,
        &mut summary,
    );
    update_guard::check_kept_part_numbers(&kept_rows, w)?;
    summary.files_moved =
        update_guard::relocate_unledgered(dir, w, &mut files, previous.as_ref(), &summary.ledger)?;
    existing.tables = tables;
    let kept = existing.manifest.clone();
    existing.manifest = w.manifest.clone();
    existing.manifest.mode = kept.mode;
    existing.manifest.workflow = kept.workflow;
    existing.manifest.users = kept.users;
    let new_text = existing.write()?;

    let mut ledger = w.ledger(&files);
    let in_use = update_guard::KeptRefs::of(&kept_rows);
    let mut gone = Vec::new();
    for rel in previous.iter().flat_map(|p| &p.files) {
        if ledger.files.contains(rel) {
            continue;
        }
        if in_use.uses(dir, rel) {
            // Still the import's file: a later update may delete it.
            summary.files_kept_in_use.push(rel.clone());
            ledger.files.insert(rel.clone());
        } else {
            gone.push(rel.clone());
        }
    }
    let first_ledger = previous
        .as_ref()
        .map_or_else(|| ledger.clone(), |p| p.union(&ledger));
    let ledgers = (first_ledger.to_bytes(), ledger.to_bytes());
    let done = (gone.as_slice(), ledgers);
    apply(dir, &snxlib, &files, (&text, &new_text), done, &mut summary).map_err(|e| {
        LibraryError::Backend(format!(
            "the update of {} stopped part way ({e}); the library now mixes old and new \
             files. Run the update again to complete it",
            dir.display()
        ))
    })?;
    Ok(summary)
}

/// The union ledger, then what changed, then the `.snxlib`, then delete
/// what is gone, then the exact new ledger.
fn apply(
    dir: &Path,
    snxlib: &Path,
    files: &BTreeMap<PathBuf, Vec<u8>>,
    (text, new_text): (&str, &str),
    (gone, (first_ledger, ledger)): (&[String], (Vec<u8>, Vec<u8>)),
    summary: &mut UpdateSummary,
) -> Result<(), LibraryError> {
    // No target may be, or lie behind, a symbolic link: checked for all of
    // them before anything is written, so a refusal changes nothing.
    update_guard::refuse_links(dir, Path::new(LEDGER_FILE))?;
    if let Ok(rel) = snxlib.strip_prefix(dir) {
        update_guard::refuse_links(dir, rel)?;
    }
    for rel in files.keys() {
        update_guard::refuse_links(dir, rel)?;
    }
    // First the union of the old and new ledgers, so a crash from here on
    // leaves every file this update writes recorded, none orphaned.
    let ledger_path = dir.join(LEDGER_FILE);
    write_if_changed(&ledger_path, &first_ledger)?;
    for (rel, bytes) in files {
        let path = dir.join(rel);
        if std::fs::read(&path).ok().as_deref() != Some(bytes.as_slice()) {
            signex_types::atomic_io::atomic_write(&path, bytes)?;
            summary.files_written += 1;
        }
    }
    if new_text != text {
        signex_types::atomic_io::atomic_write_existing_dir(snxlib, new_text.as_bytes())?;
    }
    summary.files_deleted = delete_recorded(dir, gone)?;
    // Last, the exact ledger: until then a re-run still knows what to delete.
    write_if_changed(&ledger_path, &ledger)
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<(), LibraryError> {
    if std::fs::read(path).ok().as_deref() != Some(bytes) {
        signex_types::atomic_io::atomic_write_existing_dir(path, bytes)?;
    }
    Ok(())
}

/// The one `.snxlib` directly in `dir`.
fn single_snxlib(dir: &Path) -> Result<PathBuf, LibraryError> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_file() && path.extension().is_some_and(|e| e == "snxlib") {
            found.push(path);
        }
    }
    match found.as_slice() {
        [one] => Ok(one.clone()),
        [] => Err(LibraryError::NotFound(format!(
            "no .snxlib in {}",
            dir.display()
        ))),
        _ => Err(LibraryError::Conflict(format!(
            "{} holds several .snxlib files",
            dir.display()
        ))),
    }
}

/// Every stored row by its `row_id`, and how many rows have a `row_id`
/// that does not read as a UUID.
fn stored_rows(file: &LibraryFile) -> (HashMap<Uuid, Arc<LibraryRow>>, usize) {
    let mut rows = HashMap::new();
    let mut unreadable = 0;
    for r in file.tables.values().flat_map(|t| &t.rows) {
        match r.cells.get("row_id").and_then(|id| id.parse::<Uuid>().ok()) {
            Some(id) => {
                rows.insert(id, Arc::clone(r));
            }
            None => unreadable += 1,
        }
    }
    (rows, unreadable)
}

/// Every stored row's cached prices, by `row_id`, whichever table holds it.
fn stored_pricing(file: &LibraryFile) -> HashMap<Uuid, RowPricing> {
    file.tables
        .values()
        .flat_map(|t| &t.pricing)
        .filter_map(|(id, p)| Some((id.parse::<Uuid>().ok()?, p.clone())))
        .collect()
}

fn merged_table(
    draft: &TableDraft,
    old_table: Option<&LibraryTable>,
    old_rows: &mut HashMap<Uuid, Arc<LibraryRow>>,
    old_pricing: &HashMap<Uuid, RowPricing>,
    now: chrono::DateTime<chrono::Utc>,
    summary: &mut UpdateSummary,
) -> Result<LibraryTable, LibraryError> {
    let mut rows = Vec::with_capacity(draft.rows.len());
    let mut pricing = draft.pricing.clone();
    for row in &draft.rows {
        let mut new = hashed(row)?;
        let key = new.row_id.to_string();
        if !pricing.contains_key(&key)
            && let Some(kept) = old_pricing.get(&new.row_id)
        {
            let mut kept = kept.clone();
            crate::pricing::prune_row_pricing(&new, &mut kept);
            kept.retain(|_, per_sku| !per_sku.is_empty());
            if !kept.is_empty() {
                pricing.insert(key, kept);
            }
        }
        let Some(old) = old_rows.remove(&new.row_id) else {
            summary.rows_added += 1;
            rows.push(Arc::new(component_to_library_row(&new)?));
            continue;
        };
        let old_row = library_row_to_component(&old)?;
        let old_hash = crate::hash::hash_row_content(&old_row)?;
        new.state = old_row.state;
        new.refresh_content_hash()?;
        if new.content_hash == old_hash {
            summary.rows_unchanged += 1;
            rows.push(old);
            continue;
        }
        new.created = old_row.created;
        new.updated = now;
        new.version = bump_patch(&old_row.version).unwrap_or(new.version);
        summary.rows_changed += 1;
        rows.push(Arc::new(component_to_library_row(&new)?));
    }
    // Column 23 is never removed once a table has it (format §2.3).
    let with_alternates = old_table
        .is_some_and(|t| t.columns.iter().any(|c| c == ALTERNATE_SYMBOLS_COLUMN))
        || rows
            .iter()
            .any(|r| r.cells.contains_key(ALTERNATE_SYMBOLS_COLUMN));
    let column_types = match old_table {
        Some(t) if draft.column_types.is_empty() => t.column_types.clone(),
        _ => draft.column_types.clone(),
    };
    Ok(LibraryTable {
        columns: header(with_alternates),
        rows,
        column_types,
        pricing,
    })
}

/// `X.Y.Z` with `Z` raised by one; `None` when `version` is not that form.
pub(super) fn bump_patch(version: &str) -> Option<String> {
    let mut parts = version.trim().split('.');
    let (major, minor, patch) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    let [major, minor, patch]: [u64; 3] = [
        major.parse().ok()?,
        minor.parse().ok()?,
        patch.parse().ok()?,
    ];
    Some(format!("{major}.{minor}.{}", patch.checked_add(1)?))
}

/// Delete the primitive files the previous ledger recorded that the writer
/// no longer produces: only regular files inside `dir` under a name the
/// writer gives (`ledger::deletable`); links and anything else stay.
fn delete_recorded(dir: &Path, gone: &[String]) -> Result<usize, LibraryError> {
    let mut deleted = 0;
    for rel in gone {
        let Some(path) = ledger::deletable(dir, rel) else {
            tracing::warn!(target: "signex::library", "not deleting {rel}: not a plain file of the library");
            continue;
        };
        match std::fs::remove_file(&path) {
            Ok(()) => deleted += 1,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(deleted)
}

/// Put back the stored rows the writer does not have and its ledger does
/// not record (a user's), in the table that held them, cells and prices
/// unchanged. Recorded ones are counted as removed.
fn keep_unrecorded_rows(
    existing: &LibraryFile,
    leftover: &HashMap<Uuid, Arc<LibraryRow>>,
    previous: Option<&Ledger>,
    tables: &mut BTreeMap<String, LibraryTable>,
    summary: &mut UpdateSummary,
) -> Vec<Arc<LibraryRow>> {
    let mut kept = Vec::new();
    for (name, old_table) in &existing.tables {
        for row in &old_table.rows {
            let Some(id) = row.cells.get("row_id").and_then(|t| t.parse::<Uuid>().ok()) else {
                continue;
            };
            if !leftover.contains_key(&id) {
                continue;
            }
            if previous.is_some_and(|l| l.rows.contains(&id)) {
                summary.rows_removed += 1;
                continue;
            }
            let table = tables.entry(name.clone()).or_insert_with(|| LibraryTable {
                columns: old_table.columns.clone(),
                rows: Vec::new(),
                column_types: old_table.column_types.clone(),
                pricing: BTreeMap::new(),
            });
            if row.cells.contains_key(ALTERNATE_SYMBOLS_COLUMN)
                && !table.columns.iter().any(|c| c == ALTERNATE_SYMBOLS_COLUMN)
            {
                table.columns.push(ALTERNATE_SYMBOLS_COLUMN.to_string());
            }
            if let Some(prices) = old_table.pricing.get(&id.to_string()) {
                table.pricing.insert(id.to_string(), prices.clone());
            }
            table.rows.push(Arc::clone(row));
            kept.push(Arc::clone(row));
            summary.rows_kept += 1;
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::bump_patch;

    #[test]
    fn bump_patch_raises_only_the_patch_number() {
        assert_eq!(bump_patch("1.2.3").as_deref(), Some("1.2.4"));
        assert_eq!(bump_patch("0.0.9").as_deref(), Some("0.0.10"));
        assert_eq!(bump_patch("1.2"), None);
        assert_eq!(bump_patch("1.2.3.4"), None);
        assert_eq!(bump_patch("1.x.3"), None);
    }
}
