# Components panel

The Components panel (`crates/signex-app/src/panels/components_panel/`) shows
every mounted part in one table, one category at a time (mockup row 52,
#2785). This page is the map of the code; the user's view is in
`docs/USER_GUIDE.md` section 5.

## Layout

From the top:

1. **Tool line** (`table/view.rs`, `toolbar`): the category picker (All
   parts, then each component class the mounted libraries hold, with its part
   count), **Filter** (with the number of active filters), **Columns**, the
   view switch **List** / **Details** and the **Previews** switch.
2. **Search field** (`look::filter_field`), value-aware (`query.rs`), with the
   match summary line under it.
3. The open **popover**, drawn as a card under the tool line
   (`table/popovers.rs`): the Columns picker, the Filter popover, a header's
   menu or the "+ Group" menu. Only one is open.
4. **Group bar** (`table/group_bar.rs`): one chip per group level, outermost
   first, with its direction arrow and ✕; "+ Group"; Expand all and
   Collapse all.
5. **Table** (`table/header.rs`, `table/body.rs`): the header strip and the
   body. The header scrolls sideways with the body (`HEADER_SCROLL` follows
   `BODY_SCROLL` through a `scroll_to` task).
6. **Status line**: "Results N of M" and the sort keys.
7. With **Details** on, the splitter and the selected part's details
   (`details/`): header card, Symbol and Footprint previews, the footprint
   pick, the parameter table with a pin star per row, Show all, References
   and Part Choices.

With nothing mounted the panel is the "No libraries mounted" card; its
**Add Library…** opens the Project / Installed / Global menu.

## Model, cache and view

The domain is in `signex-library`: `components_view` (cell values, nested
sort, nested grouping, the saved setup and its edits, the Details parameter
order) and `range_filter` (bounds, Any prefix, Covers / Min / Max). The panel
keeps state and draws; it does not sort or compare values itself.

- `table/state.rs`: `ComponentsTable`, in `ComponentsPanelState::table`. The
  category shown, the user's saved setups by category
  (`CategoryKey::pref_key`: `*` for All parts, else the class), open groups by
  `GroupPath`, session range filters and In stock only per category, the open
  popover, a column resize in progress, the body scroll and the view switch.
- `table/update.rs`: `ComponentsTable::apply(ComponentsTableMsg)`, pure. It
  returns `TableEffects`: save the setups, move the header strip, scroll the
  body to its top. `dispatch/library/components_table.rs` turns them into
  tasks.
- `table/cache.rs`: `TableCache`, rebuilt by `LibraryState::sync_view_caches_in`
  after every message, in three stages, each only when its inputs change:
  1. **base** (data version, category, language): the category's rows with
     their `RowFacts` (symbol and footprint names, cached stock and price), the
     class template, the offered columns, coverage counts and the measurement
     columns a range filter can use;
  2. **arrange** (search text, sort, group, active filters, In stock): the
     rows the search and filters keep, then `arrange_rows`;
  3. **lines** (the open set's generation): the group lines and rows to draw.
  The selected part's Details lists are rebuilt when the part, the data or its
  category's pins change.
- The views only read the cache. The body draws a window of lines
  (`library::browser::visible_range` with the reported scroll offset), so a
  frame builds a few dozen rows whatever the part count. See
  `docs/architecture/library-scale-bench.md` for the budget; the
  `table/cache_tests.rs` 25,000-part test pins that a hover does not sort
  again and that a frame builds a window.

### Categories and templates

The picker lists All parts, Recently placed, then each class. **Recently
placed** is the session log of parts placed from the panel
(`recent::RecentlyPlaced`, #2631): a refused place is not recorded, the
most recent comes first and the log keeps five. Its rows keep the log's
order until the user sorts or groups; a row whose library or row is gone is
left out.

Each row reads its own class template (`table/templates.rs`,
`RowTemplates`): a library's own template for the class
(`TemplateRegistry::insert_for_library`) wins over the global one for that
library's rows (Caner, 2026-11-27). A class category's columns are those of
every template its rows use, the leading template's first (the global one
when rows use it); its first view is the leading template's `[view]`. A
column that only a library's own template offers, and the global one does
not, names that library in the Columns picker ("Temp (b template)",
`TableCache::column_origin`). All parts and Recently placed show only row
fields.

### Previews

The **Previews** switch (`row_style`, the `components_panel_previews`
preference, #2535) adds a leading thumbnail column and makes every line a
Previews row tall. The thumbnail read is asked only for the rows in view
(`table::drawn_rows`) and the dragged row's card (`wanted_thumbnails`); the
cache and its refresh on a library re-read are `thumbs::SymbolThumbs`.

## Setups and persistence

A category opens with its class template's `[view]` (decision 3). Any change
to columns, widths, sort, grouping or pins stores the category's setup in
`ComponentsTable::setups` and writes the whole map to `prefs.json` key
`components_panel_views` (`fonts/components_views.rs`) through
`background_pref_write`, the atomic read-modify-write, off the UI thread
(decision 1). The category shown last is saved in the same entry and the
panel opens on it (Caner, 2026-11-27):
`{"category": "capacitor", "setups": {..}}`. At start the entry is read back; each setup is fitted to the
columns the category offers now (`ViewSetup::fitted`) for display only. An
edit is made on the setup as shown and saved over the stored one with what
that holds for columns not offered now (an unmounted library's) kept:
their place, hidden flag and width, and the stored sort or grouping when
the edit left it alone (`ViewSetup::keeping_unoffered`, #3236). The write
keeps what this build cannot read: a setup that does not parse (unless the
category now has a setup in memory), unknown fields of a setup, and other
subkeys of the entry. Reset to category default removes the category's
setup (pins other than the template's stay). Filters are not saved.

## Interactions

| Gesture | Message | Effect |
|---|---|---|
| Header click / Shift+click | `HeaderPressed` | `ViewSetup::click_sort`; Shift is tracked from `ModifiersChanged` in `dispatch/input.rs` |
| Header right-click | `HeaderRightClicked` | header menu: sort, add to / remove from sort, group by, hide, reset width, Columns… |
| Handle drag | `ResizeStarted` / `ResizeMoved` / `ResizeEnded` | width from the pointer's travel, 44 to 420 px; saved on release. A release anywhere, the pointer leaving the window or the window losing focus ends it (`window_subscriptions::column_resize_end`, #3236) |
| Group line click | `GroupToggled` | open or close by path |
| "+ Group", chip arrow, chip ✕ | `GroupAdded` / `GroupFlipped` / `GroupRemoved` | the setup's group levels |
| Columns picker | `ColumnShown` / `ColumnMoved` / `ResetPressed` | show, hide, ↑ ↓ (a visible column swaps with the next visible one of its kind, a hidden one with the next hidden one), Reset to category default |
| Filter popover | `RangeFilter`, `BoundPrefixPicked`, `RangeTargetPicked`, `InStockToggled` | a new range filter starts on Any prefix and a min-to-max one on Covers |
| Row press, right press, double-click | `ComponentsPanelRowPressed` / `RowRightClicked` / `ComponentsPanelPlace` | select and arm a drag, the row menu, place |
| Hover Place (first column) | `ComponentsPanelPlace` | place |
| Previews switch | `ComponentsPanelRowStylePicked` | thumbnail column and taller rows, saved as a preference |
| Esc with a popover open | `PopoverDismissed` | closes the popover before anything else Esc does in the panel (`dispatch/escape.rs`) while the panel is shown (in front in the dock, or the detached window the key was typed in, #3236); not from a detached modal's window, nor while a dialog takes the Esc |
| Details star | `PinToggled` | pin for the part's category |

Not built yet (#3199): dragging a header to reorder it or onto the group
bar, dragging group chips, the table's keyboard navigation, the text-value
filter chips, the Datasheet and Distributor buttons, the 2D / 3D footprint
toggle and clickable references. The header menu, the Columns picker's ↑ ↓
and "+ Group" cover the same edits meanwhile.

## Display rules

- A value that is not set draws "—" in a cell and "Not set" in a group line
  and in Details; it sorts and groups last in both directions.
- A min-to-max pair shows as "3 to 3.6 V". With one end set it shows "≥ 2.7 V"
  or "≤ 5.5 V" (Caner, 2026-11-27); it still counts as not set for sort,
  group and filters.
- °C is stored `degC` and shown `°C`; data rates show in bps with k, M, G.
- Stock is the largest stock any of the part's distributors last reported
  (`signex_library::best_stock`), so In stock only keeps a part any
  distributor has, as the Change Part session's In stock filter does
  (#3236). Price is the first offer's quantity-1 price.
- Details lists the pinned parameters, the rest under Show all. A part with
  parameters and no pins shows the Show all line alone; "No parameters." is
  only for a part that has none (#3236).
