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
palette). It runs an installed library converter
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

**Next** checks the folder on a worker: absent or empty is accepted, a file
or a folder that holds anything is refused with a banner and nothing runs.

**Step 2, preview.** A `--dry-run` into a new temp folder
(`signex-library-preview-<uuid>` under the system temp folder, removed when the
dialog moves on or closes). It shows the counts (parts, symbols, footprints,
3D models, skipped), a chip per report severity present (a press hides or
shows that severity in the list), a filter over the items' source name,
message and code, the list (first 200 rows), and the selected item's kind,
source, code, what it was written as and its detail fields. A dry run writes
only the report (contract §2.2), so no symbol or footprint is drawn here; the
caption says they can be looked at after the import (a drawn preview is
follow-up #3171; approved deviation, DECISIONS.md 2026-11-27). The amber strip counts
what will be skipped. Back cancels a running dry run. A failed dry run is
shown in the step, not as a notice.

**Import** checks the folder again. The run itself takes the folder
(`library_import::run::claim_output`): it creates it, replacing an empty
folder and refusing one that holds anything before the converter starts, so
a failed or cancelled run removes only a folder it made. Next waits ("The
previous run is stopping…") while a dry run left by Back or Close has not
ended; that dry run's temp folder is removed when it does.

**Step 3, import.** The phase name with "done of total" and the percent, the
bar, "Now: <item>" from the progress stream, every phase so far, and the
silence rule (no event for 120 s stops the run). Cancel stops after the
current item: an item is in flight while the last event is a `progress`
naming one, and the run is stopped on the next event that is not a repeat of
it; with no item in flight it stops at once. While it waits the button reads
Stop now: a second Cancel (or Esc, or Close) stops the run at once, so a
converter stuck on one item, silent, or never exiting is always stopped.
Quitting kills a running converter's process group. Esc and the close button cancel
too while the import runs; the dialog stays until it has stopped, then says
whether the folder was removed, with Back and Close.

**Done: result and loss report (#3121).** A finished import stays on step 3
("Step 3 of 3 · Done") with two tabs. Result is the result card: the library
name, tiles for imported parts, approximated, dropped and skipped items, the
library folder, and View loss report with the counts. Loss report is the
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
