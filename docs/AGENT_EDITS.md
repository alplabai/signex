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
  covers (a decision for a follow-up, see Known limits). A test pins the set.
- Every edit acts on the call's `document`; it never names another. Where a
  command takes a `document` operand (`color_net`), it is filled in; an edit
  that carries one is refused. Every edit is parsed before any runs, so a typo
  in the last one changes nothing.
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
- The answer is `{changed, revision, group, label, edits}`. `changed: false`
  (an empty list, or edits that changed nothing) records no step and keeps the
  person's redo; a refusal is `Disabled`, never a bare `Ran`.
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
- The agent has no `undo` or `redo` inside `apply_edits`. `revert group=N`
  takes back step N only while it is the newest entry of that document
  (`Engine::discard_undo_group_top`): it is refused, and nothing changes, when
  - a later step sits on top. A net-class pick is a `HistoryStep::Pick`
    marker with no group, so it counts as a later step;
  - the person undid a step and can redo it: that redo would restore a
    snapshot that still holds the agent's edits;
  - the person is drawing on the sheet;
  - the group is not one an agent made in this session.
  A reverted step does not land on the redo stack.
- The headless `undo document=..` of #2172 still exists; on an agent's step it
  now says to use `revert`.

## signex-cli

`apply_edits` needs `edit`; each edit is authorized again as a command of its
own (`gui_only`, capability, every path operand confined, every `document`
resolved and checked, as for a top-level command), then written back with the
checked paths. `--save` still gates `save_document`. The result line carries
the answer in `data` and the sheet in `changed`.

## Known limits

- A call acts on one sheet. Edits across sheets under one group id (as project
  replace does) are not offered; a placement has no `document` operand yet.
- `assign_net_class`, annotation and other project-wide data are not edits;
  that needs a decision on how project data joins a sheet's undo step.
- A person's redo is ended by an agent edit, as by any edit.
- The grid check on a placement point reads the person's snap settings.
