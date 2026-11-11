# Update PCB from schematic

The Update PCB dialog brings a project's board in line with its schematic
(#1312). The change list itself (which rows exist, which start ticked, what
blocks a row) is `signex_engine::pcb::eco`; this page is the dialog, its
entry points and New PCB. Code: `crates/signex-app/src/app/update_pcb/` and
`crates/signex-app/src/app/view/dialogs/update_pcb/`.

## Entry points

All of them run the one command `update_pcb_from_schematic` (mutating,
undoable):

- Design ▸ Update PCB Document `<board>` on a schematic, Design ▸ Import
  Changes From `<project>` on a board ([MENU_BAR.md](MENU_BAR.md)), in the
  main window and in an undocked window's own menu bar;
- the command palette and any key a profile binds to it (none by default);
- the Projects tree's project row, Update PCB Document, for the clicked
  project;
- headless and CLI: `update_pcb_from_schematic apply=true` executes the rows
  ticked by default without a dialog and answers `Ran`; without `apply=true`
  it is refused.

The project is the one that owns the document in view. When its board is not
open, it is opened first and the list is read once it is.

## The dialog

- **Header**: the project and the board.
- **Summary**: how many rows are ticked, or "Nothing to change".
- **Banner**, when something stands in the way:
  - no board in the project: **Create PCB** (New PCB, below), after which the
    list is read for the new board;
  - the list is stale (the board moved past the list's revision, or a sheet
    it read was edited): **Refresh**;
  - the board's tab closed with unsaved edits (parked): Execute is off,
    **Reopen** gives the board its tab back;
  - the board opened read-only (a newer format): Execute is off, with the
    reason.
- **Rows**, grouped (add, remove and change footprints, designators, values,
  links, nets, pad nets). Each group has a box: on when every row that can be
  ticked is, drawn mixed (soft accent) when some are; a click ticks them all,
  or unticks them all when all are ticked. A blocked row has no box to tick
  and says why, with **Annotate...** for a part that is not annotated or
  shares its designator, and **Cross Probe** to show an unplaced part on its
  sheet. Both links close the dialog.
- **Notes** under the rows (an unreadable sheet, a pad that loses its net or
  lock, a kept unlinked footprint).
- **Footer**: Close, Report... (the list as text, written where the user
  saves it), Validate (runs the ticked rows on a copy of the board) and
  Execute.

Esc closes the dialog.

## Off the update thread

The design read runs on a worker: the project's sheets are assembled (open
sheets with their unsaved edits, the rest from disk), the netlist is stitched,
each part's footprint is resolved from its library row (the alternate it was
placed with, else the row's default, through `resolve_part_footprint`) and the
board is diffed. Ticking a footprint row recomputes the net rows on the worker
too. Refresh reads again and keeps the user's ticks
(`EcoChangeList::keep_choices_from`).

## Execute

The ticked rows are lowered to board commands and run as one labelled step at
the list's revision (`execute_board_commands` with a `CheckedStep`): all or
nothing, one undo step, refused once the board moved on. On success the dialog
closes, the Messages panel says how many changes were applied and the board
comes forward.

## Decisions

- A part whose footprint cannot be resolved (no library row, a library that is
  not open, a footprint the board cannot hold) is a blocked row; the rest
  still runs.
- DNP parts are placed; parts marked not on the board are removed.
- Another library version of the same footprint is listed, unticked.
- Remove and change rows of a locked footprint start unticked; pads whose net
  is locked are left alone.
- A footprint with a 3D body or a STEP model converts; the board reaches the
  model through `library_ref`.

## New PCB

`new_pcb_document` (File ▸ New ▸ PCB, Projects ▸ Add New ▸ PCB, Create PCB in
the dialog) gives a project its one board:

- refused when the project already names a board, or when a file of the new
  board's name (`<project name>.snxpcb`, next to the `.snxprj`) exists (checked
  on a worker);
- the board starts from the project defaults (#2232: copper layers,
  thickness, board setup rules), else from the Standard profile;
- it opens as an unsaved tab and the project names it, so both are unsaved:
  nothing is written until the user saves.
