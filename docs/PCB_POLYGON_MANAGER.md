# Polygon Manager

Every polygon pour of a board in one table (mockup row 178). It is a page
tab of the board, in the shared document-tab shell, and it pairs with the
PCB panel's Polygons mode (mockup row 106): one model, two pure views
(DECISIONS 2026-11-18, "Tab + compact panel pair").

## Opening it

- Tools ▸ Polygon Manager... runs `polygon_manager` on the board in view.
- The board menu offers Polygon Manager on a polygon pour; the pours
  selected on the board are ticked in the tab, or else the pour under the
  pointer.
- The PCB panel's Polygons mode has Open Polygon Manager in its header.

All three send `PolygonMsg::ManagerOpened`. A second open focuses the tab.
The tab sits after its board's tab (the board page rules in
[PROJECT_TAB.md](PROJECT_TAB.md), "Page tabs"), shows the board's dirty dot,
and Save and Undo act on the board.

## The tab

- **Toolbar.** Repour all, Repour selected; Shelve all, Unshelve all, Shelve
  selected; the status "N pours · N shelved", with "· N not filled" when a
  pour has no fill. Repour all skips shelved pours. The buttons that act on
  the ticked rows are greyed while none is ticked.
- **Sub-tabs.** Pours (every pour), Shelved (the shelved pours only) and
  Bulk edit.
- **Filters.** Layer, Net and State (All, Poured, Shelved, Not filled). An
  unfilled pour (#2506) is Not filled and counts in "· N not filled" until
  it is repoured.
- **Table.** One row per pour, grouped by copper layer in stack order, each
  layer in pour order with its shelved pours last. Columns: a tick box,
  Order (the place in the fill order of the layer, from 1; a shelved pour
  has none), Name (the board's pour naming scheme, numbered in board
  order), Net, Layer, State, Clearance (empty follows the board), Min
  width, Islands below, Dead copper, Area mm², Pieces, and Repour and
  Shelve / Unshelve. Clearance, Min width and Islands below are typed in
  place and applied with Enter; an emptied Clearance follows the board
  again. Dead copper is a tick box. A shelved pour's row is dimmed. A click
  on the name ticks that row alone and shows it on the right.
- **Typed text.** Text typed and not applied is dropped when the board
  changes (an edit, undo, redo), so every cell shows what the board stores;
  a bulk field's text is dropped too when the ticked rows change.
- **Fills.** No canvas fills the board while its Polygon Manager tab is in
  view, so the tab fills it for the current revision whenever the panels
  are rebuilt (a tab switch, a reload, an edit).
- **Detail.** The pour picked last: its net, layer, copper, the islands its
  rules removed, and the issues of its fill (why it has none).
- **Bulk edit.** With two or more rows ticked, one field per typed column
  and Dead copper. A field the ticked pours do not share is empty and reads
  "mixed". Enter applies it to every ticked pour as one undo step.

## Edits

Every edit is the command the pour's Properties runs for the same field
(`signex_engine::pcb::primitive_edit::zone_edit_command`, or
`pour_edit::shelve_all_pours`), one board undo step under the same label,
so it shows in Properties at once. A read-only board takes no edit. A typed
text that is no number changes nothing and says so in Messages. Repour is
the Repour Selected / Repour All path (see [PCB_TOOLS.md](PCB_TOOLS.md),
"Polygon Pour"): the fill is computed, not stored, so it makes no undo step.
Nothing here writes to disk; pours save with the board.

The Order column reads `pcb_geom::pour_fill_order`, the order the fill uses:
a higher Priority first, then board order. Pours are reordered through
Priority in Properties.

## The Polygons mode of the PCB panel

A header with Open Polygon Manager, Repour all, the All / Needs attention
chips (Needs attention keeps the shelved pours and those with no fill), and
one line per pour: its name, net and layer, the state chip, Repour and
Shelve / Unshelve. A click on a line picks the pour the tab shows.

## Not built yet

- Out of date, Repour modified only and Auto-repour after edits: a pour's
  copper is computed from the board as it is now, so it is never out of
  date. They need a stored fill first.
- The Pour order page and dragging a row to reorder pours.
- Violations (a pad of the pour's net left unconnected, a neck below Min
  width) and their Zoom to / Select links.
- The preview drawing with editable values and corner handles, the
  overlap view, and Expand / Fit.
- Typed names, Fill mode, Pad connect and Corners: the board stores no pour
  name, and Hatched, thermal relief and corner shapes are not filled.
- Ctrl-click and Shift-click selection, the row ⋯ menu and right-click menu,
  Duplicate to layer and Delete, Last poured.
- A pour picked in the panel is not highlighted on the board.
- A branch switch does not reopen the tab.
- Esc in a typed cell does not cancel its text.
- The layer names over the rows scroll sideways with the table; they are
  not pinned to the left edge.
- The Shelve all, Unshelve all and Shelve selected messages have no Undo
  link; Undo on the board takes the step back.
- The detail column shows the pour's values; they are typed in the table
  or in Properties, not in the detail column.
- A right-click in an undocked board window: the pours ticked are read
  from the main window's selection and hover, not checked for that window.
