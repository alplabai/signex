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
    check_step_models(w)?;
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
        alternate_supply: draft.alternate_supply.clone(),
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

/// Refuse a footprint whose STEP attachment names a model the writer was
/// not given, embedded in its file (`[[models]]`) or added with
/// [`LibraryWriter::add_step_model`]: the library would ship the reference
/// without the model. An embedded model that does not decode to its size
/// and hash is refused too.
fn check_step_models(w: &LibraryWriter) -> Result<(), LibraryError> {
    for file in w.footprints.values() {
        file.verify_models()
            .map_err(|e| LibraryError::InvalidData(e.to_string()))?;
        for fp in &file.footprints {
            let Some(step) = &fp.step_attachment else {
                continue;
            };
            let embedded = file.models.get(&step.content_hash).is_some();
            if !embedded && !w.step_models.contains_key(&step.content_hash) {
                return Err(LibraryError::InvalidData(format!(
                    "footprint {} ({}) attaches STEP model {} ({}), which is neither embedded \
                     in its file nor added with add_step_model",
                    fp.name, fp.uuid, step.content_hash, step.filename
                )));
            }
        }
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
