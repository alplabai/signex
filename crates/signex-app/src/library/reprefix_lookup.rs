//! Off-thread half of Re-prefix to standard (#1315): read, from each
//! mounted library the project's parts were placed from, the facts a part's
//! prefix is decided by - the row's component class and its prefix override.
//!
//! Pure IO with no app state, so it runs in `spawn_blocking`; the `update`
//! side is `app/handlers/erc/reprefix.rs`, and the decision itself is the
//! pure planner in `app/handlers/erc/reprefix_plan.rs`.

use std::collections::HashMap;

use uuid::Uuid;

use super::drift::MountedLibrary;

/// What decides one library row's reference prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowFacts {
    /// The row's internal part number, to name it in Messages lines.
    pub internal_pn: String,
    /// The symbol the row binds, `(library id, symbol id)`: the pair a
    /// placed part's `lib_id` (`"<library>:<symbol>"`) names.
    pub symbol: (Uuid, Uuid),
    /// The row's component class key (`"diode"`, `"generic"`).
    pub class: String,
    /// The row's raw `designator_prefix` parameter, not yet validated.
    pub prefix_override: Option<String>,
}

/// The libraries to read, and the token that makes a late answer droppable.
#[derive(Debug)]
pub struct ReprefixLookupJob {
    pub generation: u64,
    /// The project the request was made for (its `.snxprj` path); a result
    /// that lands while another project is active is dropped.
    pub project: Option<std::path::PathBuf>,
    /// Every mounted library: which of them the project's parts link to is
    /// decided on the `update` side, from state, not by reading sheets here.
    pub libs: Vec<MountedLibrary>,
}

/// What the worker found. A library is a key of `libraries` only when it was
/// read completely; a library that could not be read is named in `problems`
/// and its parts are reported as skipped, never guessed at.
#[derive(Debug, Clone, Default)]
pub struct ReprefixLookup {
    pub generation: u64,
    pub project: Option<std::path::PathBuf>,
    /// `library id -> row id -> facts`.
    pub libraries: HashMap<Uuid, HashMap<Uuid, RowFacts>>,
    pub problems: Vec<String>,
}

impl ReprefixLookupJob {
    pub fn run(self) -> ReprefixLookup {
        let mut lookup = ReprefixLookup {
            generation: self.generation,
            project: self.project,
            ..Default::default()
        };
        for lib in self.libs {
            match read_library(&lib) {
                Ok(rows) => {
                    lookup.libraries.insert(lib.id, rows);
                }
                Err(problem) => lookup.problems.push(problem),
            }
        }
        lookup
    }
}

fn read_library(lib: &MountedLibrary) -> Result<HashMap<Uuid, RowFacts>, String> {
    use signex_library::LibraryAdapter as _;
    use signex_library::adapters::local_git::LocalGitAdapter;

    let adapter = LocalGitAdapter::open(&lib.path).map_err(|e| {
        format!(
            "Re-prefix: could not open library '{}' ({}): {e}",
            lib.display_name,
            lib.path.display()
        )
    })?;
    let rows = adapter.iter_rows().map_err(|e| {
        format!(
            "Re-prefix: could not read library '{}': {e}",
            lib.display_name
        )
    })?;
    Ok(rows
        .into_iter()
        .map(|(_table, row)| {
            let facts = RowFacts {
                internal_pn: row.internal_pn.to_string(),
                symbol: (row.symbol_ref.library_id, row.symbol_ref.uuid),
                class: row.class.as_str().to_owned(),
                prefix_override: row.designator_prefix_override().map(str::to_owned),
            };
            (row.row_id, facts)
        })
        .collect())
}
