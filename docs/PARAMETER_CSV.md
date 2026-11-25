# Parameter Manager CSV export and import

The Parameter Manager's footer has **Export…** and **Import…** (#1295); the
command palette runs the same two commands, `export_parameters` and
`import_parameters`. Neither has a default key or a top-menu row. CSV is the
only format for now; XLSX comes later (Caner, 2026-11-25).

The format code is `signex_output::parameter_csv`; the dialogs, the file IO
and the preview are `crates/signex-app/src/app/parameter_io/`.

## What Export writes

The parts the Parameter Manager lists on the active sheet: every placed
symbol except power ports, in sheet order. The row filter does not limit the
export.

- RFC 4180: comma separated; a cell with a comma, a quote or a line break is
  quoted, and its quotes are doubled. UTF-8 with a byte-order mark, CRLF line
  ends, so spreadsheets open it with the right encoding.
- The first line holds the column titles: `Unique Id`, `Reference`, `Value`,
  `Footprint`, then one column per parameter any listed part carries, sorted
  by name.
- One line per listed part, keyed by its `Unique Id`: the part's uuid, which
  re-annotation does not change. A part without a parameter has an empty
  cell.
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
sheet it was read for. Nothing changes until **Apply**; Cancel, Esc or closing
the window changes nothing. The list is worked out from the sheet as it is
now, so it follows edits made while the window is open.

- The titles `Unique Id`, `Reference`, `Value` and `Footprint` match in any
  case, with spaces around them. Every other title with text is a parameter,
  matched as written. A column with a blank title is skipped.
- A file with neither a `Unique Id` nor a `Reference` column, two columns of
  the same title, or text that is not UTF-8 is refused, and Messages says
  why.
- Value and Footprint are read-only in the Parameter Manager, so their
  columns are never imported.
- A line goes to the part its `Unique Id` names, whatever its Reference cell
  says. A unique id that names no listed part (one from another sheet, or a
  part deleted since), names a part an earlier line took, or is not a uuid
  leaves its line unmatched; its reference is never tried instead.
- A line with no unique id (a hand-made file, or a blank cell) goes by its
  reference, to the parts no unique id in the file names: the first such line
  naming `U1` to the first such `U1`, the second to the second, so the units
  of one part each take their own line. A line with neither is skipped.
- The window counts the parameters it adds (Before reads Not set) and the
  ones it changes, and counts and names the unmatched lines (by reference, or
  by unique id when the reference is blank).
- On a matched line, a cell changes a parameter whose value differs. An empty cell empties a
  parameter the part has, and adds none it lacks; a line that ends before a
  column leaves that parameter alone. Import never removes a parameter, and a
  parameter the file has no column for stays as it is.
- Apply writes every change as one undo step (one engine batch through the
  edit gateway), so one Undo takes the whole import back. The changed cells
  join the Parameter Manager's session: they carry the changed mark and
  Revert changes puts them back.
- Apply works on the sheet the file was read for while it is the active
  sheet. If another sheet is active, Messages says to switch back and the
  window stays open.
