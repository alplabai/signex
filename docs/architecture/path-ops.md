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
because the adapter still points at the old path until the re-point.

The one library entry of the saved `.snxprj` is patched atomically on
disk (so a Discard, quit or crash cannot leave the project listing a file
that no longer exists), but every other unsaved project edit stays in
memory and the project stays dirty for it. A project that was never saved
has no file to patch, so it is marked dirty and the Messages panel says it
needs saving.

Only project-local libraries are renameable here. Shared and Global
libraries live outside the project and stay refused (#657).
