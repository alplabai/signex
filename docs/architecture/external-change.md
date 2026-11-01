# Saves and the file behind a document (#1826, #1827, #1828)

A save must write the file the document is, and must not overwrite or invent
what the user did not mean.

## Missing folder (#1828)

`atomic_write` creates parent folders, which is right for New Project, exports
and caches. Document saves go through `atomic_write_existing_dir`
(`signex-types/src/atomic_io.rs`): the sink behind every document write
(`persist::AtomicFileSink`) and the engine's own saves use it, as do the library
adapter's primitive, symbol-container and manifest writes and `rewrite_file`
(a library that means to create a folder does so first), so a save after the
project or library folder moved fails with `MissingParentDir` instead of
scattering files into a re-created half project. The failure keeps the sheet modified
and reaches Messages like any save failure.

## Symlinks (#1827)

Only the document-save variant (`atomic_write_existing_dir`) writes through a
symlink, and only to a regular file: the temp file goes beside the target so the
rename stays on one filesystem, the link survives and the read-only refusal
applies to the target. A dangling link, or a link to anything else, is refused.
Plain `atomic_write` replaces a link and never follows it: its callers (caches,
sidecars, prefs) must not be steered outside their folder by a planted link.

An open that reads a path whose real file (`canonicalize`, taken on the read
worker) is the file of another open sheet shows that sheet and makes no second
engine (`focus_alias_if_open`). A project reached through a symlinked folder is
matched the same way in `load_or_activate_project`. Case and normalisation
variants stay with `held_spelling` (#1149); this is the part that needs the
disk.

## External change (#1826)

`DocumentSaves` keeps a baseline per open sheet: the SHA-256 of the bytes it was
read from, or last written as. Opens record it (`DiskRead`), a landed write
replaces it (only the newest write begun for the file may).

The save job (`DocumentSaveJob::run`, on the worker) reads the file right before
it writes and compares: the baseline, or any earlier write of this process to
the same file, is accepted. Anything else returns `SaveOutcome::DiskChanged`
with `DiskDrift::Changed` or `Missing` and writes nothing. A rapid second save
is therefore not drift, whatever order the jobs run in.

On `DiskChanged` the sheet stays modified, the failure goes to Messages and into
the save ledger (Save All lists it), and the notice card offers:

- **Overwrite** (Missing: Re-create): one save that is not compared; a failed
  write leaves the baseline, so the next save asks again.
- **Reload from disk**: read and parse the file on a worker, then replace the
  document as a History restore does. The unsaved edits are dropped only when
  the read succeeded.
- **Cancel** (dismiss): nothing changes; the next save asks again.

One card shows at a time; a second drifted sheet of a Save All is in Messages
and asks on its next save. The one blocking save (`try_save_dirty_path_with`, used by Exclude from Project
on a dirty sheet) checks through `DocumentSaves::drift_of`, which accepts the same
fingerprints as the job and refuses with an error naming the reason. Close and
quit go through the async Save All, which uses the job.

## Not covered

- A sheet with no baseline (created in memory, renamed in the project tree,
  opened by a route that does not record one) is saved without the comparison.
- PCB saves are not compared.
- There is no background poll: a change is found when the user saves.
