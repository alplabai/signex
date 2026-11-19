//! State of the BOM export dialog (m112, #1988): the rolled-up table, the
//! options that drive the next rollup, and the view-only table state (sort,
//! filter, selection, column drag and resize, scroll).

use std::path::PathBuf;

/// Which tab the BOM dialog's Properties sidebar shows: General, Columns or
/// Rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BomSidebarTab {
    General,
    Columns,
    Rules,
}

/// Live BOM preview state — the rolled-up table for the active project
/// plus the user-editable options that drive the next rollup. Re-rolled
/// whenever an option toggle fires.
pub struct BomPreviewState {
    /// The document the preview was built for; Export writes this one even if
    /// the user has switched tabs since (#852).
    pub source: PathBuf,
    pub options: signex_output::BomOptions,
    pub table: signex_output::BomTable,
    /// Live validation of `table` under `options.rule_options`. Refreshed on
    /// every rollup and every rule toggle so it never goes stale against the
    /// displayed table. Advisory only — it never blocks export.
    pub validation_report: signex_output::BomValidationReport,
    /// Variants the project defines; the toolbar's Variant picker lists them
    /// after Base. Empty when the project defines none.
    pub variants: Vec<String>,
    /// Active sort spec — `(column index in options.columns, ascending)`.
    /// `None` = render rollup order (the default emit order from
    /// `bom::rollup`). Click a header cell to set; click the same one
    /// again to flip direction.
    pub sort: Option<(usize, bool)>,
    /// Row indexes into `table.rows` in display order under `sort`, before
    /// the filter. Recomputed in `update` whenever the rows, columns or sort
    /// change, so `view` only reads it.
    pub row_order: Vec<usize>,
    /// In-flight column drag — `Some(from_idx)` while the user is
    /// holding the mouse down on a header cell or a Columns-tab grip. The
    /// header only renders the drag highlight once the cursor has moved past
    /// `column_drag_press_x` by at least the threshold (see `view`); a quick
    /// press-and-release counts as a click and the cell never lights up.
    pub column_drag: Option<usize>,
    /// Cursor x at the moment the column drag was armed. Compared
    /// against `last_mouse_pos.0` in the view to decide whether
    /// the press has graduated into an actual drag.
    pub column_drag_press_x: Option<f32>,
    /// Index of the column header currently under the cursor.
    /// Tracked via on_enter/on_exit on each header cell so the
    /// release handler (which fires on the press-source widget,
    /// not the cursor target) can resolve where the drop landed.
    pub column_hover: Option<usize>,
    /// Per-column width overrides keyed by index in
    /// `options.columns`. Populated as the user drags a header
    /// resize handle; consulted by the width helper before falling
    /// back to the per-`BomColumn` default. Cleared on close.
    pub column_widths: std::collections::HashMap<usize, f32>,
    /// In-flight column-resize state — `Some` while the user is
    /// dragging a header's right-edge handle. `start_x` is the
    /// global cursor x at press; `start_width` is the column's
    /// width at press. Width updates each mouse-move tick are
    /// computed against these baselines.
    pub column_resize: Option<ColumnResizeState>,
    /// Currently-shown tab inside the Properties sidebar.
    pub sidebar_tab: BomSidebarTab,
    /// Text of the toolbar filter. View-only: it hides lines on screen and
    /// never changes what Export writes.
    pub filter: String,
    /// Positions in `row_order` (0-based lines) the filter keeps, in display
    /// order. Recomputed in `update` with `row_order`.
    pub visible_lines: Vec<usize>,
    /// Worst error or warning flagged on each row of `table`, by row index;
    /// `None` for a clean row. Recomputed in `update` with `row_order`.
    pub row_flags: Vec<Option<signex_output::BomIssueSeverity>>,
    /// Where each issue of `validation_report` sits in the table, by issue
    /// index; `None` when none of its designators is on a line.
    pub issue_lines: Vec<Option<BomIssueLine>>,
    /// Row of `table` the user selected, by index.
    pub selected_row: Option<usize>,
    /// Horizontal offset of the table body; the sticky header follows it.
    pub scroll_x: f32,
}

/// The table line an issue points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BomIssueLine {
    /// Row index in `BomPreviewState::table`.
    pub row: usize,
    /// 1-based line number, the one the row-number gutter shows.
    pub line: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct ColumnResizeState {
    pub idx: usize,
    pub start_x: f32,
    pub start_width: f32,
}
