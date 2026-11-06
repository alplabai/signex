//! Off-thread read behind the details section: the row, its symbol and its
//! footprint. Pure IO, no app state, so it runs in `spawn_blocking`.

use std::sync::Arc;

use signex_library::LibraryAdapter;
use signex_library::RowId;
use signex_library::primitive::PrimitiveRef;
use uuid::Uuid;

use super::{ComponentDetails, FootprintOption};
use crate::library::editor::footprint::canvas::PreviewFootprint;
use crate::library::state::ComponentsPanelRowKey;

/// The mounted adapters, shared out of the library set by library id.
pub type MountedAdapters = Vec<(Uuid, Arc<dyn LibraryAdapter>)>;

/// The adapter of the open library a reference points into.
fn owner_of<'a>(
    libraries: &'a MountedAdapters,
    reference: &PrimitiveRef,
) -> Result<&'a dyn LibraryAdapter, String> {
    libraries
        .iter()
        .find(|(id, _)| *id == reference.library_id)
        .map(|(_, adapter)| adapter.as_ref())
        .ok_or_else(|| format!("its library {} is not open", reference.library_id))
}

/// Read what the details section shows for `key` through the mounted
/// adapters (no re-open per click). The row must read; a symbol or
/// footprint that cannot is reported in `notes` and left out.
pub fn load_details(
    key: &ComponentsPanelRowKey,
    own: &dyn LibraryAdapter,
    libraries: &MountedAdapters,
) -> Result<ComponentDetails, String> {
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
            .and_then(|a| a.get_symbol(reference.uuid).map_err(|e| e.to_string()))
    };
    let symbol = match read {
        Ok(symbol) => Some(symbol.to_lib_symbol(crate::library::place::lib_id_of(&reference))),
        Err(why) => {
            notes.push(format!("Symbol not available: {why}"));
            None
        }
    };
    let mut read_footprint = |reference: &PrimitiveRef| {
        let read = if reference.library_id == own.library_id() {
            own.get_footprint(reference.uuid).map_err(|e| e.to_string())
        } else {
            owner_of(libraries, reference)
                .and_then(|a| a.get_footprint(reference.uuid).map_err(|e| e.to_string()))
        };
        match read {
            Ok(footprint) => Some(PreviewFootprint::build(&footprint)),
            Err(why) => {
                notes.push(format!("Footprint not available: {why}"));
                None
            }
        }
    };
    let footprint = row.footprint_ref.as_ref().and_then(&mut read_footprint);
    // The alternates are read with the rest, so switching the picker is
    // instant and needs no second read (#1234).
    let alternate_footprints = row
        .alternate_footprints
        .iter()
        .map(|binding| {
            let preview = read_footprint(&binding.footprint);
            FootprintOption {
                reference: binding.footprint,
                name: preview.as_ref().map_or_else(
                    || format!("Footprint {}", short_id(&binding.footprint)),
                    |p| p.name.clone(),
                ),
                preview,
            }
        })
        .collect();

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
        default_footprint: row.footprint_ref,
        alternate_footprints,
        parameters: row
            .parameters
            .iter()
            .map(|(name, value)| (name.clone(), value.display()))
            .collect(),
        notes,
    })
}

/// The first eight characters of a primitive's id, for a footprint whose name
/// could not be read.
fn short_id(reference: &PrimitiveRef) -> String {
    reference
        .uuid
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect()
}
