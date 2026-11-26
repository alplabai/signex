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
- The Projects tree lists every `.snxblk` in a project's folder (three
  folders deep, hidden folders skipped) and every diagram held open there,
  under Source Documents, with the `.snxblk` icon and a BLK tag; a double
  click opens its tab. The folder is listed by the tree's off-thread file
  probe, never in `update` or `view`.
- A command that names a diagram that is not open reads it first and holds
  it without a tab ([CLI.md](CLI.md), "Block diagrams").

## Editing

- The toolbar starts with the view switch (Block diagram, Power tree) and
  Re-sync, then the Select and Connect tools, Add Block, Add Port, Group,
  Delete, Undo, Redo, Save and Fit, and SVG and PDF…. A button with nothing
  to act on is greyed and its hint says why.
- Re-sync rebuilds blocks and connections from the sheet hierarchy and
  keeps the layout (`regenerate_block_diagram apply=true`). On the open
  diagram it is one undo step and writes nothing until Save; an edit made
  while it read the hierarchy wins and Messages says nothing was applied.
- Added blocks and renamed or moved sheets apply at once. A block whose
  sheet left the project is not unlinked at once: the "Re-sync from sheets"
  side sheet (mockup row 210) opens in Properties' place, with one checked
  row per such block. The diagram previews the run while it is open: the
  added blocks dashed in the added colour, the renamed or moved ones
  outlined, and each checked row's block hatched amber, each with its tag
  ("new", "renamed", "moved", "unlinked") in the same colour. Hovering a
  row outlines its block. Apply selected
  unlinks the checked blocks as one undo step; Cancel leaves them. An
  unchecked block is not remembered: the next Re-sync lists it again,
  checked. A Re-sync run while the side sheet is open and finding nothing
  to unlink shows "Nothing to change".
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

## Power tree view

The view switch's Power tree reads the project's power tree off the update
thread with the run behind `query_power_tree`, for the variant in the
toolbar's Variant field (the project's active one until a person picks;
picking does not change the project), each time the view or a variant is
picked ([POWER_TREE.md](POWER_TREE.md)). The totals line names the scenario
(Max load, every fitted load at its stated draw, the one scenario there
is), how many sources feed a load in it, and the rail with the least
headroom. One row per rail: its sources (a converter or a source part), the
rail with its voltage and total draw, and its loads. A source's bar is the
rail's draw against its limit, green under 50 %, amber to 90 %, red above;
a load's bar is its share of the rail, accent under 50 %, amber to 90 %,
red above. A click on a source or load opens its sheet at the part's sheet
occurrence with the part selected. The side panel lists the rails; a click
on one shows its nominal voltage, source, limit, load and headroom. Showing
the tree changes nothing in the diagram.

Show GND on the Block diagram view's toolbar draws and picks the ground
connections (a power connection whose net is a ground); it is off, as
drawn. It is view state: nothing is saved, and an export draws every
connection.

## Export popover

SVG and PDF… open a popover over the canvas: the format, print colours or
as shown, and for a PDF its Pages (Block diagram and Power tree ticked,
Budget table not, as drawn), its Paper (A4, A3, A2, Letter, Tabloid,
landscape or portrait) and title block, and the file, offered as
`outputs/<name>_blocks.<svg or pdf>` beside the diagram. Export runs
`export_block_diagram` with those operands
([BLOCK_DIAGRAM_EXPORT.md](BLOCK_DIAGRAM_EXPORT.md)); the saved file is
exported, and the popover says so while the diagram has unsaved edits.

## Saving

Save writes the diagram in a task through the document save tracker and
marks it clean with the diagram it wrote, so an edit made while the write
runs stays dirty. Writes to one file are ordered and quit waits for a
running one. A file another program changed since it was read or saved is
not overwritten: the save is refused with a notice and the edits stay
(`docs/architecture/external-change.md`). The first save of a new diagram refuses to replace a file already at
its path. Closing the tab of a diagram with unsaved edits keeps the diagram
held: Save All and the quit prompt see it, and opening the file again shows
the edits.
