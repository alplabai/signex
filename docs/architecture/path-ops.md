# Renames and deletes off the UI thread (#859)

## Registry (`app/path_ops.rs`)

Filesystem moves and deletes that run off the UI thread (#859).

A rename or delete used to run inside one `update`, so nothing could touch
the file between the disk change and the in-memory re-key that followed it.
The disk work now runs on a worker (`path_op_jobs`) and the re-key happens
in the completion message, which opens a window the synchronous code never
had: the user can save, reopen or restore the same file while the move is
in flight, and a write landing then recreates the old path or is lost.

[`PendingPathOps`] closes it. `update` registers the path before it
returns the `Task`; every entry that writes to, opens, or starts another
move of a path asks [`Signex::refuse_while_path_op`] first and refuses with
a reason-first message. The completion drops the registration.
Edits to an open document are not refused: the completion re-keys engines,
tabs and dirty markers by moving them, so edits made meanwhile travel with
the document.

### The document epoch (#1401)

A move or delete changes what is on disk behind the engines, so it obeys the
`documents_epoch` contract in `state/mod.rs`: a background read of a sheet
(project Replace's `prepare`) records the epoch and applies only if it is
unchanged. Every operation moves the epoch twice. Starting it bumps it, so a
read already in flight is out of date before the worker touches the file.
Completing it bumps it again (`FileRenamed`, `LibraryRenamed` and its rollback,
a delete through `drop_deleted_documents`, a project-folder move), whether or
not an engine was
open for the path, and re-keyed or rewritten engines go through
`insert_engine` / `remove_engine`. Otherwise a Replace whose read finished
before the move would install a dirty tabless engine at the old path, and Save
All would write the moved or deleted sheet back.

A project-wide rewrite that starts while an operation runs is refused up front
with a card and a Messages line: project Replace (`refuse_replace_while_path_op`)
and Annotate / Reset Duplicate Designators (`refuse_annotate_while_path_op`).
Both use `refuse_project_wide_while_path_op`, which checks the sheets the pass
reads (Replace: the matched sheets and the root; Annotate: the parsed sheets),
the project's declared pages and its folder. Open sheets of other projects are
not read, so their operations do not refuse it. Between the worker's move and
the completion, a moved page is in neither the parsed sheets nor the unreadable
list, so the sheets alone would miss it.

The folder check is deliberately broad: any operation under the project folder
(a `.snxsym`, `.snxfpt` or library rename too) refuses these passes until it
finishes. It cannot tell an unlisted sheet mid-move from any other file, and the
refusal lasts only as long as the worker.

## Library rename (`handlers/dock/project_navigation/rename_library.rs`)

Renaming a project library's `.snxlib` from the project tree (#739).

The file move and the patch of the saved `.snxprj` entry run on a worker
(#859; `path_op_jobs::LibraryRenameJob`), which also moves the file back if
the project file cannot follow. When it reports back, one `update`
re-points everything that knows the library by its path: the project's
`LibraryEntry`, the mounted adapter, and every open tab, editor map and
dirty marker, so nothing in memory ever names a path that does not exist.
If that re-point fails (the project closed meanwhile, the mount was
busy), a second worker undoes the project patch and the move, and the
in-memory state stays on the old path throughout.

While the rename runs the library is registered in `pending_path_ops`:
saves, opens and a second move of it are refused, and library edits that
write through the mounted adapter are refused (`library_write_refusal`),
because the adapter still points at the old path until the re-point. A refused
edit shows a "Library Change Not Applied" card as well as a Messages line.

The one library entry of the saved `.snxprj` is patched atomically on
disk (so a Discard, quit or crash cannot leave the project listing a file
that no longer exists), but every other unsaved project edit stays in
memory and the project stays dirty for it. A project that was never saved
has no file to patch, so it is marked dirty and the Messages panel says it
needs saving.

Only project-local libraries are renameable here. Shared and Global
libraries live outside the project and stay refused (#657).

## Project file save (`handlers/document_files/project_save.rs`)

The `.snxprj` save runs no filesystem or git work in `update` (#859).
`begin_project_save` plans the write from the model (`persist::ProjectWritePlan`:
a snapshot of the project data, the pending libraries in a fixed order, the
new-library defaults) and returns one `Task`. Its worker creates each pending
library (`git init` included), lists the ones it created in a copy of the
snapshot, encodes that and writes the file through the document-save ledger.
`ProjectMsg::FileSaved` carries every result back.

One job rather than two chained ones, because the file must come after the
libraries it lists, and a single completion then holds both outcomes for one
`update` to apply. It applies, in this order:

1. Each created library, if the same loaded project (by `ProjectId`, not by
   path) still has it pending: register the entry, drop it from
   `pending_libraries`, and mount it through the usual off-thread
   `request_mount`. This runs whatever the write's own result was, since the
   library is on disk and a retry would fail on "already exists". A library
   nothing asks for any more is reported and left on disk.
   `create_pending_library` accepts a library already at the path when its
   file has the same `library_id`, so a save lost between the init and the
   file write is finished by the retry. It adopts it only after
   `LocalGitAdapter::complete_init` has made the version control match the
   request (git repo with a commit, LFS rules). Its commit holds only the
   `.snxlib` and `.gitattributes`, built in memory, and is refused when the
   directory is a repository with history that does not track the library (the
   project's own); if anything fails the library stays pending and the refusal
   names the partly created library.
2. A library that could not be created stays pending and is reported (Messages
   panel and the Save Failed card); the file is still written without it and
   the project stays modified.
3. The write result. The project is marked clean only for the newest write
   begun, when it has nothing pending and still encodes to exactly the bytes
   written, so an edit made meanwhile keeps it dirty. A written file moves
   `documents_epoch` (#1401).

A save asked for while a write of the file runs (`is_saving`) queues one
follow-up in the ledger instead of starting a second writer; it starts when the
first completion is handled. Save All on close and quit starts the same job and
waits for its completion through the `SaveAllSettle` chain; one that finds the
write already running stays open and names it, as for a primitive. The quit
barrier covers the job from `begin` until it is dropped. Saves are refused
while a rename, delete or folder move touches the project or a pending
library's path.

## Annotate and Reset Duplicate Designators (`handlers/erc/annotate_job.rs`)

Neither pass reads or writes a file in `update` (#859). A run has three steps:
a worker reads the project's sheets (`plan_project_sheets_for(..).assemble()`,
the same reader ERC uses); ONE `update` numbers them, open sheets in place
through their engines and unopened sheets in memory, then encodes each
unopened sheet; a second worker writes those through the document-save
ledger (`document_saves.begin`), so quit waits for it and an open of the file
is refused while it runs. `PassSheetsRead` and `PassSheetsWritten` (under
`AnnotateMsg`) carry the results back. The shared counters, the part grouping
and the sheet order are untouched, so the designators are the ones the
synchronous pass produced (`annotate_golden_tests.rs` pins them).

**Stale results are all or nothing.** A partly applied run could give two
sheets the same designator, so when the read result arrives the run is dropped,
with a Messages line and a notice and nothing changed, if `documents_epoch`
moved (any open, save, close, rename, delete or folder move), if an open
sheet's child-sheet placements changed, if the active project or its root
changed, or if a rename, delete or folder move now holds one of the sheets.
Open sheets are always numbered from their live engines, never from the
snapshot the worker read, so a symbol typed meanwhile is counted and kept. A
sheet whose file a History restore, a read or a save still owns is skipped, as
before.

**Guards.** The run is registered in `pending_path_ops` (`ProjectRewrite`) from
`begin` until its last write completes: the declared pages and root while the
sheets are read, the unopened sheets while they are written. A rename, delete,
open or save of them is refused meanwhile. One run at a time: a second request
is refused with a notice rather than queued, because a queued run would number
the project as it was when the first started.

**Failures.** A sheet that cannot be written keeps its old bytes (the write is
atomic), is not marked clean, and is named in the Messages panel and a notice;
the open sheets stay numbered and modified, and the other sheets still land.
Run it again once the cause is fixed. A dead worker releases the guards and
reports the same way. Reset marks an unopened sheet clean only after its bytes
landed. Re-prefix still assembles and numbers inline; it is not part of this
change.
