//! The optional trailing `alternate_symbols` column of a component table.

use crate::adapter::LibraryError;
use crate::alternate_symbol::AlternateSymbol;
use crate::component::ComponentRow;

use super::{TABLE_HEADER, from_json_cell, json_cell};

/// Optional trailing column holding a row's `alternate_symbols` as JSON.
pub const ALTERNATE_SYMBOLS_COLUMN: &str = "alternate_symbols";

/// Check a table header against the legacy columns, with or without the
/// optional trailing one. `Err` carries the mismatch detail.
pub(crate) fn check_columns(columns: &[&str]) -> Result<(), String> {
    let width = columns.len();
    let with_alternates = width == TABLE_HEADER.len() + 1;
    if width != TABLE_HEADER.len() && !with_alternates {
        return Err(format!(
            "{width} columns, expected {} or {}",
            TABLE_HEADER.len(),
            TABLE_HEADER.len() + 1
        ));
    }
    let extra = with_alternates.then_some(ALTERNATE_SYMBOLS_COLUMN);
    for (&got, want) in columns
        .iter()
        .zip(TABLE_HEADER.iter().copied().chain(extra))
    {
        if got != want {
            return Err(format!("column {got:?}, expected {want:?}"));
        }
    }
    Ok(())
}

/// JSON cell for `row.alternate_symbols`; `None` when there are none.
pub(crate) fn alternate_symbols_cell(row: &ComponentRow) -> Result<Option<String>, LibraryError> {
    if row.alternate_symbols.is_empty() {
        return Ok(None);
    }
    json_cell(&row.alternate_symbols).map(Some)
}

/// Parse an `alternate_symbols` cell; empty = none.
pub(super) fn alternate_symbols_from_cell(s: &str) -> Result<Vec<AlternateSymbol>, LibraryError> {
    if s.is_empty() {
        return Ok(Vec::new());
    }
    from_json_cell(s, ALTERNATE_SYMBOLS_COLUMN)
}

/// Repair for a component table's row that is exactly one cell short of the
/// 23-column header (a merge artefact): reads as no alternates. Only when
/// the row is shaped like a full 22-cell row, so a dropped middle cell,
/// which shifts every later cell left, stays an error whenever the shift
/// shows. A dropped middle cell with no alternates leaves the last cell
/// empty and moves the description into the sim_version slot; a sim
/// version with no sim_ref is refused too. Not repaired: any other table, any
/// other count.
pub(crate) fn repair_short_component_row(
    columns: &[String],
    cells: &[&str],
) -> Option<Vec<String>> {
    let names: Vec<&str> = columns.iter().map(String::as_str).collect();
    let legacy = TABLE_HEADER.len();
    if names.len() != legacy + 1 || cells.len() != legacy || check_columns(&names).is_err() {
        return None;
    }
    let at = |name: &str| TABLE_HEADER.iter().position(|c| *c == name);
    let released = cells[at("released")?];
    let row_version = cells[at("version")?];
    let versions_ok = ["symbol_version", "footprint_version", "sim_version"]
        .iter()
        .all(|n| at(n).is_some_and(|i| is_version_like(cells[i])));
    // Both writers always emit `true`/`false` for `released`, so an empty
    // cell is a shift, not a legacy row. `version` may be written empty
    // (the reader maps it to 0.0.1) but is never `true`/`false`.
    let version_ok = is_version_like(row_version) && !matches!(row_version, "true" | "false");
    // A shifted alternates cell lands in the description slot as a
    // non-empty JSON array (the writer never emits `[]` for it), so `[]`
    // there is a description.
    let description_ok = at("description").is_some_and(|i| {
        !serde_json::from_str::<serde_json::Value>(cells[i])
            .is_ok_and(|v| v.as_array().is_some_and(|a| !a.is_empty()))
    });
    if !matches!(released, "true" | "false") || !version_ok || !versions_ok || !description_ok {
        return None;
    }
    // An empty last cell is also what a dropped middle cell leaves (the
    // empty alternates cell moved up), and the description then sits in the
    // sim_version slot. A sim version with no sim to pin is that reading:
    // refuse. (footprint_version is not checked: rows are written with one
    // and no footprint.)
    if cells[cells.len() - 1].is_empty() {
        let pinned_sim_without_ref = matches!(
            (at("sim_ref"), at("sim_version")),
            (Some(r), Some(v)) if cells[r].is_empty() && !cells[v].is_empty()
        );
        if pinned_sim_without_ref {
            return None;
        }
    }
    let mut fixed: Vec<String> = cells.iter().map(|c| (*c).to_string()).collect();
    fixed.push(String::new());
    Some(fixed)
}

/// A pinned-version cell: empty, or made of version characters. A shifted
/// alternates cell is JSON and never is.
fn is_version_like(cell: &str) -> bool {
    !cell.starts_with('[')
        && cell
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+' | '_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_cells() -> Vec<&'static str> {
        let mut cells = vec![""; TABLE_HEADER.len()];
        cells[16] = "0.0.1";
        cells[17] = "false";
        cells[18] = "0.0.1";
        cells
    }

    fn full_header() -> Vec<String> {
        TABLE_HEADER
            .iter()
            .copied()
            .chain([ALTERNATE_SYMBOLS_COLUMN])
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn a_legacy_shaped_short_row_is_padded() {
        let fixed = repair_short_component_row(&full_header(), &legacy_cells()).unwrap();
        assert_eq!(fixed.len(), TABLE_HEADER.len() + 1);
        assert_eq!(fixed.last().unwrap(), "");
    }

    #[test]
    fn a_shifted_alternates_cell_in_the_last_slot_is_refused() {
        let mut cells = legacy_cells();
        cells[21] = r#"[{"label":"IEC"}]"#;
        assert!(repair_short_component_row(&full_header(), &cells).is_none());
    }

    #[test]
    fn a_description_of_empty_brackets_is_a_description() {
        let mut cells = legacy_cells();
        cells[21] = "[]";
        let fixed = repair_short_component_row(&full_header(), &cells).unwrap();
        assert_eq!(fixed[21], "[]");
    }

    #[test]
    fn a_row_with_a_description_is_padded_and_keeps_it() {
        let mut cells = legacy_cells();
        cells[21] = "100 nF 50 V";
        let fixed = repair_short_component_row(&full_header(), &cells).unwrap();
        assert_eq!(fixed[21], "100 nF 50 V");
        assert_eq!(fixed[22], "");
    }

    /// footprint_version dropped from a row with a version-looking
    /// description and no alternates: the description shifts into the
    /// sim_version slot and must not be repaired into one.
    #[test]
    fn a_dropped_middle_cell_with_a_version_like_description_is_refused() {
        let mut cells = legacy_cells();
        cells[19] = "";
        cells[20] = "BC547";
        cells[21] = "";
        assert!(repair_short_component_row(&full_header(), &cells).is_none());
    }

    #[test]
    fn a_legacy_row_written_with_an_empty_version_still_reads() {
        let mut cells = legacy_cells();
        cells[16] = "";
        assert!(repair_short_component_row(&full_header(), &cells).is_some());
    }

    /// `released` dropped from a row whose later cells are empty: the
    /// shifted `released` reads as an empty cell, which must not pass.
    #[test]
    fn a_dropped_released_cell_is_refused_even_with_empty_pins() {
        let mut cells = legacy_cells();
        cells.remove(17);
        cells.push("");
        cells[17] = "";
        assert!(repair_short_component_row(&full_header(), &cells).is_none());
    }

    /// `version` dropped: the `released` value lands in the version slot.
    #[test]
    fn a_dropped_version_cell_is_refused() {
        for released in ["true", "false"] {
            let mut cells = legacy_cells();
            cells.remove(16);
            cells.push("");
            cells[16] = released;
            assert!(repair_short_component_row(&full_header(), &cells).is_none());
        }
    }

    #[test]
    fn a_shifted_released_cell_is_refused() {
        let mut cells = legacy_cells();
        cells[17] = "0.0.1";
        assert!(repair_short_component_row(&full_header(), &cells).is_none());
    }

    #[test]
    fn a_user_table_ending_in_alternate_symbols_is_never_padded() {
        let columns: Vec<String> = ["row_id", "mpn", ALTERNATE_SYMBOLS_COLUMN]
            .map(str::to_string)
            .into();
        assert!(repair_short_component_row(&columns, &["1", "a"]).is_none());
    }

    #[test]
    fn any_other_cell_count_is_refused() {
        let mut two_short = legacy_cells();
        two_short.pop();
        assert!(repair_short_component_row(&full_header(), &two_short).is_none());
    }
}
