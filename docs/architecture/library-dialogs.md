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
(same `lib_id`, row, Internal PN and version pair) and carry no conflict share
one table row listing every designator ("R10, R11"); the grouping is
`LibraryUpdatesState::rows` in `updates_dialog/rows.rs`. A part with a conflict
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
