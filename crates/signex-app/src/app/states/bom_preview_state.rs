/// Live BOM preview state — the rolled-up table for the active project
/// plus the user-editable options that drive the next rollup. Re-rolled
/// whenever an option toggle fires.
pub struct BomPreviewState {
    pub options: signex_output::BomOptions,
    pub table: signex_output::BomTable,
    /// Available variants for the active project. Reserved for the
    /// variant picker dropdown — empty when no variants are defined.
    /// Currently only seeded; the picker UI lands in v0.8.1.
    #[allow(dead_code)]
    pub variants: Vec<String>,
    /// Active sort spec — `(column index in options.columns, ascending)`.
    /// `None` = render rollup order (the default emit order from
    /// `bom::rollup`). Click a header cell to set; click the same one
    /// again to flip direction.
    pub sort: Option<(usize, bool)>,
    /// In-flight column drag — `Some(from_idx)` while the user is
    /// holding the mouse down on a header cell. The header only
    /// renders the drag highlight once the cursor has moved past
    /// `column_drag_press_x` by at least the threshold (see
    /// `view`); a quick press-and-release counts as a click and
    /// the cell never lights up.
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
    pub column_resize: Option<super::ColumnResizeState>,
    /// Currently-shown tab inside the Properties sidebar.
    pub sidebar_tab: super::BomSidebarTab,
}
