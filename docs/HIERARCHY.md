# Hierarchical sheets: creating and wiring the child sheet

A sheet symbol on a parent sheet names a child file (`filename`) and carries
entries (pins) on its border. The child file holds one Hierarchical label per
entry; the netlist stitches the two by name.

## Creating the child sheet {#creating-the-child-sheet}

A sheet symbol whose file does not exist opens the **Create Sheet** dialog:

- **When.** Right after the Place Sheet tool commits a symbol, and when the
  symbol is descended into without a file (double-click, Enter, "Open Child
  Sheet", the context menu's "Create Sheet From Sheet Symbol"). The Navigator's
  missing rows ask the same way.
- **Where.** The field is the file path, relative to the PARENT sheet's folder
  (default: the symbol's file name, so beside the parent). A sub-folder path
  such as `power/regulator.snxsch` is fine; "Browse..." opens the native save
  dialog in the parent's folder. The path must end in `.snxsch` and resolve
  inside the project folder (`..` and absolute paths that leave it are refused
  with an inline error and nothing is written). Missing sub-folders are created.
- **Create.** Writes a blank sheet plus one Hierarchical label per entry on
  the symbol (`signex_engine::child_document_for_symbol`), on a worker thread
  with a temp file, fsync and a no-clobber rename. The file is listed
  in the project's `sheets` with its project-relative path (`.snxprj` marked
  changed, saved with the project), the symbol's File field is set to the path
  relative to the parent sheet (the note under the field says so), and the
  child opens with the placement recorded. Enter in the field is Create.
- **A file that already exists.** It is never overwritten. The dialog says so
  in an error box and offers "Use existing file", which points the symbol at
  that file and opens it, writing nothing. It is not the default: Create stays
  the primary button, disabled while the typed name exists, and Enter in the
  field does nothing, so picking another name is the safe path. When the
  project already has that file as a page, or a sheet symbol on another open
  sheet places it, a warning says that using it makes this symbol a copy of
  that sheet ("mcu.snxsch is already sheet 3; ..."; the number is the file's
  position in the project's sheet list, left out when the file is not
  listed). A file another sheet symbol on the same parent already points at is
  refused instead, with no "Use existing file" (#1835).
- **Later.** Keeps the symbol, writes nothing, and says so in the Messages
  panel. The footer hint beside it says the same.
- **Which document.** The dialog holds the parent sheet's path and the symbol's
  id and acts on that document, whatever tab is active when it is answered.
  If the symbol has vanished, the Messages panel says so. An unsaved parent
  sheet (no file) gets a notice instead of the dialog.
- **Undo.** Writing the file is like Save: it is not an edit and is not undone;
  the Messages panel records it. The File-field change is one undo step, as is
  each sheet-entry placement.
- **Window.** It is a confirm-class dialog: it opens in its own window and locks
  the main window and the native menu (docs/DETACHED_MODALS.md).

## Sheet Entry tool {#sheet-entry-tool}

The Active Bar's Sheet Entry places one entry per click on the border of the
sheet symbol under the cursor, snapped to the nearest edge and kept off the
corners (`signex_engine::sheet_symbol_at`, `new_sheet_entry`). The name is the
first unused `P1`, `P2`, ... on that symbol, and the direction follows the edge
(input left, output right, bidirectional top and bottom); edit both in
Properties. Placement is `Command::PlaceSheetPin`: one undo step, and a blank
or repeated name on a symbol is refused. A click away from every symbol places
nothing and says so in the Messages panel.

## Navigator panel {#navigator-panel}

The Navigator lists the project's sheet hierarchy (Graphite mockup #54)
under a Sheets heading with the sheet count. Each row has the sheet icon, the sheet name, its file name in muted mono and,
on the right, the page number from the project's sheet numbering; a child
sheet placed twice has a row and a number per placement. The focused sheet
has the accent bar and fill. Missing, unreadable, rejected and cycle rows
read in the error colour; loose pages are muted italic. The Filter sheets
field keeps the rows whose sheet name or file name holds the text, with
their parents; when nothing matches, a card names the filter and offers
Clear filter. With no project open the panel shows a card with
Open Project…. A click highlights a row; a double click opens the sheet.
