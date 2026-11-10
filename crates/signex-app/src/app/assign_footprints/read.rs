//! The disk half of opening the dialog, off the UI thread: which sheets the
//! project has, and the sheets no engine holds. The table itself is built in
//! `update` from the open sheets' live documents plus this read.

use std::collections::HashMap;
use std::path::PathBuf;

use signex_types::schematic::{SchematicSheet, Symbol};

use crate::app::project_sheets::{self, OpenSheets};
use crate::app::state::scope::CaseFold;

/// Everything the read needs, captured from `update` (memory only).
pub(crate) struct ReadInputs {
    pub(crate) open: OpenSheets,
    pub(crate) pages: Vec<PathBuf>,
    pub(crate) root: PathBuf,
    pub(crate) active: Option<PathBuf>,
    pub(crate) fold: CaseFold,
}

/// What the read found.
#[derive(Debug, Clone, Default)]
pub struct FootprintRead {
    /// Every sheet of the project, the active one first.
    pub(crate) sheets: Vec<PathBuf>,
    /// Each sheet no engine held, as read from disk.
    pub(crate) closed: HashMap<PathBuf, SchematicSheet>,
    /// Sheets that exist but would not read, and why.
    pub(crate) unreadable: Vec<(PathBuf, String)>,
}

/// Assemble the project's sheets and keep the closed ones.
pub(crate) fn read_parts(inputs: &ReadInputs) -> FootprintRead {
    let set = project_sheets::assemble_project_sheets_from(
        &inputs.open,
        &inputs.pages,
        &inputs.root,
        inputs.fold,
    );
    let sheets = project_sheets::ordered_project_sheet_paths(&set, inputs.active.as_deref());
    let closed = sheets
        .iter()
        .filter(|path| !inputs.open.contains_key(*path))
        .filter_map(|path| Some((path.clone(), set.sheets.get(path)?.clone())))
        .collect();
    FootprintRead {
        sheets,
        closed,
        unreadable: set.unreadable,
    }
}

/// The parts of `doc` the table lists: no power ports, and one row per part
/// (a multi-unit part's later units take the first unit's footprint).
pub(crate) fn listed_parts(doc: &SchematicSheet) -> Vec<&Symbol> {
    let mut seen = std::collections::HashSet::new();
    let mut parts = Vec::new();
    for symbol in doc.symbols.iter().filter(|s| !s.is_power) {
        if seen.contains(&symbol.uuid) {
            continue;
        }
        seen.insert(symbol.uuid);
        seen.extend(signex_engine::placed_unit_ids(doc, symbol.uuid));
        parts.push(symbol);
    }
    parts
}
