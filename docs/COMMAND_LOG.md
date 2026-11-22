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
- **Not logged:** pointer gestures that are not catalog commands (a drag, a
  click on the canvas), panel interactions, and the panel's own actions.

Each row keeps the time with milliseconds, the source, the catalog id and its
label in the UI language, the operands as given (name to value), the outcome,
how long the synchronous part of the dispatch took, the document it acted on
(its `document` operand, else the window's document) and that document's
project file. A GUI command has no operands; one that acts on the selection
records its targets (kind and stable id) as the `recorded_targets` argument,
shown in the detail pane and the saved log and left out of the CLI line.

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
the undo stack. A step made without a command (a drag) is not in the log, so
the row marked is the newest logged undoable command of that document.

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

## Actions

- **Pause logging** stops recording until Resume logging; **Clear** empties
  the log (numbering goes on).
- **Save log as…** asks for a file through the file port (`io_ports`) and
  writes the whole log there as JSON Lines, one object per row (`seq`,
  `time`, `source`, `client`, `pid`, `window`, `id`, `args`, `result`, `text`,
  `took_ms`, `document`, `undoable`, `undone_at`), off the update thread.
  Nothing is written to disk unless asked.
- **Replay** runs the row's command again on what it acted on, as the person's
  command, and logs it as a new GUI row; it is one undo step. A failed row
  fails the same way until its cause is fixed. By the kind of row:
  - **Operands:** runs through the command's operand handler
    (`CommandArgs::parse_operands`, then `run_operands`), as the CLI does.
  - **Recorded targets** (delete, cut, copy, rotate, mirror, flip, drawing
    order): runs on exactly those items, in their document, in the window or
    tab that shows it, then puts the person's selection back. A sheet behind
    the one in view is brought forward for the run and left again, which ends
    a placement in progress on the sheet in view.
  - **Refused, nothing changes** (a new failed row and a Messages line): a
    target is gone (the reason names it; Replay never runs on the survivors),
    its document is not open, the row recorded no targets (a command run in a
    footprint or symbol editor), or the command keeps working on the selection
    after it starts (Move, Duplicate, Properties, a dialog on the selection).
  - **No target** (place wire, a view command): runs again as it is, with the
    current selection held aside. Select All and Unselect All run as they are:
    the selection is what they change.
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
