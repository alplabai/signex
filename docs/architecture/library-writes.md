# Library writes off the UI thread (#893)

`update` never takes a library's write lock or runs its git work. Every
library mutation snapshots what it needs, runs the adapter call on the
blocking pool and comes back as `LibraryMessage::WriteFinished`
(`signex-app` `library/writes/`).

## Order: one queue per library

Each write, and each refresh read that follows one, takes a numbered
`Turn` on its library's `WriteGate` while `update` still has the message.
The worker waits for its number. Two quick edits of one library therefore
land in the order they were made, and a read sees the writes before it.
Rejecting a second edit while one runs would drop it and make the user
retype it, so edits queue. The owed work is Model state
(`LibraryState.writes`, ADR-0003 D1). A turn is released on `Drop`: a
dropped task or a panicking worker cannot stall the writes behind it (D5).

Every row write of this app goes through `row_edit_task` (cell edit, modal
Save, primitive binds, Component Preview Save including Save All and
close-with-save), and the primitive container saves of a mounted library
take a turn too. Nothing blocks `update` to wait for them.

## Rebase on this app's own writes

The gate keeps, per row, the revision this app's latest write produced
(`OwnRevisions`). A queued cell edit or browser bind was built from the row
the user saw; if an earlier write of this app moved the row on, the edit's
patch is applied on top of the stored row instead of being refused. A row
that changed for any other reason (another process or user) does not match
the ledger, so the adapter's `RowChanged` check still refuses the edit and
the typed text stays for a retry. Whole-row writes (modal Save, Component
Preview Save and bind) are never rebased.

The worker returns the row it actually wrote and the completion caches that
whole row with its revision, so a cached copy and its revision always
describe the same stored row.

## Boundaries (ADR-0003 D2)

- Quit waits (bounded) for every library's queue, looking at the libraries
  when the wait starts.
- Closing a library with writes pending is deferred until they finish; it
  never unmounts under a running write.
- Rename library is refused while the library has writes pending, and the
  blocking primitive save is refused the same way, instead of waiting on a
  worker from the UI thread.
