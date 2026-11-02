//! Off-thread read behind the details section: the row, its symbol and its
//! footprint. Pure IO, no app state, so it runs in `spawn_blocking`.

use std::path::PathBuf;

use signex_library::LibraryAdapter as _;
use signex_library::RowId;
use signex_library::primitive::PrimitiveRef;
use uuid::Uuid;

use super::ComponentDetails;
use crate::library::state::ComponentsPanelRowKey;

/// Path of the open library a reference points into, from the
/// `(library id, root)` map the caller built from the mounted libraries.
fn owner_of(libraries: &[(Uuid, PathBuf)], reference: &PrimitiveRef) -> Result<PathBuf, String> {
    libraries
        .iter()
        .find(|(id, _)| *id == reference.library_id)
        .map(|(_, path)| path.clone())
        .ok_or_else(|| format!("its library {} is not open", reference.library_id))
}

fn open(
    path: &std::path::Path,
) -> Result<signex_library::adapters::local_git::LocalGitAdapter, String> {
    signex_library::adapters::local_git::LocalGitAdapter::open(path)
        .map_err(|e| format!("Could not open {}: {e}", path.display()))
}

/// Read what the details section shows for `key`. The row must read; a
/// symbol or footprint that cannot is reported in `notes` and left out.
pub fn load_details(
    key: &ComponentsPanelRowKey,
    libraries: &[(Uuid, PathBuf)],
) -> Result<ComponentDetails, String> {
    let own = open(&key.library_path)?;
    let row_id: RowId = key.row_id;
    let row = own
        .read_row(&key.table, row_id)
        .map_err(|e| format!("Could not read row {row_id} of table {}: {e}", key.table))?;

    let mut notes = Vec::new();
    let reference = row.symbol_ref;
    let read = if reference.library_id == own.library_id() {
        own.get_symbol(reference.uuid).map_err(|e| e.to_string())
    } else {
        owner_of(libraries, &reference)
            .and_then(|p| open(&p))
            .and_then(|a| a.get_symbol(reference.uuid).map_err(|e| e.to_string()))
    };
    let symbol = match read {
        Ok(symbol) => Some(symbol.to_lib_symbol(crate::library::place::lib_id_of(&reference))),
        Err(why) => {
            notes.push(format!("Symbol not available: {why}"));
            None
        }
    };
    let footprint = row.footprint_ref.and_then(|reference| {
        let read = if reference.library_id == own.library_id() {
            own.get_footprint(reference.uuid).map_err(|e| e.to_string())
        } else {
            owner_of(libraries, &reference)
                .and_then(|p| open(&p))
                .and_then(|a| a.get_footprint(reference.uuid).map_err(|e| e.to_string()))
        };
        match read {
            Ok(footprint) => Some(footprint),
            Err(why) => {
                notes.push(format!("Footprint not available: {why}"));
                None
            }
        }
    });

    Ok(ComponentDetails {
        internal_pn: row.internal_pn.to_string(),
        class: row.class.as_str().to_owned(),
        lifecycle: format!("{:?}", row.state),
        manufacturer: row.primary_mpn.manufacturer.clone(),
        mpn: row.primary_mpn.mpn.clone(),
        description: row.description.clone(),
        has_footprint_ref: row.footprint_ref.is_some(),
        symbol,
        footprint,
        parameters: row
            .parameters
            .iter()
            .map(|(name, value)| (name.clone(), value.display()))
            .collect(),
        notes,
    })
}
