//! The IO-free pieces of the open-time Library Updates scan: which placed
//! parts it looks at, why a shared copy is not auto-applied, and what each
//! part becomes (a modal entry, a silent Personal restamp, or nothing).

use crate::library::drift::{VERSION_NOT_STAMPED, rebound_row_warning, row_binds_lib_id};
use crate::library::part_sync::{Choices, ConflictView, PartEval};
use crate::library::updates_dialog::LibraryUpdateEntry;
use signex_library::RowId;
use signex_types::schematic::{LibSymbol, LinkStamp};

/// A library-backed symbol placed on the sheet being scanned.
#[derive(Clone)]
pub(super) struct PlacedLibrarySymbol {
    pub(super) uuid: uuid::Uuid,
    pub(super) reference: String,
    pub(super) library_id: uuid::Uuid,
    pub(super) row_id: RowId,
    pub(super) version: String,
    pub(super) lib_id: String,
}

/// A placed symbol's library row.
pub(super) struct InstanceRow {
    /// Its version differs from the placed symbol's.
    pub(super) stale: bool,
    pub(super) mode: signex_library::WorkflowMode,
    pub(super) library_name: String,
    pub(super) library_path: std::path::PathBuf,
    pub(super) row: signex_library::ComponentRow,
}

/// Per `lib_id`: the library's symbol and the `(placed symbol, version)`
/// stamps to apply with it.
pub(super) type PersonalUpdates =
    std::collections::BTreeMap<String, (LibSymbol, Vec<(uuid::Uuid, LinkStamp)>)>;

/// A placed instance with its row (`None`: unreadable).
pub(super) type Instance = (PlacedLibrarySymbol, Option<InstanceRow>);

/// The instances of `lib_id` whose row still binds it. A stale row re-bound
/// to another symbol is logged and counted in `any_failed`.
///
/// A part whose version has not moved is in the list too: its value and its
/// row's can still disagree (a row edited outside the version bump, a
/// footprint renamed under it), and a conflict there has to reach the modal
/// to be answered.
pub(super) fn bound_instances<'a>(
    group: &'a [Instance],
    lib_id: &str,
    any_failed: &mut bool,
) -> Vec<(&'a PlacedLibrarySymbol, &'a InstanceRow)> {
    let mut bound = Vec::new();
    for (symbol, row) in group {
        let Some(row) = row.as_ref() else {
            continue;
        };
        if row_binds_lib_id(&row.row, lib_id) {
            bound.push((symbol, row));
        } else if row.stale {
            crate::diagnostics::log_warning(rebound_row_warning(
                &symbol.reference,
                symbol.row_id,
                lib_id,
                VERSION_NOT_STAMPED,
            ));
            *any_failed = true;
        }
    }
    bound
}

/// Why `symbol` stops its `lib_id` from being auto-applied: the shared copy
/// moves only when every instance has a readable Personal-mode row that
/// still binds it. The Messages line names the instance.
pub(super) fn blocker(
    symbol: &PlacedLibrarySymbol,
    row: Option<&InstanceRow>,
    lib_id: &str,
) -> Option<String> {
    let reason = match row {
        None => "its library row is unreadable",
        Some(r) if !row_binds_lib_id(&r.row, lib_id) => "its row binds a different symbol",
        Some(r) if r.mode != signex_library::WorkflowMode::Personal => "its library is Team-mode",
        Some(_) => return None,
    };
    Some(format!(
        "{}: {reason}, so the shared copy was not auto-updated ({lib_id}).",
        symbol.reference
    ))
}

/// The silent Personal restamp of a stale part, `None` when it is left alone.
///
/// A part that does not hold its row's footprint / Manufacturer / MPN yet
/// keeps its old stamp: restamping would read as "row unchanged" and hide
/// that drift. A part whose value and row's both moved is the user's to
/// settle, in the modal (#2004). `checked` is whether the part could be
/// checked at all (its sheet is open); an unchecked part is stamped by
/// version alone, a checked part the check could not read is left alone.
pub(super) fn personal_link(
    version: String,
    checked: bool,
    eval: Option<&PartEval>,
) -> Option<LinkStamp> {
    match (checked, eval) {
        (false, _) => Some(LinkStamp::version(version)),
        (true, None) => None,
        (true, Some(eval)) => (!eval.is_flagged()).then(|| LinkStamp {
            version,
            values: eval.settle(false, &Choices::new()).values,
        }),
    }
}

/// The modal row for `symbol`, with the conflicts the user may answer.
pub(super) fn modal_entry(
    symbol: PlacedLibrarySymbol,
    row: &InstanceRow,
    conflicts: Vec<ConflictView>,
) -> LibraryUpdateEntry {
    let bump_kind =
        crate::library::updates_dialog::classify_bump(&symbol.version, &row.row.version);
    LibraryUpdateEntry {
        symbol_uuid: symbol.uuid,
        ref_des: symbol.reference,
        lib_id: symbol.lib_id,
        library_id: symbol.library_id,
        library_name: row.library_name.clone(),
        row_id: symbol.row_id,
        library_path: row.library_path.clone(),
        current_version: symbol.version,
        latest_version: row.row.version.clone(),
        bump_kind,
        selected: false,
        conflicts: conflicts
            .into_iter()
            .map(crate::library::updates_dialog::ConflictChoice::new)
            .collect(),
    }
}
