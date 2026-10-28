# Hierarchical sheets: creating and wiring the child sheet

A sheet symbol on a parent sheet names a child file (`filename`) and carries
entries (pins) on its border. The child file holds one Hierarchical label per
entry; the netlist stitches the two by name.

## Creating the child sheet {#creating-the-child-sheet}

- **Where.** Right-click a sheet symbol, then "Create Sheet From Sheet Symbol".
  The file is written beside the PARENT sheet, at the path descending resolves
  (`resolve_child_sheet_path`), so the next double-click opens what was made.
- **What.** A blank sheet plus one Hierarchical label per entry, with the
  entry's name and direction (`signex_engine::child_document_for_symbol`).
- **Registered.** The file is added to the project's sheet list, relative to
  the project folder, and the `.snxprj` is marked changed; it is saved with
  the project like any other Add New. The Projects tree shows it at once. A
  file outside the project folder is created but not listed, and the Messages
  panel says so.
- **Never overwrites.** The write is a temp file, fsync and a no-clobber
  rename on a worker thread. If the file already exists the sheet is just
  opened.
- **Not undone.** Writing the file is like Save: it is not an edit, so Undo
  does not remove it; the Messages panel records "Created child sheet ...".
- **Double-click without a file.** Descending through a symbol whose file is
  missing (double-click, Enter, "Open Child Sheet") asks whether to create it
  instead of doing nothing. If the parent sheet is not saved there is no folder
  to create it in, and a notice says to save the parent first. The dialog is a
  confirm dialog: it locks the macOS system menu bar like the other confirms
  (docs/MENU_BAR.md).

## Sheet Entry tool {#sheet-entry-tool}

The Active Bar's Sheet Entry places one entry per click on the border of the
sheet symbol under the cursor, snapped to the nearest edge and kept off the
corners (`signex_engine::sheet_symbol_at`, `new_sheet_entry`). The name is the
first unused `P1`, `P2`, ... on that symbol, and the direction follows the edge
(input left, output right, bidirectional top and bottom); edit both in
Properties. Placement is `Command::PlaceSheetPin`: one undo step, and a blank
or repeated name on a symbol is refused. A click away from every symbol places
nothing and says so in the Messages panel.
