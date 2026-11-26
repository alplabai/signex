//! [`LibraryWriter::write_update`]: overwrite a library the same converter
//! wrote before, per `docs/LIBRARY_CONVERTER_CONTRACT.md` §5.
//!
//! - A row whose content is unchanged keeps its stored cells byte for byte.
//! - A changed row keeps its lifecycle `state` and `created`, gets the old
//!   version with the patch number bumped, and the writer's update time.
//! - Rows, envelopes and STEP files no longer in the writer are deleted.
//! - The manifest's `[mode]`, `[workflow]` and `[users]` stay as they are
//!   (library administration, not converter output); the rest comes from the
//!   writer. Keys this build does not model survive, as with every save.
//!
//! Files are replaced one by one with `atomic_write`, not as one directory
//! swap: primitives first, then the `.snxlib`, then deletions, so the
//! `.snxlib` never names a file that is not there yet.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use uuid::Uuid;

use super::render::{hashed, header, render};
use super::{FOOTPRINTS_DIR, LibraryWriter, SIMS_DIR, STEP_DIR, SYMBOLS_DIR, TableDraft};
use crate::adapter::LibraryError;
use crate::adapters::local_git::{component_to_library_row, library_row_to_component};
use crate::library_file::{LibraryFile, LibraryRow, LibraryTable};
use crate::tables::ALTERNATE_SYMBOLS_COLUMN;

/// What [`LibraryWriter::write_update`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpdateSummary {
    /// The `.snxlib` that was updated.
    pub snxlib: PathBuf,
    pub rows_added: usize,
    pub rows_changed: usize,
    pub rows_unchanged: usize,
    pub rows_removed: usize,
    /// Primitive and model files written because their bytes changed.
    pub files_written: usize,
    /// Primitive and model files deleted because the writer no longer has them.
    pub files_deleted: usize,
}

pub(super) fn write_update(w: &LibraryWriter, dir: &Path) -> Result<UpdateSummary, LibraryError> {
    let snxlib = single_snxlib(dir)?;
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
    let mut old_rows = stored_rows(&existing);
    let now = w.update_time.unwrap_or_else(chrono::Utc::now);
    let mut tables = BTreeMap::new();
    for (name, draft) in &w.tables {
        let old_table = existing.tables.get(name);
        let table = merged_table(draft, old_table, &mut old_rows, now, &mut summary)?;
        tables.insert(name.clone(), table);
    }
    summary.rows_removed = old_rows.len();
    existing.tables = tables;
    let kept = existing.manifest.clone();
    existing.manifest = w.manifest.clone();
    existing.manifest.mode = kept.mode;
    existing.manifest.workflow = kept.workflow;
    existing.manifest.users = kept.users;
    let new_text = existing.write()?;

    for (rel, bytes) in &files {
        let path = dir.join(rel);
        if std::fs::read(&path).ok().as_deref() != Some(bytes.as_slice()) {
            signex_types::atomic_io::atomic_write(&path, bytes)?;
            summary.files_written += 1;
        }
    }
    if new_text != text {
        signex_types::atomic_io::atomic_write_existing_dir(&snxlib, new_text.as_bytes())?;
    }
    summary.files_deleted = delete_stale(dir, &files)?;
    Ok(summary)
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

/// Every stored row by its `row_id`; rows whose id does not parse are
/// dropped with the update (they cannot match a writer row).
fn stored_rows(file: &LibraryFile) -> HashMap<Uuid, Arc<LibraryRow>> {
    file.tables
        .values()
        .flat_map(|t| &t.rows)
        .filter_map(|r| {
            let id = r.cells.get("row_id")?.parse::<Uuid>().ok()?;
            Some((id, Arc::clone(r)))
        })
        .collect()
}

fn merged_table(
    draft: &TableDraft,
    old_table: Option<&LibraryTable>,
    old_rows: &mut HashMap<Uuid, Arc<LibraryRow>>,
    now: chrono::DateTime<chrono::Utc>,
    summary: &mut UpdateSummary,
) -> Result<LibraryTable, LibraryError> {
    let mut rows = Vec::with_capacity(draft.rows.len());
    for row in &draft.rows {
        let mut new = hashed(row)?;
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
        pricing: draft.pricing.clone(),
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

/// Delete the envelopes and STEP files in the writer's folders that the
/// writer no longer has. Only the four folders and their own extensions.
fn delete_stale(dir: &Path, keep: &BTreeMap<PathBuf, Vec<u8>>) -> Result<usize, LibraryError> {
    let keep: HashSet<&Path> = keep.keys().map(PathBuf::as_path).collect();
    let mut deleted = 0;
    for (sub, ext) in [
        (SYMBOLS_DIR, "snxsym"),
        (FOOTPRINTS_DIR, "snxfpt"),
        (SIMS_DIR, "snxsim"),
        (STEP_DIR, "step"),
    ] {
        let folder = dir.join(sub);
        let entries = match std::fs::read_dir(&folder) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        for entry in entries {
            let path = entry?.path();
            let Some(name) = path.file_name() else {
                continue;
            };
            let rel = Path::new(sub).join(name);
            if path.is_file()
                && path.extension().is_some_and(|e| e == ext)
                && !keep.contains(rel.as_path())
            {
                std::fs::remove_file(&path)?;
                deleted += 1;
            }
        }
    }
    Ok(deleted)
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
