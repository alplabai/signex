# Project document tab, shell + General section (#1202, epic #1084)

> Note (#661): the dirty-state names below (`dirty_paths`, `TabInfo::dirty`, `sync_editor_dirty_paths`) are superseded. A document's unsaved state is `DocumentState::is_document_dirty`; see `docs/DIRTY_STATE.md`.

Status: implemented in `feat/1202-project-tab-shell`. Scope: the first slice of
#1084. The read-only Project Options modal is replaced by a **Project** document
tab with one section, General.

## 1. How the existing document kinds plug in

| Concern | Schematic | PCB | Symbol / footprint editor | Library Browser |
|---|---|---|---|---|
| Tab key | `TabInfo.path` (`.snxsch`), `TabKind::Schematic` | `.snxpcb`, `TabKind::Pcb` | the file path, carried in `TabKind::SymbolEditor(path)` / `FootprintEditor(path)` | the `.snxlib` dir, `TabKind::LibraryBrowser(path)` |
| Document state | `DocumentState::engines[path]` | `TabInfo::cached_document` | `DocumentState::{symbol,footprint}_editors[path]` | `LibraryState::library_browsers[path]` |
| Dirty | `dirty_paths` + `TabInfo::dirty` (the tab bar reads either, `tab_bar::tab_is_dirty`) | same | editor flag mirrored into `dirty_paths` by `sync_editor_dirty_paths` | n/a |
| Undo / redo | engine history (`signex_engine::Engine`) | n/a | per-editor snapshot history | n/a |
| Save | `begin_schematic_save`: encode on the UI thread, `DocumentSaves::begin` hands out a ledger-ordered job, `spawn_blocking` writes, `DocumentSaved` completion clears dirty only at the same engine revision | `begin_pcb_save`, same completion | `save_primitive_tab_at`, its own completion, same ledger (`begin_work`) | n/a |
| Save All (close / quit) | `begin_held_path_save` routes each dirty path by kind | — | — | — |
| Close | `close_tab`: never prompts. A dirty document is **parked** (engine / editor kept, `dirty_paths` kept); Close Project and quit prompt for everything in `dirty_paths` | marker dropped | parked | browser state dropped |
| Undock | `WindowMsg::UndockTab` → `WindowKind::UndockedTab { path }`; `view_center` renders the window's document | same | main window only | main window only |
| Quit flush | `flush_quit_writes_then` → `wait_for_document_saves_then` waits the ledger barrier | same | same | — |

The `.snxprj` has been edited in place all along: `LoadedProject::data` is the
in-memory document, its dirty bit is `dirty_paths.contains(project.path)`, and
Save writes the whole `data` (`persist_project_by_id`, synchronous until now).
Net classes (#1042) already edit `data.net_classes` through a
`signex_engine::NetClassHistory` keyed by `ProjectId`.

## 2. Decisions

**Key and kind.** `TabKind::Project(ProjectId)`, with `TabInfo.path` = the
`.snxprj` path and `TabInfo.project_id = Some(id)`. `ProjectId` is minted once
per load and never reused, so it is the instance id (ADR-0003 D1/D4); the path is
what `dirty_paths`, the tab bar and the save ledger key on. One tab per project:
opening it again focuses it. Title is the file name (`Oscar.snxprj`), so it never
reads like the root sheet's tab (`Oscar`).

**State.** `DocumentState::project_tabs: HashMap<ProjectId, ProjectTabState>`
holds the selected section and the General history. It lives as long as the
project is loaded, not as long as the tab: closing a dirty Project tab parks it
like any other document (the edits stay in `data`, the marker stays in
`dirty_paths`), and reopening the tab brings its undo history back. Close
Project drops the entry.

**No draft copy of `ProjectData`.** The tab edits `LoadedProject::data` in
place, through `update` only. Text input changes are committed per keystroke;
consecutive edits of one field coalesce into one undo step, so there is no owed
("typed but not committed") state for a save or a tab switch to flush (ADR-0003
D2 is then trivially met).

**History.** `signex_engine::ProjectGeneralHistory`, an engine-owned snapshot
history of the field the General section edits (`description`), shaped like
`NetClassHistory`: commands are plain data (`ProjectGeneralCommand`), an
accepted command is one undo step unless it extends the open typing run; undo,
redo, a section switch and a save close the run, so what is typed after a save
is its own step. Ctrl+Z / Ctrl+Y route by the window they were pressed in
(#1209): in the ERC dialog's window they step its rule history (#1190); in the
main window with a Project tab active, `handle_undo_requested` steps this
history. `project_tab/undo_routing_tests.rs` pins both. Command enablement
reads the same history.

**Format.** `ProjectData::description: Option<String>`,
`#[serde(default, skip_serializing_if = "Option::is_none")]`. An empty
description is stored as `None`, so clearing it removes the key and a file that
never had one re-saves byte-identical.

**Name: not a document edit.** The project name is the `.snxprj` file stem:
`parse_project` overrides the stored `name` with the stem. Writing an edited
name into the JSON would lose it on the next open, and renaming the file as
part of a save made a save block on the ledger barrier inside `update` while a
write handed out in the same `update` could only run after it returned (a
deadlock, reproduced in review). So the General section shows the name
read-only with **Rename…**, which opens the project tree's Rename dialog and
runs its flow (`handle_project_rename_submit`, with the folder move in
`rename_project_on_disk`) immediately, as its own message. The name is
not in the tab's history, nothing dirties the project, and no save path renames
anything. That flow:
- refuses (dialog error) while a write of the `.snxprj` is in flight, instead of
  waiting for it: that write only runs after `update` returns;
- handles a case-only rename (`Oscar` → `oscar` on a case-insensitive volume)
  with a single `std::fs::rename`, guarded by `same_path_on_disk` so the
  existing-name check does not mistake the file for a collision; APFS and NTFS
  record the new spelling, so no temporary name is needed;
- re-keys the project, tabs (the Project tab keeps its `.snxprj` title), engines,
  active path, dirty marker, save bookkeeping, page setup, undocked windows and
  File > Open Recent, and logs a `[rename]` line to Messages.

**Docked only.** A Project tab is not undocked in this slice: commands typed in
an undocked window act on the main window's active tab, so Ctrl+Z / Ctrl+S
there would reach another document. Open In New Window is greyed for it and
`handle_undock_tab` refuses it.

**Save.**
- Ctrl+S in a Project tab: `begin_project_save(id)`. On the UI thread it
  materialises pending libraries (unchanged behaviour) and encodes the bytes (`signex_types::project::encode_project`, split out of
  `write_project`); `DocumentSaves::begin` hands out the ledger-ordered job, which
  runs under `spawn_blocking`. The completion `ProjectMsg::FileSaved` frees the
  slot (`finish`), and clears the dirty marker only if the project with that id
  is still loaded at that path, has no pending library, its current data
  encodes to exactly the bytes written, and this job is the newest begun for
  the file (`DocumentSaves::is_newest_begun`, by ledger sequence). A failure, or a lost task (`JoinError`),
  reaches the Messages panel and is recorded for Save All (D5).
- The other `.snxprj` saves (Ctrl+S in a schematic / primitive tab when the
  project is dirty, Save All on close / quit) stay synchronous, as before, but now
  write through the same ledger (`begin(..).run()`), so a later synchronous write
  always wins over an earlier off-thread one, whatever order they finish in.
- Save All needs no new branch: the project's dirty marker is in `dirty_paths`.

Why bytes and not a revision counter: `ProjectData` has no mutation gateway yet;
every writer listed in §3 mutates it directly. A revision counter would miss every
writer that does not bump it. Equality of the persisted form catches every
edit made since the snapshot, and "memory encodes to what is on disk" is the
definition of clean. Equality alone is not enough with two writes in flight:
an edit reverted to an older snapshot's bytes matches that snapshot while a
newer write, holding the edit, still lands on disk. So only the newest write
begun for the file may clear the marker; an older completion never does, and
a failed newest write leaves it dirty.

## 3. The `.snxprj` concurrent-writer rule

**Every writer edits only its own fields of the one in-memory document
(`LoadedProject::data`) and marks the project dirty; every save writes the
whole current document; nobody holds a copy of `ProjectData` across messages.**
Consequences:

1. The Project tab cannot clobber another writer: it has no draft to write back.
   Its undo snapshots hold only `description`, so undoing in the tab never
   reverts a rename, variant, library, sheet, net class or rule change.
2. Another writer cannot clobber the tab: its change lands in the same `data`,
   and the next save writes both.
3. A save started before another writer's change writes the older snapshot; its
   completion sees that the current data no longer encodes to the written bytes
   and leaves the project dirty. The next save writes both changes.
4. A writer that writes or moves the file **directly** (library rename's
   `retarget_library_entry`, project rename) refuses while an off-thread write
   of it is in flight, so that write cannot land after it. It refuses rather
   than waits: the write runs only after `update` returns.

Writers on trunk (all follow the rule): Add Existing / Add New (sheets, PCB,
libraries), Remove from Project, file rename (`schematic_root` / `pcb_file`),
project rename (file move + `name`), Enable Version Control (`enable_git`), net
classes (#1042), library registration and recovery (`libraries`), pending-library
materialisation at save, library rename (`retarget_library_entry`, disk + data),
the ERC dialog's project rule set (#1190: `data.rules`, its own history, marks
the project dirty). Variants have no in-app writer yet; when their section lands
it follows the same rule, with its own field-scoped history.

## 4. ADR-0003 checklist for the off-thread project save

1. **Owed state:** `DocumentSaves` (in flight per ledger key); instance =
   `ProjectId`; "generation" = the encoded bytes (see above).
2. **Flush:** write-type. The project rename and `retarget_library_entry`
   refuse while the `.snxprj` is being written (no wait inside `update`);
   project close and quit already wait the barrier.
3. **Undo snapshots:** General fields only, always finished (edits are
   synchronous).
4. **Late result:** id + path + exact bytes + newest-begun sequence, else the
   marker stays.
5. **Lost result:** `JoinError` maps to `Err`, reported to Messages and recorded
   for Save All; the job's `InFlight` guard frees the barrier when dropped.
6. **One authority:** one rename flow (`handle_project_rename_submit`) for the tree and
   the tab; the write goes through the one ledger.

## 5. Out of scope (later slices)

Parameters (#1080), Sheets (#1081 / #1082), Variants, Libraries, ERC and
annotation (#1085), Version control sections; a menu / command entry beyond the
existing Project Options item (menu content is not this slice's call); taking
the project rename's file move off the UI thread (it stays synchronous, as in
the Rename dialog); recording a project rename in the project's git history
(the old file name's removal is not staged; the rename was never committed by
the Rename dialog either); undocking the Project tab.
