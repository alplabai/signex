# Hosting board edits: one engine per open `.snxpcb`

Addendum to ADR-0005 (board geometry) and epic #661 slice 7. Nothing in the
editor edits a board yet; this fixes where edits will enter, so the first
editing UI does not have to decide it.

## Shape

- A PCB tab holds a `signex_engine::pcb::BoardEngine` (`TabDocument::Pcb`),
  not a bare `PcbBoard`. A dirty tab that closes parks the engine, history
  included (`state/pcb_board.rs`, #2149, #2196); reopening takes it back.
  A parked engine is read-only: no edit, undo or redo reaches it
  (`BoardEditError::Parked`, `undo document=` refuses), because Save All
  may be writing it, and a step during that write would let the landing
  save clean a board that moved after the snapshot.
- `BoardEngine::execute_batch(Vec<BoardCommand>)` is the only mutation path.
  `execute` is one command; both are one undo step, all or nothing. In the app
  the door is `Signex::execute_board_commands` (`handlers/pcb_edit.rs`), which
  runs `DocumentState::execute_board` and repaints. No handler writes a
  `PcbBoard`: the one remaining direct write is a document-id repair, which
  is `BoardEngine::reassign_document_id` (not an edit, as for a sheet).
- The history is `History<Vec<BoardCommand>>` on `signex-history`. A step
  holds the inverse commands, last first, not a board: a 10k-track board
  makes per-step snapshots too expensive. Undoing a step runs it and stores
  the commands that redo it. The depth, the cap and the save point are
  `History`'s, so undo back to the saved board reads clean (#1852, #1913) and
  `undo_depth` reaches boards through `sync_undo_depth`.
- The revision rises on every execute, undo and redo. A new engine starts
  above every revision its path reached before (`pcb_revisions` outlives the
  tab), so a stale save completion can never match a reopened board. The
  ratsnest cache `(path, revision)` and the `BoardIndex` key on it.
- Dirty, save and close go through the single owner (`docs/DIRTY_STATE.md`):
  `sync_history_dirty` reads the board engine as it reads a sheet engine,
  `mark_document_clean` marks the save point, and a save that lands for a
  snapshot the board has moved past forgets the point and re-syncs the mark.
  That revision check runs whether or not the tab is still open, and
  `forget_board_save_point` reaches a parked engine (a stale completion, a
  History restore that wrote the file under it); only edits are refused
  there. `mark_board_saved` reaches the open board only: a landed save
  drops a parked one.
- A failed rollback (`RollbackFailed`, unreachable by construction) takes a
  new revision and forgets the save point, so caches rebuild and the board
  reads unsaved.
- After any board change every window that shows the board repaints: the
  main canvas when it is the active tab, and each undocked window showing
  that path, with its ratsnest (`PcbRatsnest::snapshot_of`). Repainting does
  not cancel the active tool.
- Undo and Redo reach a board through `UndoTarget::Document(Pcb)`: the same
  `undo` / `redo` command ids, menu enablement and keys as every document:
  Edit menu rows, the toolbar and Active Bar buttons, Ctrl+Z / Ctrl+Y, the
  palette and `undo document=<board>` all run that command; each is greyed
  by the board engine's history (`undo_availability`). In an undocked board
  window they step that window's board, whatever the main window shows.

## Agents

`execute_labelled(commands, label, expected_revision)` is the agent path of
#2178 for a board: one labelled undo step, refused with
`BoardCommandError::StaleRevision` and nothing applied when the board moved.
`apply_edits` itself is not wired to boards: it runs catalog `AppCommandId`s
through operand handlers, and no board command has an id yet. When the first
board editing command gets one, its handler calls `execute_board_commands`.
`read_document` already reports the board's revision.

## Forward annotation (#1312)

Update PCB from schematic runs its change list as one `execute_labelled` step
(through `CheckedStep`, the labelled and revision-checked form of
`execute_board_commands`), so it is all or nothing, one undo step, and
refused when the board moved since the list was built. The commands it uses,
each with an exact inverse:

- `AddItems` appends items to the end of their lists; refused with
  `ItemAlreadyPresent`, `DuplicateItem`, `NonFinite` (a position or angle,
  a footprint's pads' included, that is not a number) or `PadAlreadyPresent`
  (a new pad uuid already used by the board, a footprint, pad, track, via,
  zone or text, or by another new item: they share one id space, as in the
  load repair) or `UuidAlreadyUsed` (a new item whose own uuid the board, a
  pad or an item of another kind uses). Inverse: `DeleteItems`.
- `ReplaceFootprint { footprint, with }` swaps in a library footprint given
  in its top-side form. The board footprint keeps its uuid, position,
  rotation, side, lock, schematic link, designator, value and preserved
  unknown data; it takes the template's name, pads, graphics and
  properties. The n-th new pad of a number takes the net and net lock of
  the n-th old pad of that number (a number the old footprint lacks gets no
  net), so a lock on one of several `EP` pads survives; when the new
  footprint has fewer pads of a number and an old pad past the new count
  holds a net or a lock, the replace is refused (`PadCountMismatch`). On the bottom side the
  template is flipped with `pcb_geom::flip_footprint` first. Refused also for
  a template not on a top-side layer this build reads (`TemplateNotTopSide`,
  an empty or unknown layer included), with a pad that is not a number
  (`NonFinite`), or with a pad uuid used on the board (`PadAlreadyPresent`). Inverse: `RestoreFootprint`, the whole old
  footprint.
- `SetFootprintText` sets the designator and value and the Reference and
  Value property texts. Inverse: `RestoreFootprintText`, which puts back
  every property text as it was.
- `SetFootprintLink` sets or clears the `SymbolLink`. Its inverse is itself.
- `SetNets(BoardNetEdit)` wraps `signex_net::board::BoardNetEdit::apply`;
  refused whole (`BoardNetEdit::check`) when a change names a footprint,
  pad or zone the board does not hold (`FootprintNotFound`, `PadNotFound`,
  `ItemNotFound`) or names one twice, so a stale list is never a silent
  no-op. The inverse is the edit `apply` returns, holding only what was
  written, so undo is never refused.

`BoardEngine::check_batch(commands)` runs the lease check and then the list
on a copy of the board, for the dialog's Validate: `BatchRefusal { index, error }` names the first command
that would be refused. The library identity of a footprint (`library_ref`)
is not carried yet.

## Collab leases

`BoardEngine::check_lease` runs before every execute, undo and redo and
allows everything today. v3.4 (COLLABORATION_PLAN) puts the advisory lease
check for `RouteTrace`, `ModifyZone` and the like there, so the API does not
change.
