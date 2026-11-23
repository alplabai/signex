# Schematic tables

A table is a drawing on a schematic sheet (#1300): placed, moved, locked,
copied, arranged and printed like the other drawings, and never part of the
netlist or ERC. Its rows are typed in, imported from CSV, or built from the
design.

## Placing a table

Place ▸ Graphics ▸ Table (`place_table`, also in the Active Bar's Drawing
Tools) places a typed table with three empty columns and three rows at the
click; the tool then goes back to Select. Properties shows it with a
`N rows · M columns` caption, an illustration of its style, and three
sections: Data source, Columns and Style.

## Sources

| Source | Rows | Refresh |
|---|---|---|
| Component pins | One row per pin of the chosen part on this sheet (every unit placed here): pin number, name, net, electrical type, unit, pin length. Sorted by pin number (2 before 10) or by name. | On compile (default) or Off |
| Typed / CSV | Typed or imported; never rebuilt. | — |
| Project sheets | One row per page of the project in hierarchy order: page number, sheet name, title, revision, last changed (the title block's date). One row per channel of a repeated sheet (`buck_channel · CH1`) or one per sheet. Each row remembers the sheet file it opens. | On compile (default) or Off |

The net of a pin is read from the sheet's own connectivity. A table keeps a
cell for every column, hidden ones included: unchecking a column in
Properties hides it on the sheet, checking it shows it again.

## Refresh

Bound tables are rebuilt only after a successful compile
(`Signex::refresh_bound_tables`), never on every edit, so a table does not
change under the cursor. Only the open sheets of the compiled project are
refreshed, and a Project sheets table reads that project's own compile;
another open project's sheets are never touched. Every sheet whose tables
changed gets one undo step; a sheet whose tables are already current gets
none. A title, revision and date come from the title block of the sheet file
the compile read, whether or not that sheet is open, so closing a sheet never
blanks its row. A row's link is the sheet's path relative to the sheet holding
the table (`power/buck.snxsch`, `../power/buck.snxsch`); a link that climbs out
of that folder opens only a file inside the project or one the last compile
read. A table set to Refresh Off keeps its rows. A table whose part left the sheet keeps its rows,
and Properties says the part is gone. The refresh is a normal edit through the
mutation gateway, so a sheet showing a channel tab or a read-only sheet
refuses it and Messages says why (`docs/MULTI_CHANNEL.md`, "Bound tables").

## CSV

A typed table's Import… reads a CSV file (comma separated, quoted fields may
hold commas, quotes and line breaks; the first line is the column titles) and
replaces its columns and rows in one undo step, on the sheet Import… was
pressed on even if another tab is active when the file is read; a table that
left that sheet meanwhile is said in Messages. Export… writes the shown
columns and every row. Both run their file dialog and file IO off the update
loop; an export is an atomic write.

## Table of contents sheet

Project ▸ Add Table of Contents Sheet (`add_table_of_contents`) writes
`contents.snxsch` next to the project's root sheet: a heading
(`<project> · Contents`) and a Project sheets table, filled from the last
compile when there is one. The write is atomic and refuses a file that is
already there: the name is claimed with a create-new open first, so a file
made there in the meantime is never overwritten. The new sheet is then added to the project the way Add
Existing adds a file. Double-clicking a row of the contents table opens that
row's sheet.

## Not built yet

- Editing a cell in place on the sheet: a cell edit is applied through
  `TablesMsg::CellEdited` (one undo step per cell), but the double-click
  editor on the canvas is not there yet.
- The contents sheet as page 1 of the numbering, and its tab's `auto` pill.
- Rows of the contents table as links in the exported PDF (they build on the
  PDF bookmarks work; #2663).
- Formulas in a typed table (the table is plain text, as drawn).
- The Properties illustration does not light a field's callout on hover yet.
