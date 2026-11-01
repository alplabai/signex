# Document dirty state

One question, one owner: "does this document hold edits that are not on
disk?" is answered by `DocumentState::is_document_dirty(path)`
(`crates/signex-app/src/app/state/dirty.rs`). Epic #661, ADR-0003 gap G8.

## What is stored

| Document | Stored flag | Why it lives there |
|---|---|---|
| Symbol or footprint draft | the editor's own `dirty` (private) | Roughly a hundred editor update sites flip it with `&mut editor` and no workspace state. |
| Schematic sheet, PCB, `.snxprj`, `library.toml` | a path in `DirtyDocuments` (private) | The mark must outlive the tab: a parked engine stays dirty after its tab closed. |
| Component editor row | `LibraryState.editors[..].dirty` | Keyed by row, not file; the tab dot reads it through `tab_bar::dirty_tab_paths`. |

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
- `FootprintDocument::set_dirty` / `SymbolDocument::set_dirty` (public, on
  the editor): the rejected-edit paths (`updates/attempt.rs`,
  `updates/sketch/solved.rs`) give the flag back as it was before the edit.
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

`DirtyDocuments` also remembers which marked paths were marked from outside
the history (`mark_document_dirty` and its display-only twin: a project
edit, a net colour or class, an annotate pass, a restore). The history can
only clear a mark it set itself, so undo never hides such a change; only a
save clears it. This stays inside the one dirty owner: one query, one
writer set. When #661 moves the flag into the engine, the save point and
this split move with it.

Symbol and footprint drafts keep their editor-local snapshot undo and do not
use the save point yet.

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
