# Document dirty state

One question, one owner: "does this document hold edits that are not on
disk?" is answered by `DocumentState::is_document_dirty(path)`
(`crates/signex-app/src/app/state/dirty.rs`). Epic #661, ADR-0003 gap G8.

## What is stored

| Document | Stored flag | Why it lives there |
|---|---|---|
| Symbol or footprint draft | the editor's own `dirty` (private) | Roughly a hundred editor update sites flip it with `&mut editor` and no workspace state. |
| Schematic sheet, PCB, `.snxprj`, `library.toml` | a path in `DirtyDocuments` (private) | The mark must outlive the tab: a parked engine stays dirty after its tab closed. |
| Component editor row | `LibraryState.editors[..].dirty`, derived from its `RowHistory` | Keyed by row, not file; the tab dot reads it through `tab_bar::dirty_tab_paths`. |

Everything else is derived. `TabInfo` has no dirty flag: the tab dot, the
title-bar "N unsaved", the quit and close-project gates, Save All, the
History panel and the project tree all read the one query (or
`dirty_document_paths()`, the union of the stored set and the dirty drafts).

## The writers

Only `DocumentState` writes the set; the editor flag is private to
`documents.rs`, with `mark_dirty` for the editor's own edits.

- `mark_document_dirty(path)`: an edit. A draft open at `path` takes the
  flag; any other document goes into the set.
- `mark_document_clean(path)`: a save landed or the edits were discarded.
  Clears the set entry and the draft flag together.
- `mark_documents_clean_where(pred)`: the same for every dirty document a
  predicate matches (one file under several case spellings, a deleted
  folder).
- `rekey_document_dirty(from, to)`: Save As and rename move the stored
  mark. A draft's flag travels with its editor, which the caller re-keys.
- `keep_document_dirty_until_saved(path)`: a save snapshot holds the mark
  until its write lands; see below.
- `release_save_hold(path)`: the write landed for an older snapshot while
  the draft has newer edits; the editor flag carries the dirt, so only the
  hold is released.
- `FootprintDocument::give_back_rejected_edit`: the rejected-edit paths
  (`updates/attempt.rs`, `updates/sketch/solved.rs`) give the flag and the
  save point back as they were before the edit. `set_dirty` is the plain
  flag setter for a save or a discard.
  The flag is the draft's only stored state once the hold is released, so
  that really makes the draft clean. While a save is still in flight the
  hold keeps it dirty until the write lands.

## Undo and the save point (#1822)

A schematic sheet's engine owns a save point: the history position
(`Engine::undo_position`) it held when its content last equalled its file.
It is **unknown by default** (`Engine::new` reads as dirty), so an engine
built from content that is not known to be on disk (an autosave recovery
copy, an in-memory rewrite) can never look clean. It is set only where the
content is known to equal the file: `Engine::open`, `open_schematic_tab`
(a sheet read from disk), a History restore, and a landed save whose
revision still matches (`Engine::mark_saved`, called from
`mark_document_clean` and `finish_save_as`). After every history step
(edit, undo, redo; `sync_history_dirty`) the sheet is dirty unless
`Engine::is_at_save_point()` holds, so undoing the only edit since the save
is clean again and Redo is dirty again.

While a write is in flight the sheet reads dirty whatever the history says
(`sync_history_dirty` checks `document_saves.is_saving`), so an Undo during
the write cannot hide the snapshot that is about to land; the completion
settles the mark (clean on a matching revision, otherwise re-synced). A
History restore that rewrites the file without the result being applied to
the surviving engine forgets its save point and marks the sheet dirty.

An autosave recovery copy reads dirty because its engine has no save point,
not because of an outside mark.

Known limit: a change to the file made outside Signex (a git checkout,
another editor) is not detected by the save point; the engine's point still
names the content it last saved or loaded.

The save point is dropped, and the sheet stays dirty until the next save,
when no history step can reproduce the file any more:

- an edit after undoing below the point (it truncates the redo branch that
  held the point);
- a coalesced parameter-cell run (#1785) amending the entry the file was
  saved at;
- an unrecorded edit (`execute_unrecorded`, `set_document`) or a discarded
  group step;
- a save whose snapshot the document had already moved past (an edit, or
  an Undo back to an older state, while the write ran): `mark_saved` and
  `finish_save_as` see a different revision, forget the point and leave the
  sheet dirty, even if the Undo had cleared it meanwhile.

Who sets the save point (#1854):

- `mark_document_clean` is for a landed save: it clears the marks and calls
  `Engine::mark_saved`. Where the engine at the path is gone or is not known
  to equal the file, `clear_document_dirty` clears the marks and leaves the
  save point alone: Discard All, an undo that drops a parked engine, a
  rolled back re-prefix, and the Save As old key and the unopened-sheet
  Annotate write (no engine at that key). A re-prefix rollback whose
  `discard_undo_group_top` fails or finds the run is not on top leaves that
  sheet dirty and says so in Messages.
- A sheet read from disk or restored is marked saved only when the engine
  was just installed (`install_active_engine` / `install_schematic_at`
  return whether it was); a failed install cannot mark the older engine.
- History-recorded edits outside the mark paths go through
  `mark_history_edit_dirty`: the non-active sheets of a project-wide Replace
  and a recorded library update on a parked sheet, so undo back to the save
  point clears them. Unrecorded updates still use the outside mark.

Known limit: Undo then Redo while a write is in flight forgets the save
point. The write's completion sees a different revision and calls
`forget_save_point()` (`save_async.rs`), so the sheet stays dirty until the
next save. It errs safe: the
sheet reads dirty when it could read clean; nothing is lost.

`DirtyDocuments` also remembers which marked paths were marked from outside
the history (`mark_document_dirty` and its display-only twin: a project
edit, a net colour or class, an annotate pass, a restore). The history can
only clear a mark it set itself, so undo never hides such a change; only a
save clears it. This stays inside the one dirty owner: one query, one
writer set. When #661 moves the flag into the engine, the save point and
this split move with it.

## Draft save point (#1854)

A symbol or footprint draft keeps its snapshot undo, and records a save
point in it (`app/history_save_point.rs`): the undo depth at which its content
last equalled its file. Depth counts from the oldest step ever recorded, so a
full stack dropping its oldest step keeps every depth meaning the same state.
A draft is built at depth 0 and clean, except a footprint whose load was
repaired, which has no save point. After Undo or Redo the flag is "away from
the save point, or rows still owed", so undoing back to the file reads clean
and redoing reads dirty; `restore` no longer marks the draft dirty itself.

The save point is set where a save snapshot is taken (`mark_saved`, the same
place that resets the flag), not when the write lands. The in-flight rule is
unchanged: the path rides in the set until the completion, so an Undo to the
snapshot during the write still reads dirty, and the completion clears it
only if the flag is clean (the draft is at the snapshot). A failed write
forgets the save point (the snapshot's depth named content that never reached
disk), so the draft stays dirty through any Undo or Redo until the next save.

The save point is forgotten, and the draft stays dirty until the next save,
when no step can reproduce the file:

- `mark_dirty` at the saved depth: an edit with no step of its own, or one
  coalesced into the newest step (the keystroke run); a recorded edit gets
  its snapshot's point back in `commit_history`, so its own `mark_dirty`
  cannot cost it;
- an edit after undoing below the point (it clears the redo branch);
- the saved step evicted at depth 100, or a capped redo stack losing it;
- a footprint drag batch collapsing after it pushed steps off a full history;
- a failed write (`fail_primitive_save`), and a landed write that applied
  version bumps while the draft was edited since (older steps hold the
  pre-bump version);
- anything built from content not known to be on disk: an autosave recovery
  copy (`mark_dirty` after construction) and a repaired load.

A rejected edit (`attempt.rs`, a rejected drag batch) gives the save point back
together with the flag.

Known limit: an accepted footprint drag whose ticks evicted steps from a full
history forgets the point; a rejected one forgets it only when it pushed its
own first step off the history.

Undo and Redo own the flag: the symbol canvas's Ctrl+Z / Ctrl+Y
(`SymbolEditorMsg::Undo`) must not call `mark_dirty` afterwards.

## Component editor row save point (#1854)

A Component editor keeps whole-row snapshots (`library/state/preview_history.rs`,
depth 100) and the same `SavePoint` as a draft. Ctrl+Z / Ctrl+Y in the tab step
it (`handlers/component_history.rs`); versions, pins and the base revision stay
the live row's, only the edits move, and the typed-but-uncommitted buffers are
dropped. An inline edit that changed the row is one step
(`begin_edit` / `finish_edit` around `apply_inline_edit` and the save-time buffer
flush). The flag after Undo, Redo or a landed save is "away from the saved
position", and a half-typed value in `params_edit_buf` still counts as unsaved
(`has_unsaved_edits`) whatever the history says.

- The point is set where the save snapshot is taken (`begin_save`, beside
  `saving_row`). A row with a write in flight stays dirty whatever Undo does;
  the landing re-derives the flag, and a landed copy that differs from the row
  on screen (`edited_since_save`) is dirty and forgets the point.
- A failed or abandoned write (`abort_save`) forgets the point.
- Forgotten, so the row stays dirty until the next save: an edit after undoing
  below the point, the saved step evicted at depth 100, and an edit that marks
  the preview dirty without changing the row (the simulation model, which the
  snapshots do not hold).
- A re-bound primitive (an edit that changes a primitive reference, and the
  bind from the picker) drops every step: a snapshot cannot hold the resolved
  symbol, footprint or model. The row keeps whatever dirtiness it had.
- An alternate symbol added from the picker is one step like an inline edit.
  A bind whose write fails leaves the row dirty and forgets the point. A write
  by the browser or a cascade to a row whose editor is dirty forgets the point,
  since it names disk content the library no longer holds.
- Staged PDFs are kept while any undo or redo step pins them, so an undo to a
  replaced pick still has bytes to write.
- A reload replaces the editor state, so its history is new and the row clean;
  Close and Discard remove the state with its history. There is no autosave
  recovery for a row, so no recovered copy starts clean.

The title-bar count still leaves a Component row out (see "Readers that differ").

## Project file save point (#1854)

The `.snxprj` has three undo histories (the Project tab's General and
Parameters fields, the stored ERC rules, the net-class table), each stepped
separately, so no single stack position names the saved state. The one cursor
is the bytes the file holds (`state/dirty/project_history.rs`): the project as
`encode_project` writes it at the last save, or at the moment before its first
history edit while it was clean. After any history step (edit, undo, redo) the
project is clean exactly when it encodes to those bytes, nothing outside the
histories marked it, and no write is in flight. That needs no hook in the
histories, so eviction at depth 100, a coalesced typing run and a cut redo
branch cannot leave a stale position behind.

- `note_project_clean_state` runs before a history edit; `sync_project_history_dirty`
  after any history step. The three paths (`project_tab/update.rs`,
  `handlers/erc/rules_edit.rs`, the net-class panel and `pick_undo.rs`) use them
  instead of `mark_document_dirty`.
- A save that lands clean records the cursor (`mark_document_clean`). A write that
  landed while the project was edited records the bytes it wrote
  (`note_project_bytes_on_disk`), so undo back to them is clean; when a newer
  write is still to land the cursor is unknown until it does.
- Forgotten, so the project stays dirty until the next save: a failed write
  (`forget_project_saved_state`), a discard (`clear_document_dirty`), a rename or
  Save As (`rekey_document_dirty`), and a project that was already dirty when its
  first history edit came in.
- The roughly twenty edits outside the histories (a library added, removed or
  renamed, a sheet list change, a sheet reference rewrite, a Save As repoint)
  keep `mark_document_dirty`: they sit in `external`, so undo never hides them and
  only a save clears them.

## Why a draft can also sit in the set

A save snapshot resets the editor flag ("edited since this snapshot"), but
the write has not landed. `snapshot_primitive` keeps the path in the set
until `mark_document_clean` runs on completion, so a quit, a tab close or a
failed write still sees the draft. A completion for a stale snapshot checks
the flag first: a newer edit leaves it set and the document stays dirty.

## Re-keying and recorded failures

`rekey_open_document` (rename, Save As, library rename) moves a recorded
Save Failed reason to the new path with the document, so its card is not
lost. A queued follow-up save is dropped: it names the old file.

## Readers that differ

The title-bar count (`dirty_document_count`) counts documents only. A dirty
Component Preview row is not a document path, so the title leaves it out,
while the quit gate, Close Project and the tab dot do count it.

## Path keys

The set is keyed by the exact spelling a document was opened under. Case
folding (#1540, #1149) stays with the callers, which decide whether two
spellings are one file and then pass each spelling to the helpers.

## What this replaced

Three mirrors (editor flag, `dirty_paths`, `TabInfo.dirty`) written from
about 60 call sites, reconciled one way by `sync_editor_dirty_paths`, with
a `clean_mirror_owed` flag for the rejected-edit case (#1040, #1047, #585).
With one stored source per document there is nothing to reconcile.
