//! Messages of the Components panel table (mockup row 52, components-cx):
//! category, columns, sort, grouping, range filters and the Details
//! parameter pins. Routed by `dispatch/library/components_table.rs`.

use signex_library::components_view::GroupPath;
use signex_library::templates::ColumnKey;

use super::RangeFilterMsg;
use crate::panels::components_panel::table::CategoryKey;

/// A header menu entry (right-click on a column header).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderAction {
    SortAscending,
    SortDescending,
    /// Add to sort / Remove from sort.
    ToggleSort,
    /// Group by this / Remove from groups.
    ToggleGroup,
    Hide,
    ResetWidth,
    /// Columns…: the Columns picker opens.
    OpenColumns,
}

/// Something the user did to the Components panel table.
#[derive(Debug, Clone, PartialEq)]
pub enum ComponentsTableMsg {
    /// The category picker picked a category.
    CategoryPicked(CategoryKey),
    /// A header was clicked: sort by it (nested while Shift is held).
    HeaderPressed(ColumnKey),
    /// A header was right-clicked: its menu opens.
    HeaderRightClicked(ColumnKey),
    /// An entry of the open header menu was picked.
    HeaderMenuPicked(HeaderAction),
    /// The pointer went down on a column's resize handle.
    ResizeStarted(ColumnKey),
    /// The pointer moved over the header strip, `x` from its left edge.
    ResizeMoved(f32),
    /// The pointer was released while resizing: the width is saved.
    ResizeEnded,
    /// A group line was clicked: it opens or closes.
    GroupToggled(GroupPath),
    ExpandAllPressed,
    CollapseAllPressed,
    /// The group bar's "+ Group" was pressed: its column menu opens.
    GroupMenuToggled,
    /// A column was picked in the "+ Group" menu: a new innermost level.
    GroupAdded(ColumnKey),
    /// A group chip's arrow was pressed: the level's direction flips.
    GroupFlipped(usize),
    /// A group chip's ✕ was pressed.
    GroupRemoved(usize),
    /// The Columns button was pressed.
    ColumnsToggled,
    /// A Columns picker check box was ticked or cleared.
    ColumnShown {
        column: ColumnKey,
        shown: bool,
    },
    /// A Columns picker ↑ / ↓ was pressed.
    ColumnMoved {
        column: ColumnKey,
        later: bool,
    },
    /// The Columns picker's search field was edited.
    ColumnSearchChanged(String),
    /// Reset to category default, in the Columns picker.
    ResetPressed,
    /// The Filter button was pressed.
    FilterToggled,
    /// In stock only, in the Filter popover.
    InStockToggled(bool),
    /// An edit of one of the category's range filters.
    RangeFilter(RangeFilterMsg),
    /// A bound's prefix picker of the range filter on `key` was set: a
    /// prefix, or `None` for Any prefix (components-cx decision 6).
    BoundPrefixPicked {
        key: String,
        high: bool,
        prefix: Option<signex_types::quantity::SiPrefix>,
    },
    /// A min-to-max filter's target was picked: Covers, Min or Max.
    RangeTargetPicked {
        key: String,
        on: signex_library::RangeOn,
    },
    /// Clear filters, in the Filter popover or the empty table.
    FiltersCleared,
    /// A press outside the open popover, or its ✕, or Esc.
    PopoverDismissed,
    /// The view switch: list only (`true`) or list with details.
    ListOnlyPicked(bool),
    /// The table body scrolled.
    Scrolled {
        x: f32,
        y: f32,
        viewport_h: f32,
    },
    /// A Details parameter's star was pressed: pin or unpin it for the
    /// category.
    PinToggled(String),
    /// Show all / Show less under the pinned Details parameters.
    ShowAllParametersToggled,
}
