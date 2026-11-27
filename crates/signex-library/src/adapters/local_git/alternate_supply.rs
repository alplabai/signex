//! The local library's `alternate_supply` cache (#3046): store fetched
//! records and drop the ones no row describes any more. The same rules as
//! the cached prices in `helpers.rs`, keyed by alternate MPN.

use super::helpers::*;
use super::*;
use crate::alternate_supply::{
    AlternateSupplyUpdate, prune_row_alternate_supply, record_is_current,
};
use crate::pricing::same_part_number;

/// Attach one fetched record to its row, or say why the row no longer takes
/// it: the row must still exist, list the distributor and list the
/// alternate. The row itself, and so the alternate's hand-entered values,
/// is not touched.
pub(super) fn apply_alternate_supply_update(
    lf: &mut LibraryFile,
    update: &AlternateSupplyUpdate,
) -> Result<(), String> {
    let entry = lf
        .tables
        .get_mut(&update.table)
        .ok_or_else(|| format!("table {} no longer exists", update.table))?;
    let row_id_text = update.row_id.to_string();
    let slot = entry
        .rows
        .iter()
        .find(|r| r.cells.get(LEGACY_ROW_ID_COL).map(String::as_str) == Some(row_id_text.as_str()))
        .ok_or_else(|| "row was deleted".to_string())?;
    let row = library_row_to_component(slot).map_err(|e| format!("row unreadable: {e}"))?;
    if !record_is_current(&row, &update.distributor, &update.mpn) {
        return Err(format!(
            "alternate {} is no longer listed with {}",
            update.mpn, update.distributor
        ));
    }
    let per_mpn = entry
        .alternate_supply
        .entry(row_id_text)
        .or_default()
        .entry(update.distributor.clone())
        .or_default();
    // A retyped MPN names the same part: replace the old record.
    per_mpn.retain(|k, _| !same_part_number(k, &update.mpn));
    per_mpn.insert(update.mpn.clone(), update.fetched.clone());
    Ok(())
}

/// Drop every fetched record whose row is gone, or whose distributor or
/// alternate the row no longer lists. A row that cannot be read keeps its
/// records. Returns whether anything changed.
pub(super) fn prune_stale_alternate_supply(lf: &mut LibraryFile) -> bool {
    let mut changed = false;
    for table in lf.tables.values_mut() {
        if table.alternate_supply.is_empty() {
            continue;
        }
        let ids: Vec<String> = table.alternate_supply.keys().cloned().collect();
        for id in ids {
            changed |= prune_alternate_supply_of(table, &id);
        }
    }
    changed
}

/// [`prune_stale_alternate_supply`] for one row id of one table: no row with
/// the id drops its records; otherwise the last row with the id that reads
/// as a component decides which still apply. Returns whether anything
/// changed.
pub(super) fn prune_alternate_supply_of(table: &mut LibraryTable, id: &str) -> bool {
    let mut live = false;
    let mut row = None;
    for r in &table.rows {
        if r.cells.get(LEGACY_ROW_ID_COL).map(String::as_str) == Some(id) {
            live = true;
            if let Ok(component) = library_row_to_component(r) {
                row = Some(component);
            }
        }
    }
    let Some(cached) = table.alternate_supply.get_mut(id) else {
        return false;
    };
    let mut changed = row
        .as_ref()
        .is_some_and(|row| prune_row_alternate_supply(row, cached));
    if !live || cached.is_empty() {
        table.alternate_supply.remove(id);
        changed = true;
    }
    changed
}
