//! Footprint resolution against the open libraries for the design read. The
//! rule (row and stamped binding first, else the footprint the part names)
//! is `signex_library::part_footprint::resolve_symbol_footprint`.

use std::collections::HashMap;
use std::path::PathBuf;

use signex_library::ComponentRow;
use signex_library::LibraryAdapter as _;
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::part_footprint::resolve_symbol_footprint;
use signex_types::part::{FootprintResolveError, ResolvedFootprint};
use signex_types::schematic::Symbol;
use uuid::Uuid;

/// One open library: its id, `.snxlib` path and rows.
#[derive(Debug, Clone)]
pub(crate) struct LibraryRows {
    pub id: Uuid,
    pub root: PathBuf,
    pub rows: Vec<ComponentRow>,
}

/// Footprint resolution against the open libraries, with one adapter per
/// library opened once.
pub(crate) struct Resolver<'a> {
    libraries: &'a [LibraryRows],
    adapters: HashMap<Uuid, LocalGitAdapter>,
    /// The libraries that did not open, with why.
    pub(crate) errors: Vec<(PathBuf, String)>,
}

impl<'a> Resolver<'a> {
    pub(crate) fn open(libraries: &'a [LibraryRows]) -> Self {
        let mut adapters = HashMap::new();
        let mut errors = Vec::new();
        for lib in libraries {
            match LocalGitAdapter::open_to_read(&lib.root) {
                Ok(adapter) => {
                    adapters.insert(lib.id, adapter);
                }
                Err(e) => errors.push((lib.root.clone(), e.to_string())),
            }
        }
        Self {
            libraries,
            adapters,
            errors,
        }
    }

    /// The board footprint of the part whose first unit is `symbol`.
    pub(crate) fn resolve(
        &self,
        symbol: &Symbol,
    ) -> Result<ResolvedFootprint, FootprintResolveError> {
        resolve_symbol_footprint(
            symbol,
            |library, row_id| {
                self.libraries
                    .iter()
                    .filter(|lib| lib.id == library)
                    .flat_map(|lib| &lib.rows)
                    .find(|row| row.row_id == row_id)
            },
            |address, _version| {
                self.adapters
                    .get(&address.library_id)?
                    .get_footprint(address.uuid)
                    .ok()
            },
        )
    }
}
