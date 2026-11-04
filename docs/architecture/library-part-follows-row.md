# A placed part follows its library row: limits of the version guard (#1867, #1939)

A placed symbol is stamped with the row's `version` when it takes the row's
footprint, Manufacturer and MPN. Editing one of those three on a row in the
app moves the row's `version` (`library/writes/part_version.rs`). "Update from
library" and the open-time scan compare the part's fields with the row; the
stamp tells a row that moved from a part the user overrode.

The guard reads the stamp, nothing more, so it has these limits. None of them
loses data: each one shows or hides an offer, and nothing is rewritten without
the user's click.

## Limits

- **Edits made outside the app.** A git pull or another tool can change the
  footprint, Manufacturer or MPN without moving `version`. Part and stamp then
  still agree on the version, so the difference reads as the user's override
  and is not flagged. Move the row's `version` by hand when editing it
  outside the app.
- **An override is lost when the row's version moves for another reason.** A
  symbol-save cascade, for one, moves the version. The guard cannot tell why,
  so it reads "the row changed since this part took its fields", and Update /
  Apply then offers the row's footprint, Manufacturer and MPN over a
  deliberate per-part value.
- **A cascade that cannot finish.** The rows a save could not move (a
  failed write, a library still busy) stay owed per file in memory
  (`library/state/owed_cascades.rs`). The next save of that primitive, or the
  editor reopened after a close, carries them out, whether or not the editor
  that saved is still open. The record is not written to disk: a restart
  drops it, because keeping it would need a new on-disk record. Rows left
  behind that way are still at the old version until the file is saved again.
- **A cleared field is filled again.** An empty field is not an override: if
  the user blanks Manufacturer, MPN or the footprint on one part, Update /
  Apply fills it from the row. A value the row leaves empty never clears the
  part's own.

## Cost

The open-time scan checks each stale instance against its row. The stamps are
built once per `lib_id`, each library opens once and each row's footprint name
is read once (`library/drift/check.rs`). The reads still run synchronously in
`update` (#665).

## Row edits

A row edit reads the stored row once. A row that is not stored yet is a plain
first write; any other read failure refuses the edit, because writing without
the read would skip the version bump with no word of it
(`library/writes/mod.rs`).

## Library deletes

A Component Preview edited while its library's delete runs is reported in
Messages ("unsaved edits ... were discarded"), not kept. When the file was
already gone, the line says so ("already gone from disk") instead of claiming a
delete, because the unmount drops the editor either way. Symbol and footprint editor edits in that library are refused while
`library_rename_running()` (#1939). This is a product decision made by the
maintainer on 2026-11-04 in the #1939 thread; before it, a rename kept dirty
drafts by moving them to the new path (`rekey_library_documents`) and only a
delete lost them. One check, `editor_edit_refusal`, serves the editor message
stream, undo/redo and the Properties panel. The Component Preview
(`LibraryMessage::EditorEvent`) is still not guarded (#1939 item 3).
