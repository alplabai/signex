//! The BOM dialog's table on screen, recomputed in `update`: display order,
//! the lines the filter keeps, the rows the validation rules flag, and the
//! line each issue points at. Pure functions of the preview state; `view`
//! only reads what they produce.

use std::collections::HashMap;

use signex_output::{BomColumn, BomIssueSeverity, BomRow, BomValidationReport};

use crate::app::state::{BomIssueLine, BomPreviewState};

impl BomPreviewState {
    /// Id of the table body scrollable.
    pub(crate) const BODY_SCROLL: iced::widget::Id = iced::widget::Id::new("bom-table-body");
    /// Id of the sticky header's scrollable, moved with the body.
    pub(crate) const HEADER_SCROLL: iced::widget::Id = iced::widget::Id::new("bom-table-header");

    /// Recompute everything the table shows from the rows, columns, sort,
    /// filter and report. Run after every message that can change one.
    pub(crate) fn refresh_rows(&mut self) {
        self.row_order = crate::app::view::dialogs::sorted_row_order(
            &self.table.rows,
            &self.options.columns,
            self.sort,
        );
        self.visible_lines = kept_lines(
            &self.table.rows,
            &self.row_order,
            &self.options.columns,
            &self.filter,
        );
        self.row_flags = row_flags(&self.table.rows, &self.validation_report);
        self.issue_lines = issue_lines(&self.table.rows, &self.row_order, &self.validation_report);
        if self
            .selected_row
            .is_some_and(|row| row >= self.table.rows.len())
        {
            self.selected_row = None;
        }
    }

    /// Whether the one not-fitted option reads as ticked: only when both
    /// domain flags are on, so a part never enters the export behind a box
    /// that looks clear (docs/DECISIONS.md, 2026-11-19).
    pub(crate) fn includes_not_fitted(&self) -> bool {
        self.options.include_dnp && self.options.include_not_fitted
    }
}

/// Lines of `row_order` (0-based positions) whose shown cells contain
/// `filter`, ignoring case and surrounding blanks. An empty filter keeps all.
pub(crate) fn kept_lines(
    rows: &[BomRow],
    row_order: &[usize],
    columns: &[BomColumn],
    filter: &str,
) -> Vec<usize> {
    let needle = filter.trim().to_lowercase();
    row_order
        .iter()
        .enumerate()
        .filter(|(_, row)| {
            needle.is_empty()
                || rows
                    .get(**row)
                    .is_some_and(|row| row_contains(row, columns, &needle))
        })
        .map(|(line, _)| line)
        .collect()
}

/// Whether any shown cell of `row` contains `needle` (already lower-case).
fn row_contains(row: &BomRow, columns: &[BomColumn], needle: &str) -> bool {
    columns.iter().any(|column| {
        signex_output::bom::cell_text(row, column)
            .to_lowercase()
            .contains(needle)
    })
}

/// Row index of every designator in the table.
fn rows_by_designator(rows: &[BomRow]) -> HashMap<&str, usize> {
    let mut by_ref = HashMap::new();
    for (index, row) in rows.iter().enumerate() {
        for reference in &row.references {
            by_ref.entry(reference.as_str()).or_insert(index);
        }
    }
    by_ref
}

/// The worst error or warning on each row; notes do not flag a row.
pub(crate) fn row_flags(
    rows: &[BomRow],
    report: &BomValidationReport,
) -> Vec<Option<BomIssueSeverity>> {
    let by_ref = rows_by_designator(rows);
    let mut flags = vec![None; rows.len()];
    for issue in &report.issues {
        if issue.severity == BomIssueSeverity::Info {
            continue;
        }
        for reference in &issue.references {
            let Some(&row) = by_ref.get(reference.as_str()) else {
                continue;
            };
            let flag: &mut Option<BomIssueSeverity> = &mut flags[row];
            if *flag != Some(BomIssueSeverity::Error) {
                *flag = Some(issue.severity);
            }
        }
    }
    flags
}

/// The line of each issue: the first of its designators that is on a line.
pub(crate) fn issue_lines(
    rows: &[BomRow],
    row_order: &[usize],
    report: &BomValidationReport,
) -> Vec<Option<BomIssueLine>> {
    let by_ref = rows_by_designator(rows);
    let mut line_of_row = vec![0; rows.len()];
    for (line, &row) in row_order.iter().enumerate() {
        if let Some(slot) = line_of_row.get_mut(row) {
            *slot = line + 1;
        }
    }
    report
        .issues
        .iter()
        .map(|issue| {
            issue
                .references
                .iter()
                .find_map(|reference| by_ref.get(reference.as_str()))
                .map(|&row| BomIssueLine {
                    row,
                    line: line_of_row[row],
                })
        })
        .collect()
}
