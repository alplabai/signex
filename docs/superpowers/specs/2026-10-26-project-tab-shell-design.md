# Project document tab, shell + General section (#1202, epic #1084)

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
history of the fields the General section owns (`name`, `description`), shaped
like `NetClassHistory`: commands are plain data (`ProjectGeneralCommand`), an
accepted command is one undo step unless it extends the open edit of the same
field, undo / redo close that run. Ctrl+Z / Ctrl+Y in a Project tab route to it
(`handle_undo_requested`), and command enablement reads it.

**Format.** `ProjectData::description: Option<String>`,
`#[serde(default, skip_serializing_if = "Option::is_none")]`. An empty
description is stored as `None`, so clearing it removes the key and a file that
never had one re-saves byte-identical.

**Name.** The project name is the `.snxprj` file stem: `parse_project` overrides
the stored `name` with the stem, and the Rename dialog renames the file. So an
edited name is a **pending rename**, applied when the project is saved: the save
validates the stem (one rule, `project_stem_problem`, shared with the Rename
dialog), waits the ledger barrier (no older write may land on the old path after
the move, D2), renames the file, re-keys tabs / dirty marker / undocked windows /
engines through the one helper the Rename dialog also uses, then writes the
`.snxprj` at the new path. A refused rename is a save failure: Messages panel +
Save Failed card, the project stays dirty. Writing the name into the JSON
without the rename would lose it on the next open.

**Save.**
- Ctrl+S in a Project tab: `begin_project_save(id)`. On the UI thread it applies
  a pending rename, materialises pending libraries (unchanged behaviour), and
  encodes the bytes (`signex_types::project::encode_project`, split out of
  `write_project`); `DocumentSaves::begin` hands out the ledger-ordered job, which
  runs under `spawn_blocking`. The completion `ProjectMsg::FileSaved` frees the
  slot (`finish`), and clears the dirty marker only if the project with that id
  is still loaded at that path, has no pending library, and its current data
  encodes to exactly the bytes written. A failure, or a lost task (`JoinError`),
  reaches the Messages panel and is recorded for Save All (D5).
- The other `.snxprj` saves (Ctrl+S in a schematic / primitive tab when the
  project is dirty, Save All on close / quit) stay synchronous, as before, but now
  write through the same ledger (`begin(..).run()`), so a later synchronous write
  always wins over an earlier off-thread one, whatever order they finish in.
- Save All needs no new branch: the project's dirty marker is in `dirty_paths`.

Why bytes and not a revision counter: `ProjectData` has no mutation gateway yet;
every writer listed in §3 mutates it directly. A revision counter would miss every
writer that does not bump it. Equality of the persisted form cannot miss one,
and "memory encodes to what is on disk" is the definition of clean, not a
heuristic.

## 3. The `.snxprj` concurrent-writer rule

**Every writer edits only its own fields of the one in-memory document
(`LoadedProject::data`) and marks the project dirty; every save writes the
whole current document; nobody holds a copy of `ProjectData` across messages.**
Consequences:

1. The Project tab cannot clobber another writer: it has no draft to write back.
   Its undo snapshots hold only `name` / `description`, so undoing in the tab
   never reverts a variant, library, sheet, net class or rule change.
2. Another writer cannot clobber the tab: its change lands in the same `data`,
   and the next save writes both.
3. A save started before another writer's change writes the older snapshot; its
   completion sees that the current data no longer encodes to the written bytes
   and leaves the project dirty. The next save writes both changes.
4. A writer that writes the file **directly** (library rename's
   `retarget_library_entry`) first waits the ledger barrier, so an older
   off-thread write cannot land after it.

Writers on trunk (all follow the rule): Add Existing / Add New (sheets, PCB,
libraries), Remove from Project, file rename (`schematic_root` / `pcb_file`),
project rename (file move + `name`), Enable Version Control (`enable_git`), net
classes (#1042), library registration and recovery (`libraries`), pending-library
materialisation at save, library rename (`retarget_library_entry`, disk + data).
Variants and the ERC rule set (#1169) have no in-app writer yet; when their
sections land they follow the same rule, with their own field-scoped history.

## 4. ADR-0003 checklist for the off-thread project save

1. **Owed state:** `DocumentSaves` (in flight per ledger key); instance =
   `ProjectId`; "generation" = the encoded bytes (see above).
2. **Flush:** write-type. Barrier waits before the rename's file move and before
   `retarget_library_entry`; project close and quit already wait it.
3. **Undo snapshots:** General fields only, always finished (edits are
   synchronous).
4. **Late result:** id + path + exact bytes, else the marker stays.
5. **Lost result:** `JoinError` maps to `Err`, reported to Messages and recorded
   for Save All; the job's `InFlight` guard frees the barrier when dropped.
6. **One authority:** the stem rule and the project re-key are shared with the
   Rename dialog; the write goes through the one ledger.

## 5. Out of scope (later slices)

Parameters (#1080), Sheets (#1081 / #1082), Variants, Libraries, ERC and
annotation (#1085), Version control sections; a menu / command entry beyond the
existing Project Options item (menu content is not this slice's call); taking
the project rename's file move off the UI thread (it stays synchronous, as in
the Rename dialog); staging the old file name's removal in the project git
commit after a rename.
