//! Checks the Edit Component modal runs on the draft before Save (lib-07:
//! an inline field error plus a footer summary, Save held until fixed).

use signex_library::{InternalPn, RowId};

use super::super::state::OpenLibrary;

/// The table of another cached row that already holds `pn`, by the rule
/// the backends refuse a write with ([`InternalPn::clashes`]). The edited
/// row `row` never collides with itself, and a PN left as the row has it
/// stored is not a conflict, as the backends allow it (a library that
/// already holds duplicates still lets their other fields be edited).
/// Only the cached tables are read; the backend's own refusal stays the
/// backstop for a table not read yet and for a race.
pub(crate) fn pn_conflict(lib: &OpenLibrary, pn: &str, row: RowId) -> Option<String> {
    let is_row = |r: &signex_library::ComponentRow| RowId::from_uuid(r.row_id) == row;
    let unchanged = lib
        .tables
        .values()
        .flatten()
        .find(|r| is_row(r))
        .is_some_and(|stored| InternalPn::clashes(pn, stored.internal_pn.as_str()));
    if unchanged {
        return None;
    }
    // The first table by name, so the message does not change between
    // frames with the map's order.
    lib.tables
        .iter()
        .filter(|(_, rows)| {
            rows.iter()
                .any(|r| !is_row(r) && InternalPn::clashes(pn, r.internal_pn.as_str()))
        })
        .map(|(table, _)| table)
        .min()
        .cloned()
}
