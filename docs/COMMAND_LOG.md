# Command Log

The Command Log panel (#2458) lists every catalog command this session ran,
newest last: from a menu, a key, the command palette, a context menu or a
tool, from `signex-cli` working through the running app, and from any other
client of the agent socket. Open it with **View ▸ Command Log** or from the
status bar's panel list (group Verify). It docks at the bottom by default and
can be moved like any panel; its saved layout key is `command_log`.

The model is `app::core::command_log` (UI-free); the app records rows in
`app::handlers::command_log` and the panel is `panels::command_log`.

## What is logged

- **One row per command.** A command dispatched through
  `Signex::dispatch_command_reporting` (menus, keys, palette, context menus,
  an undocked window's rows and keys) adds one row when it returns. A command
  run inside another (an undocked window's Undo stepping that window's
  document, a command an agent request runs) is not logged again: the outer
  call owns the row.
- **Agent socket requests** add one row each, recorded where the request is
  answered (`app::agent::handler`), whatever the request ran: a query,
  `apply_edits`, an operand handler, or nothing because it was refused. A
  request whose session token did not match names no command and is not
  logged; it stays an access-journal event. The session token never reaches
  the log: only the request's `args` and `client` are read.
- A request whose reply waits for its reads is logged at once as waiting and
  takes its outcome when the reply is sent.
- **A finished operation** is one row. A tool or dialog a command started
  (Move, Change Footprint, a union) gives its input to that command's row when
  it finishes; started and cancelled, the row has nothing to replay. A move, a
  Properties edit or a union edit made without a command gets a row of its
  own, named after the catalog command it matches. Keystrokes in one field
  within two seconds share one row, which keeps the last value.
- **Not logged:** other pointer gestures (a click on the canvas, a placement
  click), other panel interactions, and the panel's own actions.

Each row keeps the time with milliseconds, the source, the catalog id and its
label in the UI language, the operands as given (name to value), the outcome,
how long the synchronous part of the dispatch took, the document it acted on
(its `document` operand, else the window's document) and that document's
project file. A GUI command has no operands; one that acts on the selection
records its targets (kind and stable id) as the `recorded_targets` argument,
and a finished edit records what it took (offset, angle, values) as
`recorded_input`. Both show in the detail pane and the saved log and are left
out of the CLI line.

## Sources

| Source | When |
|---|---|
| GUI | the person: main window, or an undocked window (the detail pane names its document) |
| CLI | an agent request whose `client` is `signex-cli` |
| Agent socket | any other agent request; the detail pane shows its client name, or its pid when it gave none |

A headless `signex-cli` run has no window and no panel; its own app logs its
commands as agent rows that nobody sees.

## Results

- **Done**: the command ran (`CommandOutcome::Ran`, or an agent reply that ran
  or answered; an agent reply's reason, when it has one, is the row's text).
- **Failed**: the command was refused or did not succeed. The reason is kept
  verbatim (`Disabled { reason }`, an unknown id, an id with no dispatch arm,
  an agent refusal). The table's Result cell shows the part before the first
  `:` (the error code) with the whole reason as its tooltip; the detail pane
  shows the whole reason first, with Copy error.
- **Warning**: a request still waiting for its reads.

## Undone rows

A successful `undo` marks the newest row of the same document that the catalog
calls undoable (`CommandFlags::undoable`), did not fail and is not undone yet;
a successful `redo` clears the mark of the row of that document undone last.
Undone rows stay in the log, struck through, so the log is a record and not
the undo stack. A step that has no row (a placement click) is not in the log,
so the row marked is the newest logged undoable command of that document.

## Filters

- The search field matches the label, the id and the argument text, without
  regard to case.
- Source chips GUI, CLI and Agent socket, each with its count; a source whose
  chip is off is hidden.
- All / Failed N / Undone N.
- **View commands** (catalog category `view` or `select` that do not change a
  document) are recorded but hidden until More ▸ Show view commands is on;
  the counts leave them out while they are hidden.
- **Follow newest** keeps the newest row in view.

With nothing logged yet every count reads 0, the filters are greyed and the
panel says what it will list.

## Panel width

All six table columns stay at any width. Command, Arguments and Result shrink
together to their least widths, then the table scrolls sideways, header and
rows together. The detail pane takes at most half the panel. In the filter
bar the search field shrinks first, then the filters wrap; in a narrow side
dock Follow newest and the actions take their own line, so they are never
clipped.

## Actions

- **Pause logging** stops recording until Resume logging; **Clear** empties
  the log (numbering goes on).
- **Save log as…** asks for a file through the file port (`io_ports`) and
  writes the whole log there as JSON Lines, one object per row (`seq`,
  `time`, `source`, `client`, `pid`, `window`, `id`, `args`, `result`, `text`,
  `took_ms`, `document`, `undoable`, `undone_at`), off the update thread.
  Nothing is written to disk unless asked.
- **Replay refuses unless proven safe** (DECISIONS 2026-11-22, #2840). A row
  runs only if its command is on the safe list
  (`command_log_replay/safe_list.rs`) and its target is unchanged. Anything
  else, a new command included, changes nothing and adds a failed row saying
  why: "can't be replayed" for a command off the list or a row of a kind its
  proof does not cover, "recorded no state" for a listed command with nothing
  to check (an agent's row, a run that changed nothing, one that finished
  after its reads), and "changed after" for a changed target.

  | Command | Proof |
  |---|---|
  | `edit_object_properties` | Only a recorded edit of whole objects by id. On a sheet, a drawing, directive or harness connector writes only the fields it changed: it runs when another field changed since and keeps that change, inside a list's elements too, and refuses only if the same field holds a third value. An edit that writes a value derived from other fields runs only on the object as it found or left it: a Rect's size or place, an Arc's geometry, a dimension's direction or an override taken from the measured text, a table's source or a bound table's rows, a directive's outline, a connector's rotation, a mirrored Bezier handle, and any edit that changes more than one field. On a board, each written item or value against its guard. |
  | `move_selection` | A recorded offset on recorded ids, on a sheet or board. Each target must be where the move found it or left it, so a replay cannot pull it from a later position. A sheet move also guards a fingerprint of everything that makes a net: wires, buses and entries, junctions, labels, no-connects, symbols and their library pins, sheet symbols with their entries, No-ERC markers and harnesses. A board move refuses if the offset would now carry a track it did not write. |
  | `delete_selection` | Recorded ids on a sheet or board, each exactly as the command found it, so an item edited since is not removed. A target already gone is named in the refusal. |
  | `duplicate` | Recorded source ids on a sheet or board, each exactly as found. Copies take new ids; the clipboard is kept. The copy lands at a fixed offset, so the row is refused while an object of the same kind already stands there. |
  | `lock_board_items`, `unlock_board_items` | Tracks and vias by uuid, an absolute flag. Every id must be one the row wrote, since an item already at the value records nothing. |
  | `set_pad_shape` | Pads by uuid, absolute pad-local shape. The whole pad is guarded, and every id must be one the row wrote. |
  | `panelize_board` | Writes the constant default panel; the board's panel is guarded. The outline only validates. |
  | `remove_board_panel` | Writes no panel; the panel is guarded. |
  | `set_board_panel` | One stored setting; the whole panel is guarded. |
  | `set_layer_color`, `reset_layer_colors` | A colour keyed by its layer token; all board options are guarded. |
  | `set_layer_stack` | The whole stack. Its rows name the layer each was, so a rerun on its own result would rename copper: it runs only on the stack, layers and thickness it found, and is refused while a via or pad holds a per-layer size on a layer it removes. |

  Off the list until a guard proves them: every other relative or selection
  command (rotate, mirror, flip, cut, `move_object`, `move_selection_xy`,
  `drag_keep_connections`); view commands and tools; `undo` and `redo`; every
  footprint and symbol editor row, Move and Delete included, since they name
  pads and pins by index; `board_shape_from_selection` and `arrange_in_box`
  (absolute writes over later edits); `set_board_graphic`, `add_board_graphic`
  and `set_footprint_text` (operands resolve through the live origin or
  designator); `set_via_span` and `set_layer_sizes` (layer names resolve by
  place in the live stack); `reset_board_origin` (reads the live outline);
  DRC waivers; `import_dxf` and `import_dxf_outline` (read a file again);
  `update_pcb_from_schematic`, tuning, teardrops, stitching, gloss and keepout
  (recompute from the live board); `swap_pins` and `swap_gates`; every net
  class, `rules_*` and `project_*` command (no guard on the project's tables).

  A listed row runs as the person's command and logs a new GUI row; it is one
  undo step. The kinds of row, for the commands the list admits:
  - **Operands:** runs through the command's operand handler
    (`CommandArgs::parse_operands`, then `run_operands`), as the CLI does.
    A row that edited a board keeps the targets it wrote, and is refused with
    nothing run once one has changed.
  - **Recorded targets** (Delete, Duplicate): the command's own handler runs
    on exactly those items, in their document, in the window or tab that shows
    it, then the person's selection comes back. Duplicate's copies get new ids
    and the clipboard stays. In project-wide numbering it counts the
    designators of every project sheet, and is refused if one of those sheets
    is not open. A target that is gone is named; Replay never runs on the
    survivors. A tool in progress refuses the row.
  - **Recorded edit** (Move, `edit_object_properties`): the same engine or
    board commands run again on the same items. A move repeats its offset. A
    move started and cancelled says it never finished. An edit that stores a
    whole drawing, directive or harness connector changes only the fields it
    changed when recorded, so a later edit of another field stays; a field it
    changed that now holds a third value refuses the row. A list it changed
    (table rows, say) is merged field by field when what changed since is
    another field; a list that changed length or was edited at the same field
    is refused. A board edit keeps each target it wrote as it stood before and
    after, and runs only while every target is still one of the two; fields
    are not merged, and one changed target refuses the whole row before
    anything runs. A Properties edit of anything else (a symbol flag, an
    editor document) has no such check and is refused.
- **Copy as CLI** copies the row as a `signex-cli run` line (the Usage of
  `docs/CLI.md`): `signex-cli run <project file> <command-id> --arg
  name=value ...`. Each value is the text `--arg` takes (a point as `x,y`),
  and a word the shell would split is single-quoted. A row with no project
  shows `<project.snxprj>` in its place.
- **Jump to** brings the row's document forward when it is open and shows the
  object its arguments name: a part by `designator` or `target`, a net label
  by `net`, or the point `at`. It is greyed when the arguments name none, and
  says in Messages when the object is not on the sheet in view.

## Kept for this session only

The log lives in memory and is gone when Signex closes (decided: session
only). It keeps at most 5000 rows; the oldest goes first.
