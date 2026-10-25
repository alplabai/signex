# ADR-0003 — Deferred work: explicit owed state, one flush before boundaries, consistent history

- **Status:** Proposed (2026-10-25)
- **Scope:** `signex-app` — every piece of work that finishes after `update` returns (`Task::perform`, `spawn_blocking`, a `Subscription` producer, the document save queue), and the persistence, undo/redo and document-switch paths that can observe it
- **Extends:** ADR-0001 §A1 (`update` is the only mutation site and never blocks; IO is returned as a `Task`)
- **Trigger:** each move of work off the UI thread reopened the same data-loss class and each feature solved it locally: schematic saves (#997, #1008, tracked in #859), the PDF preview (#940), and the footprint sketch drag solve (#1047, branch `perf/1047-async-sketch-solve`, not merged). Review of #1047 found four holes in a row (see the appendix); each wrote wrong geometry to disk or marked a dirty document clean. Nothing in the codebase states the rules those fixes rely on.
- **Tracking:** #1087. Gaps found by the audit are filed as separate issues (see "Audit").

## Context

ADR-0001 says `update` never blocks, so IO and CPU work leave it as a `Task` and come back as a `Message`. That is the right shape, and it has one consequence the ADR did not spell out: between `update` returning and the result message arriving, **the Model is in a state that is not yet true**. A save has been started but the document is still marked dirty; a drag has moved the geometry but the solver has not answered; a project open has been requested but the tab does not exist.

Three kinds of code can run inside that window and must not be fooled by it:

- **Persistence** (Save, Save All, close-with-save, Save As, History restore, export) reads or writes the document.
- **Undo/redo** moves the document to a snapshot taken earlier.
- **Switches** (tab, footprint inside an editor, project close) replace what "the active document" means.

And one kind of code runs *after* the window: the **late result**, which must not land on a document it was not computed for.

Trunk already contains good local answers. They were written one at a time, in different shapes:

- Schematic saves (`persist/document_save.rs`, `handlers/document_files/save_async.rs`): a per-path ledger, a `SaveBarrier`, and a revision stamp checked when the completion arrives.
- `Engine::revision()` and `Engine::instance_id()` (`signex-engine/src/lib.rs:155`, `:172`): process-unique, bumped by undo and redo (`signex-engine/src/history.rs:64-83`).
- Generation counters on the ERC run, the Annotate preview, the History panel, the drift pass, the DigiKey flow, the project-tree probe and the print preview.
- `ProjectId` carried by the case probe and by the Add Existing / Add New completions, and re-resolved when the result lands.

What is missing is the contract: which state must exist, who flushes it, what an undo snapshot may contain, what a late result may touch, and what happens to a result that never arrives. This ADR states it. It makes no code change.

## Decisions

### D1. Owed work is explicit Model state

Any work that finishes after `update` returns is tracked **in the Model, per document**, as one of *in flight* or *owed* (wanted, not started), carrying a **generation** and a **document-instance id**. It is never only inside the `Task`: a `Task` is invisible to `update`, so a save, an undo or a switch cannot ask it anything.

Reference shapes on trunk:

- `DocumentSaves` (`persist/document_save.rs:132-138`): `saving: HashMap<PathBuf, usize>` is the in-flight count per path; `is_saving(path)` is the question a Save As asks (`handlers/document_files/save.rs:337`).
- `Engine::revision()` is the generation, `Engine::instance_id()` the instance. The save snapshot records the revision (`save_async.rs:32`), the completion compares it (`save_async.rs:197`).

The first adopter of the full shape is `SketchSolveSlot` (#1047, `library/editor/footprint/sketch_solve_task.rs`, unmerged). It lives in the footprint editor state, so it is per tab and gone with the tab:

- `instance: Uuid`, unique per editor state; an answer for another instance is never applied whatever its generation;
- `generation: u64`, bumped by every edit that moves geometry, every inline solve, every undo and redo;
- `in_flight: Option<u64>`, at most one running solve;
- `wanted: bool`, an edit is owed a solve that has not started;
- `unsolved: Option<UnsolvedEdits>`, what a rejected result must put back.

`is_solve_owed()` answers "is something owed?" in one call. This ADR does not mandate a shared trait yet: two shapes (a revision-stamped write and a solve slot) do not justify an abstraction (YAGNI). The vocabulary — instance, generation, in flight, owed, `finish_pending` — is what is shared. A shared type is reconsidered when a third adopter appears.

### D2. One flush hook before every boundary

Every boundary calls one per-document **`finish_pending()`** before it reads, writes, restores or replaces the document. It completes or cancels the owed work **synchronously**, with the same semantics as when the result arrives (so a rejected solve is rolled back exactly as the arriving rejection would be).

The boundaries:

1. **Persist**: Save, Save All, close-with-save, Save As, export of the document, History restore, the quit chain.
2. **Undo / redo**.
3. **Document, footprint or tab switch**, and project close.

Two flavours of `finish_pending`, chosen by what the work does:

- **Write-type work** (a file write already on a worker thread) cannot be cancelled; the flush *waits*. This is `DocumentSaves::barrier()` and `SaveBarrier::wait()` (`document_save.rs:73-88`, `:180`). Trunk calls it before every synchronous write that could be overtaken by an older one: `close_project.rs:283`, `rename.rs:137`, `remove.rs:123`, `document_files/history.rs:74`; the quit path chains `wait_for_document_saves_then` after the debounced preference writes (`search_prefs_persist.rs:56-62`).
- **Compute-type work** (a solve, a derivation) is *settled inline*: the flush runs the work on the UI thread and applies the result, then makes any task still running stale. This is `settle_pending_inline` (#1047, `updates/sketch/solved.rs`): up to three rounds, because a rejection owes one more solve of the sketch it restored, then `supersede()`.

Today each site hand-writes the first flavour (`barrier().wait()` five times, never behind a name). The decision is that a new boundary does not decide *whether* to flush, only calls the hook; and that the hook is one function per document kind, so adding an async producer means adding one line to it, not auditing every boundary.

### D3. Undo snapshots are always of finished states

An undo snapshot is taken only of a finished state. An async intermediate state is never recorded as its own step: intermediates collapse into **one undo step per committed user action**, recorded once the result settles.

Trunk's history types, for reference:

- Schematic: `signex_engine::history::HistoryEntry { snapshot: SchematicSheet, patch_pair, group }`, `MAX_HISTORY_ENTRIES = 100` (`signex-engine/src/history.rs:8-17`). Edits apply synchronously, so every snapshot is finished by construction; undo and redo bump the revision.
- Footprint editor: `FootprintHistorySnapshot` (`app/documents.rs:503`), `FootprintEditorState::push_history` / `commit_history` / `undo` / `redo`, `HISTORY_DEPTH = 100` (`documents.rs:600-699`).
- Symbol editor: `SymbolHistorySnapshot` (`app/symbol_history.rs:20`), `HISTORY_DEPTH = 100`.

The editors' histories are where D3 bites, because a solve can be owed when a snapshot is taken. #1047's answer, which this ADR adopts as the pattern:

- `begin_edit` and `push_history` call `settle_pending_solve()` first, so the snapshot is of a solved sketch (`documents.rs`, #1047 branch). A drag tick uses `begin_tick_edit`, which deliberately does not settle: it owes its own solve.
- A drag's ticks each push a step, and `UnsolvedEdits` (`history_steps`, `history_len_before`/`_after`, `evicted`, `redo_before`, `dirty_before`) records what the ticks did to history, the redo stack and the dirty flag, so a **rejected** result can take them back and restore the pre-drag history exactly, including steps a full history evicted.
- Undo or redo into a snapshot that is not known to be solved calls `owe_solve()`: the restored sketch is owed a solve, and whoever next reads or replaces it settles it first.

The rule for the next adopter: if a snapshot can capture an unfinished state, either settle before taking it, or mark the restored state as owed on the way back.

### D4. Late results apply only to the same instance and generation

A result applies only if its **instance and generation are still current**. Applying it sets the dirty state **exactly as the equivalent synchronous edit would**, and the dirty mirrors are updated in **one place**.

Examples of the check on trunk:

- Save completion: `mark_saved` returns without clearing dirty unless the engine still exists at `path` and its revision and net-colour revision equal the snapshot's (`save_async.rs:187-206`). Edit, undo, redo or a History restore while the write ran all leave the document dirty.
- Library update from the drift modal: compares `engine_instance` with `Engine::instance_id()` and skips a sheet that was closed or reloaded (`dispatch/library/updates.rs:364-380`).
- Place from library: the target sheet is carried in the message and re-checked against the active tab and `engines` (`handlers/library_place.rs:105-126`); the placement then goes through the engine as one undoable command.
- #1047: `apply_solved` drops an answer whose `instance`, `generation` or `footprint_idx` is not current (`updates/sketch/solved.rs`).

On "one place": the dirty state of a document has three mirrors — the editor's own `dirty` flag, `DocumentState::dirty_paths`, and `TabInfo::dirty`. `sync_editor_dirty_paths` (`runtime/primitive_dirty.rs:39`) is already the one-way reconciliation for editor edits, but 50 non-test `dirty_paths.insert` / `.remove` calls in 21 files still write the mirror directly. The rule: a late result, and every other edit path, goes through one function that sets or clears all three; the direct calls migrate to it over time. The audit files that migration as an issue.

### D5. Lost results are failures

A result can be lost: the worker panics, the `Task` is dropped (the window closed, an abortable task was superseded), or the runtime shuts down. A lost result is mapped to a **failure message that reaches the Messages panel** and **frees the in-flight slot**. A dialog status string is not reporting, and a slot that stays "in flight" forever turns one lost result into a feature that never runs again.

Trunk does the first half everywhere that matters for documents: every `spawn_blocking` is followed by an `unwrap_or_else` that turns a `JoinError` into the completion's `Err` (`spawn_document_write`, `save_async.rs:106-109`; `open.rs:426-428`, `:485-487`; `git.rs:143`; `library_place.rs:79`; `add_io.rs:8-14`). Reclaiming a slot whose message never comes is done three different ways: a `Drop` guard (`DocumentSaveJob::drop` decrements `in_flight`, `document_save.rs:121-129`; `StatRunning::drop`, `library/external_poll.rs:93-101`), a deadline (`ProjectFsCache::begin_probe` reclaims a probe older than `PROBE_DEADLINE`, `state/project_fs.rs:146-159`), and a generation bump that makes the old answer stale (`SketchSolveSlot::supersede`, DigiKey `DigiKeyCancel`).

#1047 adds the compute case: `run_solve_with` wraps the solver in `catch_unwind` so a panic is a `SolveFailed` answer rather than a message that never comes, `run_solve_blocking` turns a `JoinError` into the same, and `finish_flight` frees the slot whichever way the answer arrives.

### D6. One authority per cross-cutting decision

When two code paths must agree on a decision, they share one implementation. A second implementation is a defect even when both are correct today: the first change to one of them is the divergence. ("Kept in step" comments are banned by ADR-0002 for the same reason.)

Two live cases, one resolved and one not:

- **Finite-number checks (#828, #1018): one authority.** `signex_types::numeric` holds `parse_finite_f64`, `parse_toml_finite` and `first_non_finite_path` (`numeric.rs:22`, `:104`, `:124`); the two branches that grew a walker each converged on it, and `format/finite_toml.rs` and `library/primitive/footprint/mod.rs:743` call the same one.
- **Path case folding (#937, #1034): not yet one authority.** The key is `path_key_with(path, CaseFold)` with the fold chosen per project from a volume probe (`state/scope.rs:40`, `state/case_fold.rs:15`, `state/project_case.rs:86`). The Discard-check reach sets in `state/sheet_refs.rs:168`, `:187`, `:198` still fold with `to_lowercase()` and the host-family answer, which is #1034. The one sanctioned difference, `volume_case`'s identity probe answering "ignores case" when it cannot decide, is documented at its site (`state/project_case.rs:8-12`) because it picks the data-loss-safe side; a deviation needs that kind of stated reason.

## How the rules compose: a checklist for new deferred work

A change that adds `Task::perform`, `spawn_blocking` or a subscription producer that later mutates the Model states, in its description or module docs:

1. Where the owed/in-flight state lives, and its instance and generation (D1).
2. Which boundaries call `finish_pending` for it, and whether it waits or settles inline (D2).
3. What an undo snapshot taken during or after the work contains (D3).
4. What the result checks before applying, and how it sets dirty (D4).
5. What a panic or a dropped task produces, where it is reported, and what frees the slot (D5).
6. Which existing helper it reuses for any decision another path already makes (D6).

Reviewers use the same list.

## Anti-patterns (extends ADR-0001 Part C and ADR-0002)

- **No state that exists only inside a `Task`.** If `update` cannot ask "is it still running?", the boundaries cannot flush it.
- **No `Task::perform` whose completion mutates a document and checks only the path.** A path is not an instance: a document closed and reopened under the same path is a different document. Carry the instance or revision.
- **No result applied "if nothing seems to have changed".** Compare the generation; a heuristic on content passes for the wrong edit.
- **No snapshot of an unfinished state.** Settle first, or mark the restored state owed.
- **No swallowed join error.** `.ok()` on a `JoinError` loses the one signal that a result is gone.
- **No second case-fold, finite-check or path-key helper.** Extend the existing one.

## Audit of current trunk (2026-10-25, trunk `0d011e5b`)

Every place where work finishes after `update` returns, found by searching `Task::perform`, `Task::future`, `spawn_blocking`, `tokio::spawn`, `std::thread::spawn`, `Subscription` producers and the document save queue under `crates/`. The columns are the four questions asked of each site. "Unfinished" means: can Save, undo or a switch observe the owed work unfinished. "Stale" means: can a late result land on a newer or different document. "Lost" means: is a lost result reported and its slot freed.

Verdicts: **C** compliant, **G** gap (numbered, concrete scenario below), **N/A** read-only or a pure user-choice fetch.

| # | Site | Mutates on arrival | Unfinished observable? | Stale lands? | Lost reported? | Verdict |
|---|------|--------------------|------------------------|--------------|----------------|---------|
| 1 | Schematic save: `save_async.rs:97-121`, `handle_document_saved` `:126` | clears dirty, re-keys on Save As, queues git commit | no: sync writes `barrier().wait()` first; Save As refuses while `is_saving` (`save.rs:337`) | no: revision and net-colour revision compared (`:197`, `:202`); Save As re-checks active tab (`:250`) | yes: join error mapped to `Err` (`:108`), `finish` frees the slot first (`:127`), `report_save_failure` | C |
| 2 | PCB save: `begin_pcb_save` `save_async.rs:57-95` | clears dirty unconditionally on completion | no (barrier) | **yes**: revision is the constant `0` (`:94`, `:156-159`) | yes | G1 (already filed as #1040) |
| 3 | Quit chain: `wait_for_document_saves_then` `save_async.rs:169`, `flush_quit_writes_then` `search_prefs_persist.rs:58` | none | n/a (it is the flush) | n/a | yes: `DocumentSaveWaitFailed` logged (`dispatch/document.rs:116`) | C |
| 4 | Sync flushes before writes: `close_project.rs:283`, `rename.rs:137`, `remove.rs:123`, `document_files/history.rs:74` | none (they wait) | no | no | n/a | C (note: block the UI thread, bounded; this is the D2 flavour for write-type work) |
| 5 | Open schematic / PCB: `open.rs:417-438`, `:477-497`; completion `dispatch/document.rs:133-160`, `open_schematic_tab` `load_gateway.rs:323` | pushes a tab, inserts the engine | no | **yes**: only `pending_opens` guards a duplicate open, not Save As | yes: `report_open_failure` | G2 |
| 6 | New Project / sample project: `open.rs:60-66`, `:145-156` | opens the project and its sheet via the sync open path | no | `pending_opens` per path | yes (`report_new_project_failure`, `report_sample_failure`) | C |
| 7 | Project close ref resolution: `close_project.rs:42-54`, `handle_close_refs_resolved` `:76` | opens the Close gate | no | no: `refs_are_current` re-validates against the live workspace and re-spawns (`:106-108`); project looked up by `ProjectId` | yes (`log_error`, close cancelled) | C |
| 8 | ERC run: `erc_run.rs:17-32`, `apply_erc_run_result` `:98` | `erc_violations_by_path`, markers | n/a (derived, display only) | superseded runs dropped by `erc_run_generation` (`:103`); a result for an edited sheet is shown until the next run | yes (`log_warning`), `erc_running` cleared | C |
| 9 | Annotate preview: `annotate_preview.rs:343-391` | preview cache | n/a (preview) | generation + fingerprint (`:331`, `:369`) | yes; failure clears the fingerprint so the next refresh retries | C |
| 10 | History panel load: `runtime/history.rs:90-125` | panel entries | n/a (display) | generation + path | yes (`Err` mapped) | C |
| 11 | Git commit after save: `git.rs:98-155`, `handle_project_git_commit_done` `:162` | none on the model; commits the working tree | **yes**: History restore does not wait for it | n/a | yes (`log_warning`), repo slot freed (`:171`) | G3 |
| 12 | Project-tree filesystem probe: `handlers/project_fs.rs:11-39`, `ProjectFsCache::finish_probe` `state/project_fs.rs:166` | tree facts | n/a | generation (`:167`); a probe past `PROBE_DEADLINE` is reclaimed and its late answer dropped | **no**: `.ok()` discards the join error (`project_fs.rs:17`), `None` is silent (`:171-177`) | G4 |
| 13 | Volume case probe: `state/project_case.rs:42-59`, `apply_case_probe` `:98` | the project's `case_fold` | n/a | keyed by `ProjectId`; a closed project is ignored; first answer wins | yes: undecided and failed outcomes warn | C |
| 14 | External-edit poll: `external_edits.rs:24-39`, `handle_external_edits_polled` `:45`; `external_poll.rs:105-121`; the `iced::time::every` subscription | refreshes library caches | n/a (caches) | observed stamp compared (`:86`); a library no longer mounted is skipped (`:81`) | yes: timeout and failure warn once, `StatRunning` guard frees the flag on panic | C |
| 15 | Library mount: `mount.rs:276-281`, `handle_mount_finished` `lifecycle.rs:63` | mounts the library | n/a | intent taken per path; closed-while-preparing result discarded | yes: `log_error`, error card for an explicit open | C |
| 16 | Library drift pass: `library_drift.rs:28-48` | drift report | n/a (display) | generation | yes (`problems`) | C |
| 17 | Library update apply: `updates.rs:327-342`, `handle_library_update_symbols_read` `:351` | adopts library symbols into the sheet through the engine | no | no: `engine_instance` compared (`:364-368`) | yes (`SymbolsRead::failed`, warnings) | C |
| 18 | Place from library: `library_place.rs:73-89`, `place_resolved` `:105` | places a symbol through the engine as one undo step | no | no: target sheet re-checked against the active tab and `engines` (`:118-126`) | yes (`report_place_failure`) | C |
| 19 | Add New schematic / Add Existing / New library file: `add.rs:195-209`, `:251-267`, `registration.rs:344-358` | registers a file on the project, marks `.snxprj` dirty | no | no: project re-resolved by `ProjectId` (`add.rs:224`); a project closed meanwhile is skipped | yes (`log_error`) | C |
| 20 | Export writes: `export/write.rs:15-40`, `handle_export_written` `:65` | none on the model (writes a file; may raise a notice) | n/a | n/a | yes (`ErrorNotice::export`) | C |
| 21 | PDF / netlist / BOM / ERC report dialogs: `pdf_netlist.rs:126-157`, `bom.rs:252`, `erc_report.rs:71`, `:125`, `print_preview.rs:287` | none (fetch a path); the export is then built synchronously from the live engine | no | no: the source document is pinned in the message and re-derived; closed source refused (`pdf_netlist.rs:172-182`) | yes | C |
| 22 | Print preview raster: `raster_job.rs:42-72`, `handle_print_preview_rasterized` `:98` | preview pages | n/a | generation; superseded and queued jobs aborted | **partly**: a panicking worker becomes an empty page list (`:65-68`), shown as "Preview has no pages for the selected range" (`:111-116`), which names the wrong cause | G5 (low) |
| 23 | Distributor settings (DigiKey OAuth, Mouser test): `dispatch/library/settings.rs:42-76`, `:138-176` | settings status, keyring write | n/a | DigiKey: generation (`:103`); Mouser: `mouser_in_flight` only | failure shown only in the settings panel status string (`:114-116`, `:177-182`), not the Messages panel | G6 (low) |
| 24 | Dock layout and search-box persistence: `layout_persist.rs:17-89`, `search_prefs_persist.rs:44-110`, `persist/debounced.rs` | none | n/a | generation (`take_if_current`) | yes (`log_error`); flushed on quit | C |
| 25 | Folder / file / save-as dialogs (rfd): `lifecycle.rs:12`, `components_panel.rs:38`, `recovery.rs:86`, `primitive_picker.rs:57`, `registration.rs:132`, `document_files/mod.rs:75`, `file_commands.rs` | none (fetch a user choice); the result runs a synchronous handler | n/a | the handler re-resolves its target | n/a | N/A |
| 26 | `open_link` (`about.rs:53`), `Task::perform(async {}, ..)` re-dispatch (`project_navigation/mod.rs:210`) | none | n/a | n/a | yes (`on_failed`) | N/A |
| 27 | Subscription producers (`bootstrap/subscription.rs:694`): keyboard, mouse, window events, hover ticks, the library poll tick | messages only (ticks map to `Noop` or to #14) | n/a | n/a | n/a | N/A |
| 28 | Footprint sketch solve (#1047) | not on trunk: trunk solves inline, so there is nothing owed | n/a | n/a | n/a | first adopter, see below |
| 29 | `signex-library-server` `tokio::spawn` (`lib.rs:131`, `:227`), distributor HTTP and throttle threads, importer parser threads | server and adapter internals, never the Model | n/a | n/a | n/a | N/A |

Not listed: `std::thread::spawn` inside `#[cfg(test)]` code (`persist/document_save.rs:256`, `persist/debounced.rs:358`, `close_project.rs:715`, `fonts/prefs_file.rs`), which exists to exercise the barriers.

### Gaps

- **G1 — PCB save has no revision (#1040, already filed).** `begin_pcb_save` passes revision `0` and `finish_clean_save` clears the dirty flag unconditionally (`save_async.rs:94`, `:156-159`). Today no PCB edit path exists, so the hole is latent; the first PCB edit makes a change made during the write read as saved, and the second tab-close then discards it. D1 and D4 require a board revision.
- **G2 — Save As onto a path whose open is still in flight.** Open of `P.snxsch` is pending (`pending_opens` holds `P`, `open.rs:417`). The user runs Save As on another sheet and picks `P`: `save_as_target_is_other_open_document` checks only `engines` and `tabs` (`save.rs:350-354`), both empty for `P`, so the write proceeds and `apply_saved_as` re-keys the engine to `P`. When the open finishes, `open_schematic_tab` pushes a second tab for `P` that starts clean (`load_gateway.rs:323-343`) and `apply_loaded_schematic` replaces the engine stored at `P` with the disk content (`sync_engine_from_schematic`, `load_gateway.rs:156-165`, `:525`), so any edit made after the Save As snapshot is gone and the two tabs disagree about the document. The other order is safe (`apply_saved_as` refuses). D4: the open result must be checked against the document that now owns the path.
- **G3 — History restore does not wait for queued git commits.** A save queues a commit that stages the working tree when its worker runs (`git.rs:98-125`; `stage_and_commit`, `signex-library/src/adapters/local_git/project.rs:137-139`). History restore waits only for document writes (`history.rs:74`). Save, then restore an older revision before the commit worker starts: the commit records the restored content under the message `Save <file>`. No data is lost (the restored blob is the old history), but history now misattributes an unsaved restore to a save. D2: the flush before a restore covers every writer of the same file, not only the document ledger.
- **G4 — Project-tree probe loses its failure silently.** A panicking probe worker becomes `None` through `.ok()` (`handlers/project_fs.rs:15-17`) and `finish_probe` treats it as "facts unchanged" (`state/project_fs.rs:171-177`). The slot is freed and the next poll retries, so the feature recovers, but a deterministic worker panic never reaches the Messages panel and the tree's `(missing)` marks stay stale without explanation. D5.
- **G5 (low) — A panicking print-preview render reports the wrong cause.** The join error becomes an empty page list (`raster_job.rs:65-68`), displayed as a page-range problem (`:111-116`). The slot is correctly freed; the report is misleading. D5: carry the failure.
- **G6 (low) — Distributor settings failures stay in the dialog.** DigiKey and Mouser failures are written to a status string only (`settings.rs:114-116`, `:177-182`). The Messages panel is where a failure must land (D5); the Mouser keychain failure is already routed visibly by #976 but its test result is not.
- **G7 — Case folding has a second implementation (#1034, already filed).** `sheet_refs.rs:168`, `:187`, `:198` fold with `to_lowercase()` while every other consumer uses the probed `CaseFold`. D6.
- **G8 — Dirty mirrors are written from 21 files.** 50 non-test `dirty_paths.insert` / `.remove` calls write one of the three mirrors directly (`primitive_dirty.rs` documents the reconciliation but is one-way). D4 "one place". No scenario of lost data is claimed here; it is the structural debt that makes G1 and G2 possible, and the first place a late result will set only two of the three.

### First adopter: #1047

Trunk solves the footprint sketch inline, so row 28 has no gap today. #1047 is the first change that creates owed compute work, and it implements the rules this ADR records, which is where they were found:

- D1: `SketchSolveSlot` (`instance`, `generation`, `in_flight`, `wanted`, `unsolved`), `is_solve_owed`, `may_be_unsolved`.
- D2: `settle_pending_inline` is called by `push_history`, `begin_edit`, `record_edit`, footprint save (`dispatch/library/editor.rs`), `editing_commands.rs`, and the geometry and selection updates; it runs up to three rounds and then `supersede()`s.
- D3: ticks push history, `UnsolvedEdits` takes them back on a rejection; undo and redo `owe_solve()`.
- D4: `apply_solved` drops a result with another `instance`, another `generation` or another `footprint_idx`.
- D5: `catch_unwind` in `run_solve_with`, the join error mapped in `run_solve_blocking`, `finish_flight` freeing the slot.

When #1047 merges, the "first adopter" paragraph becomes a row in the table and the line references move.

## Consequences

- **Cost of a new async producer rises, by design.** The six-point checklist is the price; the four holes found in #1047's review were each a missing answer to one of its questions.
- **`finish_pending` is a one-function-per-document-kind hook.** It does not need a trait; the schematic one is `DocumentSaves::barrier()` plus the revision stamp, the footprint one is `settle_pending_inline`. Shared extraction waits for a third adopter.
- **Synchronous flushes block `update` for a bounded time.** Trunk accepts this today (`barrier().wait()`); the save write is short and a settle runs at most three solves. The alternative, a chained `Task`, is what the quit path uses (`flush_quit_writes_then`) because exit has no later `update` to be blocked in.
- **Out of scope here:** moving the remaining synchronous IO out of `update` (#859), the per-instance annotation behind `SharedReferenceAcrossInstances`, and a board revision (#1040 tracks it; G1 only records that D1 and D4 depend on it).

## Appendix — the evidence this ADR generalises

Review of the #1047 branch (head `dc83be2e`), four defects found one after another, each fixed by a rule now stated above:

| # | Finding | Rule |
|---|---------|------|
| 1 | A save while a solve was owed wrote the pads of the previous solve and cleared the dirty flag under it | D2: settle before persist (`dispatch/library/editor.rs`, the footprint save leg) |
| 2 | Undo into a mid-drag snapshot restored an unsolved sketch; a save then wrote it | D3: `owe_solve()` on undo and redo |
| 3 | A long rejected drag left the rejected geometry's ticks in undo history | D3: `UnsolvedEdits` takes back the ticks, the evicted steps and the redo stack |
| 4 | A drag through a conflict and back, then undo, restored the over-constrained state | D3 and D4: the restored snapshot is owed a solve and a stale answer is dropped |

Trunk's own precedent, for the same class:

| Issue | What it fixed | Rule |
|-------|---------------|------|
| #997 | Schematic saves leave the UI thread; `DocumentSaves`, `SaveBarrier`, per-path sequence ledger so an older write never lands after a newer one | D1, D2 |
| #1008 | Exports and New Project files leave the UI thread; `write_export_task` | D2, D5 |
| #940 | The PDF preview renders off the UI thread with a generation and abortable jobs | D4 |
| #1034 | Case folding in the Discard-check reach sets disagrees with the probed fold | D6 |
| #828 / #1018 | Finite-number checks converged on `signex_types::numeric` | D6 |

## References

- ADR-0001 — App structure, MVU, and domain boundaries (§A1 `update` and `Task`; Part C anti-patterns)
- ADR-0002 — Netlist single derivation ("kept in step is not a synchronization mechanism")
- Iced 0.14 — `Task`, `Task::abortable`, `Subscription`: <https://docs.iced.rs>
- Issues: #859, #997, #1008, #940, #1047, #1040, #1034, #937, #828, #1018, #1087
