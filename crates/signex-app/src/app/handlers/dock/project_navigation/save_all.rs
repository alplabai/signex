//! Save All for the project-close and app-quit prompts (#859).
//!
//! Every document write runs off the UI thread through the same task and
//! completion message as Ctrl+S, so dirty tracking, failure reporting and
//! the project git commit are the single-document ones. When all of them
//! have completed, a [`SaveAllSettle`] message decides: anything still
//! dirty did not save and keeps the project (or Signex) open with the
//! reason; otherwise the close / quit goes ahead.

use super::*;
use crate::app::FileMsg;
use crate::app::contracts::{
    CaseVariants, PinnedSpelling, SaveAllNext, SaveAllPlan, SaveAllSettle, SaveAllWrite,
};
use crate::app::document::DocumentKey;
use crate::app::handlers::document_files::FoldAnswer;
use crate::app::state::project_case::CaseProbeOutcome;
use crate::app::state::scope::path_key_with;
use crate::app::state::volume_case::probe_directory_case;
use signex_i18n::RenderTranslatable;
use std::collections::HashMap;

/// The reason Save All records for a spelling it kept unsaved.
const CASE_RULE_UNKNOWN_REASON: &str = "folder case rule unknown, save it separately";

/// The reason Save All gives for a row whose write stopped without a result.
const WRITE_STOPPED_REASON: &str = "its save stopped unexpectedly; save it again";

impl Signex {
    /// Start the write of one dirty file, matching `path` to the spelling it
    /// is held under by the volume rule `fold`; see [`Self::begin_held_save`].
    #[cfg(test)]
    pub(super) fn begin_dirty_path_save_with(
        &mut self,
        path: &std::path::Path,
        fold: CaseFold,
    ) -> anyhow::Result<Option<Task<Message>>> {
        let held = self.held_spellings(path, fold);
        let target = self.write_target(&held).clone();
        self.begin_held_save(&held, &target)
    }

    /// Write one file held under `held` spellings (matched by a volume rule):
    /// refuse differing variants, then write `target` once.
    ///
    /// The document maps are keyed by the exact spelling a document was
    /// opened under, while the requested path may match a held spelling only
    /// under the volume rule. On a folding host `Lib.snxsch` and `lib.snxsch`
    /// are ONE file, so two documents held under both spellings cannot both
    /// be written: the second write would silently replace the first's edits
    /// (#1077). The write is refused unless every variant is an engine
    /// encoding to identical bytes, and then the first dirty spelling is
    /// written once. The other spellings' dirty markers are cleared when the
    /// Save All settles ([`Self::clear_saved_case_variants`]).
    ///
    /// The task returned must be output-only: a future whose one action is
    /// its completion message. Save All runs it under a panic guard
    /// ([`writes_then_settle`]) that drops any other runtime action with a
    /// warning.
    fn begin_held_save(
        &mut self,
        held: &[std::path::PathBuf],
        target: &std::path::Path,
    ) -> anyhow::Result<Option<Task<Message>>> {
        if held.len() > 1 {
            self.ensure_variants_agree(held)?;
        }
        self.begin_held_path_save(target)
    }

    /// The spelling to write among the `held` ones of a file: the first
    /// dirty one, else the first. Writing a clean spelling would leave the
    /// edited one's work unsaved (#1423).
    pub(super) fn write_target<'a>(
        &self,
        held: &'a [std::path::PathBuf],
    ) -> &'a std::path::PathBuf {
        held.iter()
            .find(|path| self.document_state.is_document_dirty(path))
            .unwrap_or(&held[0])
    }

    /// Start the write of one dirty path, routing by the kind of document
    /// it backs:
    ///
    /// * a live schematic **engine** → an off-thread schematic save;
    /// * a **PCB** tab, or a board parked when its dirty tab closed → an
    ///   off-thread board save;
    /// * an open **symbol/footprint primitive editor** → an off-thread
    ///   container save;
    /// * the project's own **`.snxprj`** → an off-thread project save, which
    ///   creates pending libraries first;
    /// * a project's **Design Notebook** (`.snxnbk`), held with or without
    ///   its tab → an off-thread notebook save;
    /// * a **design report** Generate made in the app → an off-thread write
    ///   of it;
    /// * an open **block diagram** (`.snxblk`), held by its slice → an
    ///   off-thread diagram save.
    ///
    /// A dirty `.snxsym`/`.snxfpt` draft or `.snxprj` has no engine, so a
    /// loop that only knew engines would fail on them (#104). `Ok(None)`
    /// means there was nothing to start. The error says WHY a file
    /// cannot be saved.
    fn begin_held_path_save(
        &mut self,
        path: &std::path::Path,
    ) -> anyhow::Result<Option<Task<Message>>> {
        // Reported as this file's failure; the rest of the batch still saves.
        self.refuse_save_while_restoring(path)?;
        // A block diagram is held by its slice (mockup row 171).
        if self
            .document_state
            .block_diagrams
            .editors
            .contains_key(path)
        {
            return self
                .begin_block_diagram_save(path)
                .map(Some)
                .map_err(|refusal| anyhow::anyhow!(refusal.english()));
        }
        if self.notebook_at_path(path).is_some() {
            return self.begin_notebook_save_at(path).map(Some);
        }
        if self.generated_report_at(path).is_some() {
            return self.begin_generated_report_save_at(path).map(Some);
        }
        match self.document_state.document_key_for_path(path) {
            Some(DocumentKey::Schematic(_)) => self
                .begin_schematic_save(path.to_path_buf(), None)
                .map(Some),
            Some(DocumentKey::Symbol(_) | DocumentKey::Footprint(_)) => {
                self.save_primitive_tab_at(path).map(Some)
            }
            Some(DocumentKey::Project(_)) => self.save_project_at_path(path).map(Some),
            Some(DocumentKey::Pcb(_)) => self.begin_pcb_save(path.to_path_buf()),
            Some(DocumentKey::Row(_)) | None => anyhow::bail!(
                "no open editor, engine, or project backs this file — it can't be saved automatically"
            ),
        }
    }

    /// Start the Save-All pass. A loose document (no open project) is keyed
    /// by a probe of its own directory, which reads the filesystem, so the
    /// probes run off-thread and the writes start when
    /// [`FileMsg::SaveAllProbed`] arrives; with none to probe they start now.
    pub(crate) fn begin_save_all(
        &mut self,
        context: &str,
        paths: &[std::path::PathBuf],
        rows: &[crate::library::state::EditorAddress],
        next: SaveAllNext,
    ) -> Task<Message> {
        // Save All and every save before a close: part fields that do not
        // evaluate are named in Messages, as Ctrl+S names them (#1986).
        for path in paths {
            self.report_part_field_issues(path);
        }
        let dirs = self.loose_parent_dirs(paths);
        let plan = SaveAllPlan {
            context: context.to_string(),
            paths: paths.to_vec(),
            rows: rows.to_vec(),
            next,
            probes: HashMap::new(),
            undecided: Vec::new(),
        };
        if dirs.is_empty() {
            return self.begin_save_all_planned(plan);
        }
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || probe_dirs(dirs))
                    .await
                    .unwrap_or_default()
            },
            move |(probes, undecided)| {
                Message::File(FileMsg::SaveAllProbed(Box::new(SaveAllPlan {
                    probes,
                    undecided,
                    ..plan
                })))
            },
        )
    }

    /// The distinct parent directories of the paths no open project owns.
    pub(super) fn loose_parent_dirs(
        &self,
        paths: &[std::path::PathBuf],
    ) -> Vec<std::path::PathBuf> {
        let mut dirs: Vec<_> = paths
            .iter()
            .filter(|path| self.project_owning(path).is_none())
            .filter_map(|path| path.parent())
            .filter(|dir| !dir.as_os_str().is_empty())
            .map(std::path::Path::to_path_buf)
            .collect();
        dirs.sort();
        dirs.dedup();
        dirs
    }

    /// Start the writes of a [`SaveAllPlan`] whose volumes are probed.
    /// Failures to even start a write, and each write's own failure, are
    /// logged to the Messages panel. The returned task ends with
    /// [`SaveAllSettle`], which continues into `next`.
    pub(crate) fn begin_save_all_planned(&mut self, plan: SaveAllPlan) -> Task<Message> {
        let SaveAllPlan {
            context,
            paths,
            rows,
            next,
            probes,
            undecided,
        } = plan;
        self.record_save_all_probes(&probes, &undecided);
        let (context, paths, rows) = (context.as_str(), &paths[..], &rows[..]);
        let mut writes = Vec::new();
        // One file reached under several spellings is written once.
        let mut begun = std::collections::HashSet::new();
        let mut case_variants = Vec::new();
        // The case-spelling reason an earlier pass left must not outlive it:
        // this pass records its own. Any other reason (say, a Ctrl+S that
        // failed) is not Save All's to clear.
        for path in paths {
            let saves = &mut self.document_state.document_saves;
            if saves.failure(path) == Some(CASE_RULE_UNKNOWN_REASON) {
                saves.clear_failure(path);
            }
        }
        for path in paths {
            let FoldAnswer { fold, decided } = self.identity_decision(path);
            if !begun.insert(path_key_with(path, fold)) {
                continue;
            }
            let held = self.held_spellings(path, fold);
            let target = self.write_target(&held).clone();
            match self.begin_held_save(&held, &target) {
                Ok(task) => {
                    if let Some(task) = task {
                        let write = SaveAllWrite::File {
                            path: target.clone(),
                            generation: self.primitive_save_generation(&target),
                        };
                        #[cfg(test)]
                        let task = stop_injected(&target, task);
                        writes.push((write, task));
                    }
                    // Only a write that began can make the other spellings
                    // clean; a refused one leaves every marker (#1423). Each
                    // other is pinned now: one edited while the write runs
                    // is not on disk and stays dirty.
                    if held.len() > 1 && !decided {
                        // The spellings may be two files and only `target`
                        // was written: the others stay dirty, and the
                        // settle step names them.
                        if let Some((kept, message)) = self.unsaved_spellings(&held, &target) {
                            crate::diagnostics::log_warning(message);
                            for path in kept {
                                self.document_state
                                    .document_saves
                                    .record_failure(path, CASE_RULE_UNKNOWN_REASON.into());
                            }
                        }
                    } else if held.len() > 1 {
                        let others = held
                            .iter()
                            .filter(|p| **p != target)
                            .map(|p| self.pin_spelling(p))
                            .collect();
                        case_variants.push(CaseVariants {
                            written: target,
                            others,
                        });
                    }
                }
                Err(err) => {
                    crate::diagnostics::log_error(
                        &format!("{context}: could not save {}", path.display()),
                        &err,
                    );
                    crate::app::handlers::document_files::log_foreign_column_offenders(&err);
                    self.document_state
                        .document_saves
                        .record_failure(path.clone(), format!("{err:#}"));
                }
            }
        }
        let mut failed = Vec::new();
        let batch = self.document_state.document_saves.mint_generation();
        // A row that pins staged PDFs stores them on a worker first; its
        // outcome reaches the settle step through `batch_row_failures`.
        for address in rows {
            match self.begin_save_row_in_batch(address, batch) {
                Ok(task) => writes.extend(task.map(|t| (SaveAllWrite::Row(address.clone()), t))),
                Err(err) => {
                    let label = self.library.editor_label(address);
                    crate::diagnostics::log_error(
                        &format!("{context}: could not save {label}"),
                        &err,
                    );
                    failed.push((label, format!("{err:#}")));
                }
            }
        }
        let settle = SaveAllSettle {
            next,
            items: begun.len() + rows.len(),
            paths: paths.to_vec(),
            failed,
            rows: rows.to_vec(),
            batch,
            case_variants,
            stopped: Vec::new(),
        };
        writes_then_settle(writes, settle)
    }

    /// Every write of a Save All completed: fail the ones that stopped
    /// without a result, then continue into the close / quit, or stay open
    /// and name what did not save.
    pub(crate) fn handle_save_all_settled(&mut self, mut settle: SaveAllSettle) -> Task<Message> {
        let stopped = self.settle_stopped_writes(&mut settle);
        Task::batch([stopped, self.continue_save_all(settle)])
    }

    /// The settle once no write of it is owed: park it while a row of its
    /// batch still writes, else close / quit or name what did not save.
    fn continue_save_all(&mut self, settle: SaveAllSettle) -> Task<Message> {
        if self.library.batch_has_rows_in_flight(settle.batch) {
            let waiting = self.library.batch_rows_in_flight_labels(settle.batch);
            crate::diagnostics::log_warning(format!(
                "Save All: waiting for {waiting} to finish saving before it continues"
            ));
            self.document_state
                .parked_save_all_settles
                .insert(settle.batch, settle);
            return Task::none();
        }
        let SaveAllSettle {
            next,
            items,
            paths,
            failed: rows_failed,
            rows,
            batch,
            case_variants,
            stopped: _,
        } = settle;
        self.clear_saved_case_variants(&case_variants);
        // Drop the dirty dots of everything that DID save.
        self.refresh_panel_ctx();
        let mut failed: Vec<(String, String)> = Vec::new();
        for path in paths
            .iter()
            .filter(|path| self.document_state.is_document_dirty(path))
        {
            let reason = self
                .document_state
                .document_saves
                .failure(path)
                .unwrap_or("it changed while it was being saved, or a save of it is still running")
                .to_string();
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("(unnamed file)")
                .to_string();
            failed.push((name, reason));
        }
        failed.extend(rows_failed);
        failed.extend(rows.iter().filter_map(|address| {
            self.library
                .batch_row_failures
                .remove(&(batch, address.clone()))
        }));
        if failed.is_empty() {
            // Everything saved, so no recovery copy of it is worth
            // keeping: remove them before the close / quit takes over.
            let sweep = self.autosave_sweep();
            return match next {
                SaveAllNext::CloseProject {
                    project_id,
                    sheet_refs,
                    kept_paths,
                } => Task::batch([
                    sweep,
                    self.close_project_after_save_all(project_id, *sheet_refs, &kept_paths),
                ]),
                // The UI stayed live during the writes, so anything edited
                // meanwhile is dirty now; the quit gate prompts for it.
                SaveAllNext::Quit => Task::batch([sweep, self.handle_app_quit_requested()]),
                SaveAllNext::SwitchBranch(request) => {
                    Task::batch([sweep, self.resume_branch_switch(*request)])
                }
                SaveAllNext::Nothing => {
                    crate::diagnostics::log_info(format!("Save All: saved {} item(s)", items));
                    sweep
                }
                // A row edited while its PDFs were written is dirty again:
                // the close gate prompts for it instead of dropping it.
                // Only the library this Save All started on: Discard All and
                // a reopen meanwhile leave the rows' editors gone.
                SaveAllNext::CloseLibrary { library_path } => {
                    let same_mount = self.library.library_at(&library_path).is_some()
                        && rows.iter().all(|a| self.library.editors.contains_key(a));
                    if same_mount {
                        Task::batch([sweep, self.handle_close_library(library_path)])
                    } else {
                        sweep
                    }
                }
            };
        }
        let listing = Self::save_all_failure_listing(&failed);
        let id = match next {
            SaveAllNext::CloseProject { .. } => signex_i18n::ids::NOTICE_SAVE_ALL_PROJECT_KEPT,
            SaveAllNext::CloseLibrary { .. } => signex_i18n::ids::NOTICE_SAVE_ALL_LIBRARY_KEPT,
            SaveAllNext::Nothing | SaveAllNext::SwitchBranch(_) => {
                signex_i18n::ids::NOTICE_SAVE_ALL_FAILED
            }
            SaveAllNext::Quit => signex_i18n::ids::NOTICE_SAVE_ALL_QUIT_HELD,
        };
        let message = signex_i18n::Translatable::new(id)
            .arg("count", failed.len())
            .arg("listing", listing);
        crate::diagnostics::log_warning(message.render_en_us());
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::save(message));
        // The app stays open: a system quit request still waiting is declined.
        if matches!(next, SaveAllNext::Quit) {
            return Self::decline_quit();
        }
        Task::none()
    }

    /// Fail each write of `settle` whose task panicked (#2269) as its own
    /// completion would have: the file or row is no longer marked saving, so
    /// a later save writes it, and the settle names it. Only these writes are
    /// touched; a row write running as a task of its own still reports.
    fn settle_stopped_writes(&mut self, settle: &mut SaveAllSettle) -> Task<Message> {
        let mut tasks = Vec::new();
        for write in std::mem::take(&mut settle.stopped) {
            match write {
                SaveAllWrite::File {
                    path,
                    generation: Some(generation),
                } => tasks.push(
                    self.handle_primitive_saved(crate::persist::PrimitiveSaveDone {
                        path,
                        generation,
                        save_as: None,
                        result: Err(WRITE_STOPPED_REASON.to_string()),
                    }),
                ),
                SaveAllWrite::File {
                    path,
                    generation: None,
                } => {
                    let saves = &mut self.document_state.document_saves;
                    saves.finish(&path);
                    saves.record_failure(path.clone(), WRITE_STOPPED_REASON.to_string());
                    tasks.push(self.drain_released_write(&path));
                }
                SaveAllWrite::Row(address) => {
                    self.stop_row_write(&address, settle.batch);
                    settle.failed.push((
                        self.library.editor_label(&address),
                        WRITE_STOPPED_REASON.to_string(),
                    ));
                }
            }
        }
        Task::batch(tasks)
    }

    /// The stopped write of `path` is released: run what its completion would
    /// have, the project save asked for meanwhile, or, for a sheet or board,
    /// the dirty marker re-read from its history and the warning for the
    /// spellings it superseded.
    fn drain_released_write(&mut self, path: &std::path::Path) -> Task<Message> {
        if let Some(DocumentKey::Project(project_id)) =
            self.document_state.document_key_for_path(path)
        {
            return self.start_project_follow_up(project_id, path);
        }
        // The file is unchanged, so an Undo back to the save point that the
        // write was holding dirty may now read clean.
        self.document_state.sync_history_dirty(path);
        self.warn_unwritten_spellings(path);
        Task::none()
    }

    /// The batch write of the row at `address` stopped without a result:
    /// release its PDF store (when that was the write), its in-flight mark
    /// and the editor's saving copy, so the row saves again.
    fn stop_row_write(&mut self, address: &crate::library::state::EditorAddress, batch: u64) {
        let store = self
            .library
            .batch_rows_in_flight
            .iter()
            .find(|(_, (b, a))| *b == batch && a == address)
            .map(|(generation, _)| *generation);
        if let Some(generation) = store {
            self.library.batch_rows_in_flight.remove(&generation);
            // The store registered its write on the library file.
            self.document_state
                .document_saves
                .finish(&address.library_path);
        }
        if let Some(state) = self.library.editors.get_mut(address)
            && state.saving_row.is_some()
            && store.is_none_or(|generation| state.saving_generation == generation)
        {
            state.abort_save();
            state.saving_batch = None;
        }
    }

    /// `path` as it is now: the pair `mark_saved` guards for the written one.
    fn pin_spelling(&self, path: &std::path::Path) -> PinnedSpelling {
        PinnedSpelling {
            path: path.to_path_buf(),
            revision: self
                .document_state
                .engines
                .get(path)
                .map_or(0, |engine| engine.revision()),
        }
    }

    /// A file held under several case-variant spellings was written once,
    /// under `written`. When that write left it clean, each other
    /// spelling's marker goes too, provided it was not edited since the
    /// write began (still equal to its pin): it names the same bytes on
    /// disk. A write that failed clears nothing. Taken from `variants` as
    /// they were when the write began, because a saved document nothing
    /// holds open is released as soon as it is clean.
    fn clear_saved_case_variants(&mut self, variants: &[CaseVariants]) {
        for CaseVariants { written, others } in variants {
            if self.document_state.is_document_dirty(written) {
                continue;
            }
            let unchanged: Vec<_> = others
                .iter()
                .filter(|pinned| self.pin_spelling(&pinned.path) == **pinned)
                .collect();
            for pinned in unchanged {
                self.document_state.mark_document_clean(&pinned.path);
                self.release_saved_sheet(&pinned.path);
            }
        }
    }

    /// Everything the Save All wrote landed. The UI stayed live during the
    /// writes, so re-run the close gate on the workspace as it is now
    /// instead of closing on the lists captured at click time: a document
    /// edited meanwhile is prompted for, never dropped.
    fn close_project_after_save_all(
        &mut self,
        project_id: crate::app::state::ProjectId,
        sheet_refs: crate::app::state::sheet_refs::SheetRefs,
        kept_paths: &[std::path::PathBuf],
    ) -> Task<Message> {
        let unchanged = self.document_state.stale_causes(&sheet_refs).is_empty()
            && self
                .document_state
                .projects
                .iter()
                .position(|p| p.id == project_id)
                .is_some_and(|idx| {
                    let scan = self.current_close_scan(idx, &sheet_refs);
                    scan.mine.is_empty()
                        && scan.mine_rows.is_empty()
                        && same_paths(&scan.kept, kept_paths)
                });
        if unchanged {
            return self.execute_close_project(project_id, &sheet_refs, kept_paths);
        }
        // Save All left a read-only project's settings unsaved (#2904).
        let read_only_left = self
            .document_state
            .project_by_id(project_id)
            .is_some_and(|p| {
                p.data.is_from_newer_minor() && self.document_state.is_document_dirty(&p.path)
            });
        crate::diagnostics::log_warning(if read_only_left {
            "Close Project: the read-only project's settings are still unsaved; review the list and confirm again"
        } else {
            "Close Project: documents changed while Save All ran; review the list and confirm again"
        });
        // The gate re-reads stale references, prompts for unsaved work, and
        // closes only when nothing of the project is left unsaved.
        self.handle_close_refs_resolved(project_id, Ok(Box::new(sheet_refs)))
    }

    /// Format the per-file failure lines (`name — reason`) for a Save-All
    /// error modal so the user sees *why* each file couldn't be saved, not
    /// just its name.
    fn save_all_failure_listing(failed: &[(String, String)]) -> String {
        failed
            .iter()
            .map(|(name, reason)| format!("{name} — {reason}"))
            .collect::<Vec<_>>()
            .join("\n  ")
    }
}

/// Run Save All's `writes`, then send `settle`. The runtime runs a task as
/// one stream, so a panicking write would end it, dropping the settle (a quit
/// request left unanswered) and every sibling write not yet reported (files
/// left marked saving forever). Each write is guarded on its own instead: a
/// panic ends only that write, the others still report, and the settle
/// arrives naming it in `stopped` (#2269). The writes are futures whose only
/// actions are their completion messages; any other action is dropped with a
/// warning.
pub(super) fn writes_then_settle(
    writes: Vec<(SaveAllWrite, Task<Message>)>,
    settle: SaveAllSettle,
) -> Task<Message> {
    use iced::futures::{StreamExt as _, stream};
    let settled = |settle: SaveAllSettle| Message::File(FileMsg::SaveAllSettled(Box::new(settle)));
    // Assumes no panic between a write's spawn_blocking start and its join.
    let guarded: Vec<_> = writes
        .into_iter()
        .filter_map(|(write, task)| {
            let stream = iced_runtime::task::into_stream(task)?;
            Some(
                std::panic::AssertUnwindSafe(stream)
                    .catch_unwind()
                    .map(move |step| step.map_err(|panic| (write.clone(), panic)))
                    .boxed(),
            )
        })
        .collect();
    if guarded.is_empty() {
        return Task::done(settled(settle));
    }
    // Each write's item, then `None` once all of them ended.
    let steps = stream::select_all(guarded)
        .map(Some)
        .chain(stream::once(std::future::ready(None)));
    let messages = steps
        .scan(Some(settle), move |settle, step| {
            let message = match step {
                Some(Ok(iced_runtime::Action::Output(message))) => Some(message),
                Some(Ok(_)) => {
                    crate::diagnostics::log_warning(
                        "Save All: a write asked the runtime for more than its result; ignored",
                    );
                    None
                }
                Some(Err((write, panic))) => {
                    crate::diagnostics::log_error(
                        "Save All: a write stopped unexpectedly",
                        &anyhow::anyhow!(panic_text(panic.as_ref())),
                    );
                    if let Some(settle) = settle.as_mut() {
                        settle.stopped.push(write);
                    }
                    None
                }
                None => settle.take().map(settled),
            };
            std::future::ready(Some(message))
        })
        .filter_map(std::future::ready);
    Task::stream(messages)
}

#[cfg(test)]
thread_local! {
    /// The file whose Save All write a test stops: the write begins as usual,
    /// then its task panics instead of reporting.
    pub(super) static STOP_WRITE_OF: std::cell::RefCell<Option<std::path::PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

/// `task`, or a task that panics in its place when a test asked to stop the
/// write of `target` (#2269).
#[cfg(test)]
fn stop_injected(target: &std::path::Path, task: Task<Message>) -> Task<Message> {
    let stop = STOP_WRITE_OF.with(|stop| {
        let mut stop = stop.borrow_mut();
        let hit = stop.as_deref() == Some(target);
        if hit {
            *stop = None;
        }
        hit
    });
    if !stop {
        return task;
    }
    // The real write began (the file is marked saving) and is dropped unrun,
    // as a panic would leave it.
    Task::future(write_that_panics())
}

#[cfg(test)]
async fn write_that_panics() -> Message {
    panic!("the write blew up")
}

/// The text of a caught panic, when it carries one.
fn panic_text(panic: &(dyn std::any::Any + Send)) -> String {
    panic
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic with no message".to_string())
}

/// Whether two path lists hold the same paths, whatever their order.
pub(super) fn same_paths(a: &[std::path::PathBuf], b: &[std::path::PathBuf]) -> bool {
    use std::collections::HashSet;
    a.iter().collect::<HashSet<_>>() == b.iter().collect::<HashSet<_>>()
}

/// The answers of [`probe_dirs`]: the decided ones, and the directories
/// whose volume could not be asked.
pub(super) type DirProbes = (
    HashMap<std::path::PathBuf, bool>,
    Vec<(std::path::PathBuf, CaseProbeOutcome)>,
);

/// Ask each directory's volume whether it ignores case. Pure: the caller
/// reports the undecided ones from `update`.
pub(super) fn probe_dirs(dirs: Vec<std::path::PathBuf>) -> DirProbes {
    let mut decided = HashMap::new();
    let mut undecided = Vec::new();
    for dir in dirs {
        match probe_directory_case(&dir) {
            Ok(Some(ignores_case)) => {
                decided.insert(dir, ignores_case);
            }
            Ok(None) => undecided.push((dir, CaseProbeOutcome::Undecided)),
            Err(reason) => undecided.push((dir, CaseProbeOutcome::StatFailed(reason))),
        }
    }
    (decided, undecided)
}
