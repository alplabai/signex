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

One exception: the Refresh All Pricing walk (`signex-library`
`pricing_refresh.rs`) stores each row's prices through `store_pricing`
on its own worker, without a turn. It is not a silent overwrite:
`store_pricing` checks each update against the row as stored, under the
library write lock and after a reload, writes only the cached prices, and
drops (and reports) an update whose row no longer matches. Closing the
library cancels the walk (`cancel_pricing_refresh_of`).

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
  never unmounts under a running write. See "Closing a library" below.
- Rename library is refused while the library has writes pending, and the
  blocking primitive save is refused the same way, instead of waiting on a
  worker from the UI thread.

## Closing a library (#1728)

Close Library asks about dirty Component Preview editors first. With none,
a library that still has writes queued records the close in
`LibraryState.closing` and says so in Messages; the library stays usable
meanwhile.

- **When it comes due.** `close_libraries_whose_writes_are_done` runs
  wherever a turn is released: a row write, a refresh, a container save, a
  git re-init. It also runs on the external-edit poll tick, which catches a
  turn released by a task the runtime dropped (no message arrives for
  that).
- **The check runs again.** Editors can be opened or edited while the
  close waits. When the close comes due, a dirty editor cancels it and
  raises the same Save All / Discard All / Cancel prompt; nothing dirty is
  dropped silently. Clean editors close with the library. Discard All
  drops the library's editors at once, so a close it defers does not ask
  again.
- **Opening again cancels it.** Opening the library (Open Library, a
  Library Browser tab, any `commands::open_library` caller) while its close
  waits clears the close.
- Each step (deferred, closed, stopped by unsaved editors, cancelled by a
  re-open) is one Messages line naming the library.

## Library repositories and the commit ledger (#1729)

A versioned library's repository carries the managed `.gitignore` block
projects get (`signex_types::project_vcs_files::GITIGNORE_BLOCK_BODY`:
`*.tmp`, `.*.signex-writelock`, `.DS_Store`, `Thumbs.db`, `desktop.ini`),
so OS and temp clutter never reaches the commit ledger or a commit. The
rule is the `.gitignore` the user can see and edit, not a filter by file
extension.

- **New library with version control.** The block is written before
  `git init` and staged in the first commit, beside `.gitattributes`.
- **Enable Version Control.** Library scope stages the block too; project
  scope also gets the managed `.gitattributes` block.
- **Older library repositories.** Opening a library writes nothing: an
  adapter is also opened for reads and for read-only mounts, and a write
  there would leave an uncommitted change in the user's repository. The
  ledger seeding applies the block's rules in memory
  (`git2::Repository::add_ignore_rule`, not persisted), so clutter is not
  listed. The library's next commit (always a writable adapter: a
  read-only mount refuses every write) merges the block into `.gitignore`,
  keeping the user's lines, and records it in that same commit. A file the
  merge refuses (broken markers, a link, read-only) is left alone and
  warned about once.

The sweep that adds every ledger file to the next commit never lets one
bad file stall the library. An ignored ledger file is dropped from the
ledger. A file that cannot be read (permissions, a lock held by another
program) is left out and stays pending; if the commit with the sweep still
fails while the caller's own change alone would land, the caller's change
is committed and the whole sweep stays pending. Each file left out is
returned by `LibraryAdapter::take_held_back_files`, and the app names it in
the Messages panel on every commit it blocks (`sync_uncommitted`).
