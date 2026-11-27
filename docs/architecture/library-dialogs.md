# Library dialogs

How the library dialogs that follow the Graphite mockup rows 129-138 are
built. The code keeps a one-line summary and points here.

## New Library dialog

`crates/signex-app/src/library/create_options.rs` (mockup row 133): the
version-control options of a library about to be created.

It pops up between the New Library save-as dialog (where the user picked the
`.snxlib` path, shown as the header note) and the registration of the pending
library. Two option cards:

- "Enable version control (Git)", off by default so a fresh library lives on
  disk as plain files;
- "Use Git LFS for binary 3D models", indented under it, greyed and inert
  while Git is off (disable-when-off).

A refused Create shows as an error banner with "Choose another location…", and
Create stays disabled until the refusal clears. Version control off clears a
refusal for a folder that is the root of a repository with history. A library
already at the path stays refused either way, and its banner adds "Open
existing library". A file name that is not portable and a path the project
already waits to create are refused the same way and also survive the toggle.

It is mounted as a full-screen overlay backdrop via
`app/view/mod.rs::collect_overlays` when `LibraryState::create_options` is
set; `create_options_overlay` owns that guard.

## Primitive picker

`crates/signex-app/src/library/primitive_picker/mod.rs` (mockup row 131):
Pick Symbol, Pick Footprint and Pick Sim Model.

Listing source: every mounted library's cached primitive summaries
(`OpenLibrary::cached_symbols` / `cached_footprints` / `cached_sims`), so the
view does no disk IO. The filter matches the primitive name and its uuid,
case-insensitively; the uuid is not shown.

```text
┌─[Pick Symbol   for R-0402-10K-1% ─────────────────── X]─┐
│ [⌕ Filter…]                                              │
├──────────────────────────────────────────────────────────┤
│ PASSIVES (3)                         │ ┌───────────────┐ │
│   resistor.snxsym · R                │ │    drawing    │ │
│ IN LIBRARY POWER-ICS (NOT OPEN) (1)  │ └───────────────┘ │
│   sense.snxsym · R_Shunt             │ 2 pins · 1 part   │
│ NOT IN A LIBRARY (1)                 │                   │
│   C:\work\old\res_trim.snxsym        │                   │
├──────────────────────────────────────────────────────────┤
│ Browse filesystem…                   [ Cancel ] [ Pick ] │
└──────────────────────────────────────────────────────────┘
```

A row click highlights the row and previews it, read off the UI thread; Pick
confirms (decided 2026-11-22). Pick and the preview follow the highlighted row
only while the filter shows it. A file named by a uuid lists as `<name>.<ext>`.
Pick on a symbol outside the row's library opens the copy confirm (below) in
place of the list. A row of a library that is not open has no button of its
own: Link without copying and Copy into are on that confirm.

### Copy into library confirm

`crates/signex-app/src/library/primitive_picker/copy_confirm.rs` (mockup row
131, decision lib-10): the small card that takes the picker's place while a
chosen symbol outside the row's library waits for an answer (#1841).

- A file in no library: Cancel, or copy it into the row's library. A row binds
  a library id and a uuid, so there is nothing to link to.
- A symbol of a library that is not open: Cancel, link without copying (open
  that library and bind its symbol), or copy it into the row's library.

Cancel goes back to the picker and changes nothing.

## Library Updates dialog

`crates/signex-app/src/library/updates_dialog/mod.rs` (mockup row 135).

It surfaces drift between a freshly opened schematic's placed `Symbol`s and
their source library rows. Each placed Symbol that went through the `.snxlib`
picker carries `library_id`, `row_id` and `library_version` (Stage 16's schema
additions on `signex_types::schematic::Symbol`). On schematic open the
dispatcher walks every Symbol that has a `library_id` set and compares its
pinned version to the row's current version through the mounted `LibrarySet`.
Mismatches accumulate into a `LibraryUpdatesState`.

In **Personal** workflow mode (the manifest's `[workflow] mode = "personal"`
default) drift is auto-applied to every instance silently: no modal opens,
just a `tracing::info` line and a dirty mark on the schematic.

In **Team** workflow mode the dispatcher opens the modal so the user can
review which updates to accept, with checkboxes pre-toggled by `BumpKind`:

- `Patch`: checked by default (compatible under semver);
- `Minor`: checked by default (compatible under semver);
- `Major`: unchecked, with a warning (likely breaking).

Instances of one part that would draw the same line but for the designator
(same `lib_id`, library, row, Internal PN and version pair) and carry no
conflict share one table row listing every designator ("R10, R11"); the
grouping is `LibraryUpdatesState::rows` in `updates_dialog/rows.rs`. The row's
checkbox shows ticked only when every member is ticked, and a click on a
row with mixed ticks ticks them all. A part with a conflict
keeps its own row, since its answers are per instance. The summary count and
the per-kind pills count table rows; "Update Selected Components (n)" still
counts instances.

"Update Selected Components" rewrites the picked Symbols to the latest version
and dirty-marks the schematic. "Skip All" leaves everything pinned, records the
schematic in `LibraryState.skipped_updates_for` and remembers each offered
`(lib_id, version)` for that sheet in `LibraryState.skipped_versions`
(`updates_dialog::SkippedVersions`, session state). A later scan of the sheet
leaves those entries out and keeps the sheet pending; a part whose row moves to
another version is offered again. "Cancel" closes the modal and remembers
nothing, so the next scan asks again. Nothing renders `skipped_updates_for`
yet; the user-visible trace of a refused or unreadable update is its line in
the Messages panel.

It is painted by `library_updates_overlay` via `collect_overlays`, which runs
every frame; the builder's `library_updates.is_some()` guard is the only open
check. The card itself is in `updates_dialog::view`.

## Import Library dialog

`crates/signex-app/src/library/import_dialog/` (mockup row 211, #3117–#3125),
opened by File ▸ Import ▸ Library… (`import_library`, also in the command
palette), by Import… in the Library Browser's LIBRARIES header, and by Import
library… in a project's Project tab ▸ Libraries section (#3122). Opened from a
project, Mount for all projects starts off and "This project" means that
project (`ImportDialogState::project`), whether or not it is the active one;
an open dialog stays as it is. The user's view of it is USER_GUIDE.md,
"Importing a library" (#3125). It runs an installed library converter
([LIBRARY_CONVERTER_CONTRACT.md](../LIBRARY_CONVERTER_CONTRACT.md)) through the
engine in `crate::library_import`, in three steps under a stepper; the header
note repeats the step ("Step 2 of 3 · Preview"). Handlers:
`app/dispatch/library/import_dialog/`. It is a tool window (resizable), and
`ModalId::LibraryImport` follows `import_dialog` like the other library
dialogs.

**Step 1, source and options.** Opening the dialog searches for converters on
a worker (`discovery::discover`, answered from the session's cache when PATH
and the candidates are unchanged). With none, an info notice gives the install
hint, lists why candidates were set aside, and offers Look again; Next stays
off. Signex ships no converter (contract §1), so the mockup's "Install
converter…" is Look again. The source is several library files, or one
database link file when a converter reads that kind (a Files / Database
choice shows only when both are offered); the picker's filters are built from
every converter's `describe` inputs, so no foreign extension is compiled in.
Detected shows a chip per input kind the converter matched and "Read by
<converter> <version>"; when several converters read every picked file, a
chip per converter chooses one. A file no converter reads, or files no single
converter reads together, keep Next off. A database input adds the Database
override (passed as `--db`; empty means the link file's own). Name follows the
first file's stem until typed; Location is `~/Signex/Libraries/<name>` and
follows the name until changed by hand. Mount for all projects is on by
default; off adds the library to the active project only, and with no project
open it is forced on and greyed. Include 3D models (embedded) passes
`--models include|skip`. The mockup's "Track with version control" switch is
not built: every import is a git snapshot (see the 2026-11-26 snapshot row in
DECISIONS.md).

**Next** checks the folder on a worker (`import_dialog/target.rs`): absent or
empty is accepted; a link, a file, or a folder that holds anything is refused
with a banner and nothing runs, except a folder that holds an imported
library.

**Updating an import (#3123).** A folder with a `.snxlib` in a git repository
with no uncommitted changes is offered as an update: a banner says what an
update does (one new commit; only what the converter wrote is deleted or
overwritten; parts the user added stay), warns when the library has commits
that are not imports (`UpdateCheck::local_commits`), and offers Update this
library; Next stays off until it is taken. Taking it runs the check again and
the preview; the preview's button reads Update with N parts and runs
`--update` over the folder. A library with no repository, one with
uncommitted changes, and one with unsaved editors or documents in the app are
refused with their own reasons and nothing runs; a part-number clash is the
converter's own refusal (contract §5) and shows as its failure, with the
library put back to its last import. Any edit of the sources, name or folder
drops the offer. On step 3 the silence and Cancel lines say a stopped or
cancelled update puts the library back to its last import, and a failed one
says whether it was put back (`FolderAfter`).

**What went wrong (#3124).** A picked file no installed converter reads
shows a warning under Detected with how to get one, why converters were set
aside (one too new for this Signex says which contract it speaks, from
`describe`), and Look again. A failed dry run (step 2) or import (step 3)
names a converter that ended without a result as having crashed or been
stopped from outside, with its exit status; one that exited 1 with its own
error as unable to convert the library, with that error; anything else in the
engine's words (`library_import::error`). Under each, the last 12 non-blank
lines of the converter's error output, in a mono box
(`import_dialog::notices`). An import that exited 2 ends on its result card
with an amber line counting what was dropped or not imported, and the loss
report lists each.

**Step 2, preview.** A new import runs the converter for real (no
`--dry-run`) into a staging folder beside the target, `.<name>.signex-preview-<id>`
in the target's parent, so the same file system (`RunMode::Stage`,
`library_import::staging`, #3171; decided by Caner, DECISIONS.md
2026-11-27). The run creates that folder (an existing one is refused, never
adopted) and records whether the target was absent or empty. The dialog
holds the result (`ImportDialogState::staged`) and draws it: a Parts tab
with the part table (part number, symbol, footprint, up to two alternate
footprints, first 200 rows, a filter) read on a worker through
`LocalGitAdapter::open_to_read`, and for the selected part its symbol and
footprint (the Components panel's `load_details`, `symbol_preview` and
`FootprintPreview`; a chip per footprint switches to an alternate), its
manufacturer and part number, suppliers and parameters
(`import_dialog::parts`, handlers in `import_dialog/staged.rs`). Under the
counts, a Report items tab keeps the report list: severity chips, filter,
the list and the selected item's details. An update's preview is still a `--dry-run`
into a temp folder (`signex-library-preview-<uuid>` under the system temp
folder), report only, with the caption that the drawings can be looked at
after the update. The amber strip counts what will be skipped. Back cancels
a running preview. A failed preview is shown in the step, not as a notice.

The staging folder is removed on Cancel, Back, a failed or cancelled run,
the dialog's close, its window vanishing, and at quit
(`discard_import_preview_then`, after the import run's wait), through the
identity recorded when it was made and without following a link
(`ClaimedOutput::discard`, `empty_dir`). A staged run that ends after a stop
was asked for is discarded instead of shown. Going back to step 1 removes
it, so changed options always preview again; Import also compares the
dialog's request with the previewed one and previews again when they
differ.

**Import** checks the folder again. For a staged preview the job adopts it
(`ImportJob::adopt`) instead of running the converter: just before the move
it checks the target is still absent or the same empty folder (not a link
or other reparse point). An absent target is the staging folder renamed; an
empty one is kept as it is, with its permissions, and each staged entry is
renamed into it, then the emptied staging folder removed (either way a copy
then a removal across file systems). A move that fails half way moves what
it moved back into the staging folder, as far as it can, and keeps the
preview. The library is then committed and mounted as an import is. A
target that was created or changed meanwhile is refused at either check;
the dialog stays on step 2 with the reason (`move_problem`) and the preview, for another try. Step 3
reads "Moving the previewed library into place…" while it runs. An update
runs over its folder under the library's write lock. A run that writes a new
folder takes it through `library_import::output::claim_output`: it creates
it with its parents, a dry run adopts an empty one as it is (a staged run
refuses anything already there), and one that holds anything is refused
before the converter starts. A failed or cancelled run removes only what it
created: a folder it made and the parents it made while they are empty, or
what it wrote into an adopted folder, which is kept. On Windows the run
holds an adopted folder open until it ends, so it cannot be renamed or
deleted meanwhile; a folder the run created (a staged preview's) is not
held, since a hold would block its own removal and its rename into place,
but its volume and file index are recorded and checked before it is
removed, and an empty target is held while the staged entries move into
it. Next waits ("The previous run is stopping…") while a preview left by
Back or Close has not ended; its folder (a dry run's temp folder or a
staging folder) is removed when it does.

**Step 3, import.** For an update (a new import only moves its staged
preview), the phase name with "done of total" and the percent, the bar,
"Now: <item>" from the progress stream, every phase so far, and the
silence rule (no event for 120 s stops the run). Cancel stops after the
current item: an item is in flight while the last event is a `progress`
naming one, and the run is stopped on the next event that is not a repeat of
it; with no item in flight it stops at once. While it waits the button reads
Stop now: a second Cancel (or Esc, or Close) stops the run at once, so a
converter stuck on one item, silent, or never exiting is always stopped.
Quitting kills a running converter's process group (its Job object on
Windows, #3134). Esc and the close button cancel
too while the import runs; the dialog stays until it has stopped, then says
whether the folder was removed, with Back and Close.

**Done: result and loss report (#3121).** A finished import stays on step 3
("Step 3 of 3 · Done") with two tabs. Result is the result card: the library
name ("imported" or "updated"), tiles for imported parts, approximated,
dropped and skipped items, the library folder, where it was mounted (for all
projects, for the project, "Mounting…" until that is settled, or not mounted
when an open library has the same id), its history (the import commit, or
that an unchanged update added none), the libraries its manifest `requires`
(by name when open, by id when not), for an update the local-commits warning,
and View loss report with the counts. Loss report is the
report the run wrote (`import-report.json`), with the same severity chips and
filter as step 2, grouped by severity (most severe first, each with what it
means for the item) and then by code (most items first), each row the item's
name in the source and the converter's message; at most 200 rows, with a
caption when the filter leaves more. The footer has Save report…, which copies
the library's `import-report.md` to a file the user picks (an atomic write;
nothing is written until a file is picked), Open report (the `.md` in the
system viewer), Close, and Open in Library Browser, on once the library is
mounted. Model: `library::import_dialog::result`; handlers:
`app/dispatch/library/import_dialog/report.rs`.

**The stored report.** A right-click on a library in the Library Browser's
LIBRARIES list opens a menu under it with View import report, which reads the
library's `import-report.json` on a worker and opens the dialog on that loss
report alone (no stepper; the header note names the library), or on why there
is none. An open dialog stays as it is, as File ▸ Import ▸ Library… leaves it.
The menu's open state is `LibraryState::import_menu`.
