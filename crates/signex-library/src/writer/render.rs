//! The bytes of every file of a [`LibraryWriter`], and the whole-directory
//! write.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use uuid::Uuid;

use super::{FOOTPRINTS_DIR, LibraryWriter, SIMS_DIR, STEP_DIR, SYMBOLS_DIR, TableDraft};
use crate::adapter::LibraryError;
use crate::adapters::local_git::{check_table_name, component_to_library_row};
use crate::component::ComponentRow;
use crate::library_file::{LibraryFile, LibraryTable};
use crate::tables::{ALTERNATE_SYMBOLS_COLUMN, TABLE_HEADER};

pub(super) fn render(w: &LibraryWriter) -> Result<BTreeMap<PathBuf, Vec<u8>>, LibraryError> {
    check_file_stem(&w.file_stem)?;
    check_identities(w)?;
    let mut out = BTreeMap::new();
    let mut file = LibraryFile {
        manifest: w.manifest.clone(),
        tables: BTreeMap::new(),
        layout: Default::default(),
    };
    for (name, draft) in &w.tables {
        check_table_name(name)?;
        file.tables.insert(name.clone(), table(draft)?);
    }
    out.insert(
        PathBuf::from(w.snxlib_file_name()),
        file.write().map_err(LibraryError::from)?.into_bytes(),
    );
    for (name, sym) in &w.symbol_files {
        let text = sym
            .to_toml_string()
            .map_err(|e| LibraryError::InvalidData(format!("symbols/{name}: {e}")))?;
        out.insert(Path::new(SYMBOLS_DIR).join(name), text.into_bytes());
    }
    for (uuid, fp) in &w.footprints {
        let text = fp
            .to_toml_string()
            .map_err(|e| LibraryError::InvalidData(format!("footprint {uuid}: {e}")))?;
        out.insert(
            Path::new(FOOTPRINTS_DIR).join(format!("{uuid}.snxfpt")),
            text.into_bytes(),
        );
    }
    for (uuid, sim) in &w.sims {
        let text = sim
            .to_toml_string()
            .map_err(|e| LibraryError::InvalidData(format!("sim model {uuid}: {e}")))?;
        out.insert(
            Path::new(SIMS_DIR).join(format!("{uuid}.snxsim")),
            text.into_bytes(),
        );
    }
    for (hash, bytes) in &w.step_models {
        out.insert(
            Path::new(STEP_DIR).join(format!("{hash}.step")),
            bytes.clone(),
        );
    }
    Ok(out)
}

/// The rows of `draft` as a `.snxlib` table: hashes refreshed, each row
/// validated and converted as the adapter does, the optional
/// `alternate_symbols` column present when a row uses it.
pub(super) fn table(draft: &TableDraft) -> Result<LibraryTable, LibraryError> {
    let mut rows = Vec::with_capacity(draft.rows.len());
    for row in &draft.rows {
        rows.push(Arc::new(component_to_library_row(&hashed(row)?)?));
    }
    let with_alternates = rows
        .iter()
        .any(|r| r.cells.contains_key(ALTERNATE_SYMBOLS_COLUMN));
    Ok(LibraryTable {
        columns: header(with_alternates),
        rows,
        column_types: draft.column_types.clone(),
        pricing: draft.pricing.clone(),
    })
}

pub(super) fn header(with_alternate_symbols: bool) -> Vec<String> {
    let extra = with_alternate_symbols.then_some(ALTERNATE_SYMBOLS_COLUMN);
    TABLE_HEADER
        .iter()
        .copied()
        .chain(extra)
        .map(str::to_string)
        .collect()
}

pub(super) fn hashed(row: &ComponentRow) -> Result<ComponentRow, LibraryError> {
    let mut row = row.clone();
    row.refresh_content_hash()?;
    Ok(row)
}

fn check_file_stem(stem: &str) -> Result<(), LibraryError> {
    let bad = stem.trim().is_empty()
        || stem.starts_with('.')
        || stem.chars().any(|c| {
            matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()
        });
    if bad {
        return Err(LibraryError::InvalidData(format!(
            "{stem:?} cannot name a .snxlib file"
        )));
    }
    Ok(())
}

/// Refuse what the library would hold twice: a row id, a part number
/// (trimmed, non-blank, as `InternalPn::clashes`), a symbol uuid.
fn check_identities(w: &LibraryWriter) -> Result<(), LibraryError> {
    let mut row_ids = HashSet::new();
    let mut pns: HashMap<&str, Uuid> = HashMap::new();
    for row in w.tables.values().flat_map(|t| &t.rows) {
        if !row_ids.insert(row.row_id) {
            return Err(LibraryError::Conflict(format!(
                "row {} is added twice",
                row.row_id
            )));
        }
        let pn = row.internal_pn.as_str().trim();
        if pn.is_empty() {
            continue;
        }
        if let Some(other) = pns.insert(pn, row.row_id) {
            return Err(LibraryError::Conflict(format!(
                "internal_pn {pn:?} is held by rows {other} and {}",
                row.row_id
            )));
        }
    }
    let mut symbols = HashSet::new();
    for (name, file) in &w.symbol_files {
        for sym in &file.symbols {
            if !symbols.insert(sym.uuid) {
                return Err(LibraryError::Conflict(format!(
                    "symbol {} is added twice (again in symbols/{name})",
                    sym.uuid
                )));
            }
        }
    }
    Ok(())
}

/// Write `files` into a fresh sibling of `dir`, then rename it to `dir`.
/// `dir` must not exist or be an empty folder.
pub(super) fn write_whole(
    dir: &Path,
    files: &BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), LibraryError> {
    let existed = match std::fs::read_dir(dir) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                return Err(LibraryError::Conflict(format!(
                    "{} is not empty; a new library is written only into a missing or empty folder",
                    dir.display()
                )));
            }
            true
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(e.into()),
    };
    let parent = dir
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let staging = staging_dir(parent, dir);
    std::fs::create_dir(&staging)?;
    let result = fill_and_move(&staging, dir, existed, files);
    if result.is_err()
        && staging.exists()
        && let Err(e) = std::fs::remove_dir_all(&staging)
    {
        tracing::warn!(
            target: "signex::library",
            "could not remove the staging folder {}: {e}",
            staging.display()
        );
    }
    result
}

fn fill_and_move(
    staging: &Path,
    dir: &Path,
    existed: bool,
    files: &BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), LibraryError> {
    for (rel, bytes) in files {
        signex_types::atomic_io::atomic_write(&staging.join(rel), bytes)?;
    }
    if existed {
        // Renaming over a folder is not portable; the empty one goes first.
        std::fs::remove_dir(dir)?;
    }
    std::fs::rename(staging, dir)?;
    Ok(())
}

fn staging_dir(parent: &Path, dir: &Path) -> PathBuf {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "library".into());
    let nonce = Uuid::now_v7().simple();
    parent.join(format!(".{name}.writing-{nonce}"))
}
