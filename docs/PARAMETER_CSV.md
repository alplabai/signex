# Parameter Manager CSV export and import

The Parameter Manager's footer has **Export…** and **Import…** (#1295); the
command palette runs the same two commands, `export_parameters` and
`import_parameters`. Neither has a default key or a top-menu row. CSV is the
only format for now; XLSX comes later (Caner, 2026-11-25).

The format code is `signex_output::parameter_csv`; the dialogs, the file IO
and the preview are `crates/signex-app/src/app/parameter_io/`.

## What Export writes

The parts the Parameter Manager lists, on the sheets its Active sheet / All
sheets switch lists (#1784): every placed symbol except power ports, sheet
by sheet (the active sheet first), in sheet order. The row filter does not
limit the export.

- RFC 4180: comma separated; a cell with a comma, a quote or a line break is
  quoted, and its quotes are doubled. UTF-8 with a byte-order mark, CRLF line
  ends, so spreadsheets open it with the right encoding.
- The first line holds the column titles: `Unique Id`, `Reference`, `Sheet`
  (only with All sheets), `Value`, `Footprint`, then one column per
  parameter any listed part carries, sorted by name. An Active sheet export
  has no `Sheet` column, as before.
- One line per listed part, keyed by its `Unique Id`: the part's uuid, which
  re-annotation does not change. `Reference` is the designator the table
  and the canvas show: on a child sheet placed more than once, the
  designator of the placement the sheet's tab shows (#1151), not the stored
  reference. `Sheet` is the name the table's Sheet column shows: the file
  name, or the path from the project folder where two listed sheets share a
  file name. A part without a parameter has an empty cell.
- A cell a spreadsheet would run as a formula (one starting with `=`, `@`, a
  tab or a carriage return, or with `+` or `-` other than a signed number with
  a unit such as `-5V`) is written with a `'` in front, the same guard as the
  BOM export. A value that already starts with quotes in front of such a cell
  gets one more, so Import gives back every value exactly as it was.

The file is written with an atomic write (temp file, fsync, rename), off the
update thread. Messages says where it went, or why it was not written.

## What Import does

Import… asks for a CSV file, reads it off the update thread (4 MB at most) and
opens the **Import Parameters** window with the changes it would make to the
sheets it was read for: the ones the Parameter Manager listed when Import…
was pressed (the active sheet alone with Active sheet). Nothing changes until
**Apply**; Cancel, Esc or closing the window changes nothing. The list is
worked out from the sheets as they are now, so it follows edits made while
the window is open; a listed sheet closed since takes no line.

- The titles `Unique Id`, `Reference`, `Sheet`, `Value` and `Footprint`
  match in any case, with spaces around them. Every other title with text is
  a parameter, matched as written. A column with a blank title is skipped.
  (So a parameter named `Sheet` cannot be imported.)
- A line with a `Sheet` cell goes only to a part of the listed sheet of that
  name; a line without one, or a file without the column, to a part of any
  listed sheet. A sheet name no listed sheet has leaves its line unmatched.
- A file with neither a `Unique Id` nor a `Reference` column, two columns of
  the same title, or text that is not UTF-8 is refused, and Messages says
  why.
- Value and Footprint are read-only in the Parameter Manager, so their
  columns are never imported.
- A line goes to the part its `Unique Id` names, whatever its Reference cell
  says. A unique id that names no listed part (one from another sheet, or a
  part deleted since), names a part an earlier line took, or is not a uuid
  leaves its line unmatched; its reference is never tried instead. So does
  an id that parts of two listed sheets carry (a copied sheet file) on a line
  with no `Sheet` cell: the line is never written to a guessed sheet.
- A line with no unique id (a hand-made file, or a blank cell) goes by its
  reference (the designator the table shows), to the parts no unique id in
  the file names: the first such line
  naming `U1` to the first such `U1`, the second to the second, so the units
  of one part each take their own line. A line with neither is skipped.
- The window counts the parameters it adds (Before reads Not set) and the
  ones it changes, and counts and names the unmatched lines (by reference, or
  by unique id when the reference is blank).
- On a matched line, a cell changes a parameter whose value differs. An empty cell empties a
  parameter the part has, and adds none it lacks; a line that ends before a
  column leaves that parameter alone. Import never removes a parameter, and a
  parameter the file has no column for stays as it is.
- Apply writes every change as one undo step, through the Parameter
  Manager's own edit path: one engine batch through the edit gateway when
  only the active sheet changes, else one undo group across the sheets, all
  or nothing, so one Undo on any of them takes the whole import back. The
  changed cells join the Parameter Manager's session: they carry the changed
  mark and Revert changes puts them back.
- Apply works while the sheet Import… was pressed on is the active sheet. If
  another sheet is active, Messages says to switch back and the window stays
  open. A sheet that opens read-only, or that shows a channel tab, refuses
  the whole import.
