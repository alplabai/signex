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

/// The parts of `sheets` the table lists, one per physical part: no power
/// ports, and the units of a multi-unit part grouped across every sheet
/// (`group_into_parts`, the rule the engine's `placed_unit_ids` uses within
/// one sheet), so a part spread over two sheets is one row with one
/// footprint. Each unit is `(index into sheets, symbol)`, in sheet order. An
/// unannotated part (`U?`) is never grouped, as the engine does.
pub(crate) fn listed_parts<'a>(sheets: &[&'a SchematicSheet]) -> Vec<Vec<(usize, &'a Symbol)>> {
    let mut annotated = Vec::new();
    let mut parts = Vec::new();
    for (index, doc) in sheets.iter().enumerate() {
        for symbol in doc.symbols.iter().filter(|s| !s.is_power) {
            let reference = symbol.reference.trim();
            if reference.is_empty() || reference.ends_with('?') {
                parts.push(vec![(index, symbol)]);
            } else {
                annotated.push((index, symbol));
            }
        }
    }
    let groups = signex_types::schematic::group_into_parts(
        annotated.iter().map(|(_, symbol)| symbol.part_key()),
    );
    parts.extend(
        groups
            .into_iter()
            .map(|group| group.into_iter().map(|at| annotated[at]).collect()),
    );
    parts
}
