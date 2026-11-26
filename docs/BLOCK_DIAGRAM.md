# Block diagram tab

The block diagram is a document of its own, a `.snxblk` file (mockup row
171, #2796). The file format is in
[architecture/snxblk-format.md](architecture/snxblk-format.md); the
commands are in [CLI.md](CLI.md), "Block diagrams". This page is how the
tab behaves.

## Opening and making one

- File ▸ New ▸ Block Diagram (`new_block_diagram`) makes an empty diagram
  named after the open project, in the project's folder, and opens it
  unsaved. Greyed with no project open.
- File ▸ Open, or `open_document`, on a `.snxblk` reads the file in a task
  and opens its tab when the read lands. A file from a newer Signex opens
  read-only: the toolbar's edits are greyed with the reason, and the bar
  says Read-only.
- One tab per diagram, in the main window only: it does not undock, and
  History does not restore a diagram yet.

## Editing

- The toolbar has the Select and Connect tools, Add Block, Add Port, Group,
  Delete, Undo, Redo, Save and Fit. A button with nothing to act on is
  greyed and its hint says why.
- On the canvas a click picks a block, a port, a connection or a group's
  frame; Shift adds to the selection. A drag on a block moves it and lands
  as one edit on release; a drag on empty canvas, or with the middle
  button, pans; the wheel zooms at the pointer. A double click on a block
  linked to a sheet opens that sheet.
- With Connect, a click on a block or a port is the start, a second click
  on another is the end: one connection, interface Signal, no label.
- Properties shows what one selected object has. A block: name, function,
  note, shape, position, size, group, rails, key specs, colour, its sheet
  link and its ports. A connection: its ends, interface and label (a name
  for an interface the list does not have). A group: name, colour and
  block count. A port is shown, not edited: the model has no port edit yet.
  Text is committed with Enter; a value the model refuses (a blank name, a
  size of zero, a spec with no `=`) is named in Messages and changes
  nothing.
- Every edit, from the tab or a `block_*` command, goes through one commit
  path (`app/block_diagram/edit.rs`): one undo step, refused on a read-only
  diagram, the dirty mark set exactly when the diagram differs from its
  file. Undo back to the saved state reads clean.
- The view holds still while the diagram changes; Fit fits it again.

## Drawing

Groups draw first as dashed frames, then connections in right angles with
an arrow at their end and their label at the middle, then blocks with name,
function and note, and their ports as small squares. A power connection is
drawn in the error colour and a bus thicker in the accent, as the mockup's
legend reads; a block whose sheet left the project has a warning edge. The
geometry (port points, routes, label points, frames, hits) is
`signex_types::block_diagram::geometry`, so an export draws the same
picture.

## Saving

Save writes the diagram in a task with an atomic write and marks it clean
with the diagram it wrote, so an edit made while the write runs stays
dirty. The first save of a new diagram refuses to replace a file already at
its path. Closing the tab of a diagram with unsaved edits keeps the diagram
held: Save All and the quit prompt see it, and opening the file again shows
the edits.
