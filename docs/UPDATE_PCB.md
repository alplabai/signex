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
- headless, CLI and the agent channel (#2470):
  `update_pcb_from_schematic [document=<project, sheet or board>] apply=true`
  executes the rows ticked by default without a dialog, and `dry_run=true`
  lists them and changes nothing; both answer with the change list as JSON
  ([CLI.md](CLI.md), "Update PCB from schematic"). Without either, or while
  the dialog is open, it is refused; the dialog does not open over a run
  that waits for its list.

From the menus and the palette the project is the one that owns the
document in view; from the Projects tree, the clicked one; from operands,
the one `document` names, else the active project (the one opened last,
not the tab in view; with one project open and none active, that one).
When the active project has no board, or several projects are open and
none is active, the run is refused and lists the boards to name; another
project's board is never updated in its place (Caner, 2026-11-25). The
agent channel always
needs `document`: its gate acts only on a named document. When the board
is not open, it is opened first and the list is read once it is.

## The dialog

The dialog's header and footer stay put; the table scrolls between them, so
it works in a short window (#2371).

- **Header**: the title, with the project and the board on a second line.
- **Summary chips**: `Add n`, `Change n`, `Remove n` and `Problems n` (in the
  warning look when there is one). Clicking a chip lists only that kind;
  clicking it again lists every row. A pad that gets its first net is an Add
  and one that loses its net a Remove, so a new board lists only Adds. Next
  to the chips, **Show problems only**. An empty list says "Nothing to
  change".
- **Banner**, when something stands in the way:
  - no board in the project: **Create PCB** (New PCB, below), after which the
    list is read for the new board;
  - the list is stale (the board moved past the list's revision, or a sheet
    it read was edited): **Refresh**;
  - the board's tab closed with unsaved edits (parked): Execute is off,
    **Reopen** gives the board its tab back;
  - the board opened read-only (a newer format): Execute is off, with the
    reason.
- **Table**, one line per row: tick, Action (an icon and Add, Change or
  Remove), Object (a designator, a net, or a pad as `R1.2`), Detail (the
  footprint, with the value unless it is the `*` placeholder, or old → new)
  and Status. Hovering the Object shows the whole change as a sentence.
- **Status** is blank until Validate passes, then a green check on every
  row that applies. A ticked row that needs an unticked one reads Skipped.
- **Groups**: Components (every footprint row: add, remove, change,
  designator, value, link), Nets and Pad nets. Each folds behind a chevron,
  and has a box: solid accent with a white tick when every row that can be
  ticked is, solid accent with a white dash when some are; a click ticks them
  all, or unticks them all when all are ticked. Its
  `n of m` counts the ticked rows of those that can be ticked. A group with a
  problem starts open; a clean group with more than ten rows starts folded.
- **Problems**: a row that cannot be ticked is greyed, unticked, and shows
  the problem in short in its Status (the full reason as the tooltip) with
  what fixes it: **Choose footprint...** for a part with no footprint,
  **Annotate...** for a part that is not annotated or shares its designator,
  and the Cross Probe icon to show an unplaced part on its sheet. Annotate
  and Cross Probe close the dialog.
- **Notes** under the rows (an unreadable sheet, a pad that loses its net or
  lock, a kept unlinked footprint).
- **Footer**: the last Validate's verdict ("Validated: every ticked change
  applies to the board." with a green check, or what blocks it), Close, Report... (the list as text,
  written where the user saves it), Validate (runs the ticked rows on a copy
  of the board) and Execute, which names how many changes it applies and is
  off when none is ticked.

Keys: Space ticks or unticks the row last clicked, Enter executes, Esc
closes. The footprint picker over the dialog keeps Space and Enter.

### Choose footprint

Choose footprint... opens the footprint picker (every open library's
footprints). The pick is set on the part's sheet, on every placed unit, as one
undo step; the sheet comes forward and the list is read again, so the row
re-checks. A part whose library row offers the footprint (its default or an
alternate) is built with that binding, as Properties does. Any other part,
one with no row or whose row offers no footprint, names the footprint itself:
its footprint text becomes `<library uuid>:<footprint uuid>`, and the read
resolves that library footprint (the latest version, pin n on pad n) whenever
the part's row gives none. The sheet must be open; a closed one is reported
in Messages.

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
comes forward. The rules the schematic asks for are checked first: when they
cannot be stored (a rule set from a newer Signex, text that does not merge),
Validate and Execute say why, the dialog stays open and nothing is applied.

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
the dialog) gives a project its one board. In the app it first opens the New
PCB setup dialog (#2232, `PROJECT_SETUP.md`); its Create PCB runs the steps
below with the values shown, and Cancel or Esc makes nothing. A headless run
makes the board directly, from `profile=` or else the project's own setup.

- refused when the project's board is already open (before the dialog
  opens), or when a file of the new board's name (`<project name>.snxpcb`,
  next to the `.snxprj`) exists (checked on a worker after Create);
- the board starts from the dialog's values (copper layers, thickness, board
  setup rules); headless, from the named profile or the project defaults,
  else from the Standard profile;
- it opens as an unsaved tab and the project names it, so both are unsaved:
  nothing is written until the user saves.

## Compare Board with Schematic

Compare Board with Schematic (#2462, `compare_board_with_schematic`) lists
every difference between the project's schematic and its board, both ways,
and lets each row choose its direction. It reads the Update PCB change list
and the Back Annotate change list (`signex_engine::pcb::eco::compare`) in
one worker task at one board revision. Code:
`crates/signex-app/src/app/compare_board/` and
`crates/signex-app/src/app/view/dialogs/compare_board/`.

Entry points: Project ▸ Compare Board with Schematic…, the board right-click
menu's Compare with Schematic (on an object or a selection; the empty-area
menu stays short), and the command palette. It is gated like
Update PCB: an open project with a schematic or a board in view. A project
with no board or no root sheet is refused with a notice.

### Rows

| Kind | From | Update PCB does | Update schematic does |
|---|---|---|---|
| Missing on board | Add footprints, Links | Add to board, Link footprint | not available yet |
| Extra on board | Remove footprints | Remove from board | not available yet |
| Net differences | Nets, Pad nets | Add, remove or rename a net; move, connect or disconnect a pin | not available yet |
| Footprint mismatches | Change footprints (+ Back Annotate's footprint row) | Swap footprint | not available yet (#2653 wires it through Assign Footprints) |
| Value changes | Values, Designators (+ Back Annotate's rows) | Set value, Set designator | Take board value, Take board designator |

An Update PCB row and the Back Annotate row that change the same field of
the same footprint are one row with two possible directions. A side that is
blocked keeps its reason, shown as the tooltip of its greyed choice, and is
never chosen. The notes of both lists are listed under the rows.

A pad row that keeps its net and only gains its lock is not a difference, so
the report leaves it out and its Apply does not set that lock; Update PCB
still lists it. A pad row whose two sides read the same net name and that
other rows need, or that needs other rows, is not drawn either (mockup row
162): it is applied while a ticked row needs it, or while every row it needs
is ticked. A blocked one stays drawn with its reason, so the row that needs
it shows why it waits. A pad moving between two net numbers of one name with
no row tied to it is a real move and stays listed.

### Directions

Each row has a three-way choice: Update PCB (the schematic wins), Update
schematic (the board wins) or Skip. The defaults:

- Extra on board and Footprint mismatches start skipped: they are likely
  intended board edits, and one is a deletion.
- A row Update PCB starts unticked (a locked footprint, another library
  version) starts skipped.
- Any other row takes Update PCB when it can, else Update schematic, else
  Skip.

"Set shown rows" sets every row the kind chip shows; a row that does not
allow the direction keeps its own. A footprint row's direction changes the
board the net rows are computed for, so the net rows are recomputed on the
worker, keeping every direction that still means the same change. Compare
again reads both lists anew and keeps the directions the same way. While
either job runs the direction controls and "Set shown rows" are greyed and
take no pick, so a landing job cannot overwrite one or supersede a read.

### Apply

When a chosen row deletes from the board (a footprint or a net), a
confirmation lists every deletion first ("Removes TP3 from the board");
Cancel applies nothing. Apply then runs:

1. the PCB side: the Update PCB rows of the rows set to Update PCB, through
   Update PCB's Execute path, as one labelled board undo step at the read's
   revision. A board that moved since the read refuses it, and nothing is
   applied;
2. the schematic side: the Back Annotate rows of the rows set to Update
   schematic, as one Back Annotate undo group across the changed sheets. A
   closed sheet is stated again off the update thread first; one that
   changed on disk refuses Apply before anything is applied.

On success the dialog closes and Messages says how many changes went each
way. Export report… writes the report, directions included, as plain text
through the file port.

### From the command line

`check_board_against_schematic` answers the same report as JSON, with an
id for every row, and changes nothing; `apply_board_schematic_differences`
takes rows by id to the board or to the schematic and runs this Apply (one
board undo step, one schematic undo group). `board_status` counts the same
differences. Operands, answers and limits: [CLI.md](CLI.md), "Board against
schematic" and "Board status".

### Not yet

Update schematic is not available for Missing on board, Extra on board and
net rows: placing a board-only part on a sheet and carrying a board net to
the schematic need their own design (#2691). The command line refuses
those directions with the same reason. Taking a footprint mismatch into the
schematic through Assign Footprints is #2653 (Caner, 2026-11-20).
