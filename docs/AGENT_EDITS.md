# Agent edits (#2178)

A person and an agent work on the same project at the same time. The agent
names its document, uses the engine channel only, and never changes what the
person is doing: the active tab, focus, panels, selection, tool, a drag or a
move in flight, a text edit, the wire being drawn, the command line, the
palette. The person's undo stack is the person's.

Two commands, both with capability `edit` in `signex-cli`:

```
{"id": "apply_edits", "args": {"document": "main.snxsch", "expect_revision": 41,
  "summary": "tie off the spare pin",
  "edits": [{"id": "place_junction", "args": {"at": [5.08, 0]}},
            {"id": "color_net", "args": {"at": [5, 0], "color": "#336699"}}]}}
{"id": "revert", "args": {"document": "main.snxsch", "group": 7}}
{"id": "revert", "args": {"document": "main.snxsch", "group": 8,
  "documents": ["main.snxsch", "power.snxsch"]}}
{"id": "apply_edits", "args": {"document": "main.snxsch", "expect_revision": 41,
  "expect_revisions": {"power.snxsch": 12},
  "edits": [{"id": "place_net_label", "args": {"at": [5.08, 0], "text": "VIN"}},
            {"id": "place_net_label", "args": {"at": [5.08, 0], "text": "VIN",
                                                "document": "power.snxsch"}}]}}
```

## apply_edits

- `edits` is a list of existing catalog commands with their operands, the same
  commands `signex-cli` already runs one by one. No engine type is a wire
  format. At most `MAX_EDITS` (256) per call: the app runs them before it
  handles the person's next key.
- `expect_revision` is the `revision` `read_document` reported. If the sheet
  changed since, nothing is applied and the reason says so; the agent reads
  again and retries.
- Which commands may be edits is derived from the catalog (`edit_refusal`): an
  undoable edit (`mutates_doc` and `undoable`) that runs headless through an
  operand handler. Queries, `undo`, `redo`, `revert`, save, open, close and
  `apply_edits` itself are refused with the reason. `assign_net_class` is
  refused too: it writes the project's net classes, which no sheet's undo step
  covers (a decision for a follow-up, see Known limits).
  `update_pcb_from_schematic` is refused as well: it edits the project's
  board, which no sheet's undo step covers; it runs as its own command. A
  test pins the set.
- An edit acts on the call's `document`, or on the sheet its own `document`
  names (an id or a path). Where a command takes a `document` operand
  (`color_net`, the placements), the call's is filled in when the edit names
  none. Every edit is parsed before any runs, so a typo in the last one
  changes nothing.
- **Several sheets in one call.** An edit may name another sheet of the same
  project as the call's `document`: it must be loaded, and in that project (a
  loose sheet has none, so two loose sheets never share a call). The call is
  then one group on every sheet it touches. `expect_revisions`, a JSON object
  of document to revision (`{"b.snxsch": 12}`), holds the revision each such
  sheet must be at, exactly as `expect_revision` does for the call's own.
  Every touched sheet but the call's own needs an entry, and an entry that
  names the call's own document or a sheet no edit touches is refused, so a
  typo does not pass unnoticed. A call that names one sheet needs none and
  answers as it always did.
- Each edit runs through its own command handler, so it has its own
  validation, read-only refusal and (through the engine) dirty tracking. The
  handlers that read "the active sheet" read the target sheet while the list
  runs (`DocumentState::edit_path`), and the refresh they would do is owed
  until the list ends (`AgentEdits::owed`).
- **One step, all or nothing.** The engine keeps one transaction
  (`Engine::begin_transaction`) for the whole list: commands and batches run
  with history suppressed, as inside `execute_batch`, and the engine writes one
  history entry stamped with an `UndoGroupId` at the end. A refused or failing
  edit rolls the sheet, its history and its revisions back, so the dirty flag
  is what it was.
- **Across sheets** (`agent_edits/group_run.rs`) the app begins a transaction on
  every touched engine, runs the edits with `DocumentState::edit_path` set to
  each edit's own sheet, and commits every engine under one `UndoGroupId`. Any
  refusal rolls every sheet back. Before the first commit each engine is asked
  whether it can still commit (`Engine::transaction_ready`), because an engine
  cannot take a commit back once it landed. A commit that fails anyway after
  others landed does not happen past that check; if it ever did, the landed
  entries would be taken back with `discard_undo_group_top` and the rest rolled
  back, but not exactly: those sheets' redo branch and any history the cap
  trimmed are lost and their revision moves, so it is logged as an error (and
  asserts in a debug build). A call on one sheet skips the check: its own
  commit refuses, with the engine's words, and nothing has landed. A read-only sheet refuses the whole
  call before anything begins. The repaint each sheet owes waits until the
  group has landed. While the list runs, the project read the name checks share
  is refreshed with the current document of every touched sheet, so an edit
  sees what an earlier edit did on another sheet.
- The answer is `{changed, revision, group, label, edits, results}`; `results`
  holds each edit's own answer by index (a wire or bus run answers its report,
  see [CLI.md](CLI.md) "Wire intent"; its `ends` field holds each end's `ref`
  and resolved `at`), `null` for an edit that only ran. A
  wire edit sees the edits before it, and is judged against the sheet as it
  stands, so a route sent as chained `place_wire` edits (A to B, then B to C)
  needs `allow_open=true` on every edit but the last; names are judged across
  the project, not only the sheet (the project is read once per list; a net
  that reaches no other sheet costs no read). `changed: false`
  comes only from an empty list or a list of dry runs: it records no step and keeps the person's
  redo. An edit that changes nothing (a junction already there, a net already
  that colour) is refused by its handler like any other, so it refuses the
  whole list; a refusal is `Disabled`, never a bare `Ran`. A call whose touched
  sheets are not exactly its own `document` (several sheets, or every edit
  naming another one) adds `sheets`, `[{document, path, revision, changed}]`,
  in order of first use; `revision` stays the call's own sheet's, so a step
  that landed elsewhere shows its revision in `sheets`. It is taken back with
  `revert` naming the sheet(s) that changed (their `changed: true` entries),
  not the call's own sheet, which has no step.
- Engine notices an edit raises (`Engine::take_warnings`) are held while the
  transaction is open and shown after the commit; a rollback drops them, so
  Messages never reports a pin or a net of an edit that did not happen.
- After the list: dirtiness is re-derived from history for that path, every
  slot that shows the sheet (the active canvas, undocked windows) is redrawn,
  and selected wires the list removed leave the selection. The person's
  selection is otherwise untouched.

## Undo

- The step is labelled "Agent: <summary>" (the `summary` argument, else the
  edit ids) in an app-side `UndoGroupId -> label` map
  (`AgentEdits::labels`); `HistoryEntry` has no new field. There is no
  per-step label in the Undo menu today, so the label is what Messages says
  when the step is applied, undone, redone or reverted.
- A person's Ctrl+Z on that sheet undoes the whole step (a one-sheet group, as
  a project-wide run is a many-sheet one), and Ctrl+Y redoes it.
- A step that spans sheets is recorded as an `UndoGroupKind::AgentEdit` group
  (`DocumentState::undo_groups`): Ctrl+Z or Ctrl+Y on any of its sheets moves
  every open sheet of the group, by the rule a project-wide annotate follows
  (`apply_grouped_step`). If a sheet has a later step on top of the group, the
  whole undo is refused with a Messages line naming it, and nothing moves: the
  person undoes the newer edit first. A sheet closed since, or whose history no
  longer holds the group, is skipped with a warning.
- The agent has no `undo` or `redo` inside `apply_edits`. `revert group=N`
  takes back step N only while it is the newest entry of that document
  (`Engine::discard_undo_group_top`); for a step that spans sheets, the call
  must name every sheet of the group (`document` and the list `documents`) and
  none other, and the step comes back on all of them, or on none: every sheet
  is judged first, and a sheet that is closed, reopened or trimmed past the
  group refuses the revert. A revert that names too few or too many sheets is
  refused with only the count of each, never the names, since a caller must not
  learn of a sheet it did not name (`signex-cli` confines each named sheet like
  a `document` operand, and drops from the answer's `sheets` any entry it did
  not name). It is refused, and nothing changes, when
  - a later step sits on top. A net-class pick is a `HistoryStep::Pick`
    marker with no group, so it counts as a later step;
  - the person undid a step and can redo it: that redo would restore a
    snapshot that still holds the agent's edits;
  - the person is in the middle of something on the sheet, in the main window
    or an undocked one (`Signex::person_busy_on`): drawing (any slot showing
    the sheet has a drawing mode, wire preview, arc, polyline or shape
    anchor), typing into an item of the sheet, or a move armed (`move_pick`
    on any slot showing the sheet: a move armed from an undocked window is
    armed on that window's slot, the app-wide flag is not read);
  - the group is not one an agent made in this session.
  A reverted step does not land on the redo stack.
- The headless `undo document=..` of #2172 still exists; on an agent's step it
  now says to use `revert`.

## Origin (#2253)

The app knows, at dispatch, who a command comes from: `Origin::Person` (a
click, key, menu or the palette) or `Origin::Agent { pid, client }`. It is
app-side only: the engine's `HistoryEntry` has no field for it (#2178,
ADR-0001 D4).

- It is held for the length of one command, as `agent.serving` and
  `agent_edits.target` are, by `Signex::with_origin`, which puts the previous
  value back when the closure returns. No handler takes it as an argument.
- The socket sets it for every request it reads, a refused one included, with
  the pid of the client. `client` is the client's own name, when its request
  carries one (`signex-cli` sends its own); otherwise it stays empty rather
  than invented.
- A headless run (`dispatch_command_headless`) sets it to an agent whose pid is
  the process's own.
- Messages: the "Agent request (pid N): ..." line is as before. An agent step
  keeps its origin next to its label (`AgentEdits::origins`), so the person's
  Ctrl+Z and Ctrl+Y say "Undone: Agent: color_net on main.snxsch (agent, pid
  N)".

Every command the agent runs on the socket that changes a document and lands
on the undo stack (`mutates_doc` and `undoable`: `color_net`,
`clear_all_net_colors`) is one engine transaction on the named sheet, stamped with a new undo group
and labelled "Agent: <command id> on <file>", the same way `apply_edits` makes
its step. The reply is unchanged (`ran`); the Messages line "Agent: color_net
on b.snxsch (undo group 7)" tells the agent the group for `revert`, which
works on these steps by the rules above. A command that makes no undo step
(`save_document`, `close_active_document`), or is refused, gets no group.

## Journal

Every agent command that changes a document (or writes or closes one), and its outcome, is also a line
of the project's `.signex/agent-edits.jsonl`, with the revision before and
after and the undo group named here, so it can be replayed at the same
revision. A call across sheets is one line per sheet that got a step, all with
the same group (a call whose edits all name another sheet has that sheet's line
only). See `docs/AGENT_JOURNALS.md`.

## signex-cli

`apply_edits` needs `edit`; each edit is authorized again as a command of its
own (`gui_only`, capability, every path operand confined, every `document`
resolved and checked, as for a top-level command), then written back with the
checked paths. The documents named as keys of `expect_revisions` are resolved
and confined like a `document` operand. `--save` still gates `save_document`. The result line carries
the answer in `data` and the sheet in `changed`.

## Known limits

- A call may span the sheets of one project, never two projects. Cross-sheet
  groups are agent steps only: a person's own undo of a sheet is blocked while
  a later step sits above the group on another of its sheets (by design, as
  for annotate).
- `assign_net_class`, annotation and other project-wide data are not edits;
  that needs a decision on how project data joins a sheet's undo step. For the
  same reason an agent's `assign_net_class` on the socket stays a net-class
  pick (`HistoryStep::Pick`, no group): it gets no label and `revert` cannot
  name it.
- A headless run has an origin but its commands are not grouped: it has no
  person, and its own `undo` refuses a grouped agent step. Grouping them waits
  for the journals (#2253).
- The origin's `client` is always empty until the wire carries a client name.
- A click-click move that is already floating is widget-local state the app
  cannot see, so `revert` cannot refuse for it. A revert does not edit the
  selection for items other than wires, so a float over reverted agent items
  survives and, on its drop, finds no such uuids: it cancels or does nothing
  for items that no longer exist. The person's own items and work are not lost,
  as with their own Ctrl+Z during a float.
- What the edits' handlers say in Messages (Info lines, for instance "Cleared
  net-color overrides on ...") is held while the list runs and shown after the
  commit; a refused list shows none of it. Only this thread's Info lines are
  held (`diagnostics::hold_info`); warnings and errors are never.
- A person's redo is ended by an agent edit, as by any edit.
- The grid check on a placement point reads the person's snap settings.
