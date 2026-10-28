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
    CaseVariants, PinnedSpelling, SaveAllNext, SaveAllPlan, SaveAllSettle,
};
use crate::app::state::project_case::{CaseProbeOutcome, undecided_message};
use crate::app::state::scope::{CaseFold, path_key_with};
use crate::app::state::volume_case::probe_directory_case;
use std::collections::HashMap;

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
            .find(|path| self.document_state.dirty_paths.contains(*path))
            .unwrap_or(&held[0])
    }

    /// Start the write of one dirty path, routing by the kind of document
    /// it backs:
    ///
    /// * a live schematic **engine** → an off-thread schematic save;
    /// * an open **symbol/footprint primitive editor** → an off-thread
    ///   container save;
    /// * the project's own **`.snxprj`** → `save_project_at_path`, which
    ///   still writes inline (it materialises pending libraries first).
    ///
    /// A dirty `.snxsym`/`.snxfpt` draft or `.snxprj` has no engine, so a
    /// loop that only knew engines would fail on them (#104). `Ok(None)`
    /// means the write already finished inline. The error says WHY a file
    /// cannot be saved.
    fn begin_held_path_save(
        &mut self,
        path: &std::path::Path,
    ) -> anyhow::Result<Option<Task<Message>>> {
        // Reported as this file's failure; the rest of the batch still saves.
        self.refuse_save_while_restoring(path)?;
        if self.document_state.engines.contains_key(path) {
            return self
                .begin_schematic_save(path.to_path_buf(), None)
                .map(Some);
        }
        if self.document_state.symbol_editors.contains_key(path)
            || self.document_state.footprint_editors.contains_key(path)
        {
            return self.save_primitive_tab_at(path).map(Some);
        }
        if self.document_state.projects.iter().any(|p| p.path == path) {
            return self.save_project_at_path(path).map(|()| None);
        }
        anyhow::bail!(
            "no open editor, engine, or project backs this file — it can't be saved automatically"
        )
    }

    /// Start the Save-All pass. A loose document (no open project) is keyed
    /// by a probe of its own directory, which reads the filesystem, so the
    /// probes run off-thread and the writes start when
    /// [`FileMsg::SaveAllProbed`] arrives; with none to probe they start now.
    pub(super) fn begin_save_all(
        &mut self,
        context: &str,
        paths: &[std::path::PathBuf],
        rows: &[crate::library::state::EditorAddress],
        next: SaveAllNext,
    ) -> Task<Message> {
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

    /// The volume rule for a write: the owning project's, else the probe of
    /// the document's own directory (macOS does not fold by default, yet its
    /// usual volume does, so two spellings of one file would otherwise start
    /// two racing writes, #1216). A directory the probe could not decide
    /// keeps the host default; it is never assumed to fold.
    pub(crate) fn fold_for_save(
        &self,
        path: &std::path::Path,
        probes: &HashMap<std::path::PathBuf, bool>,
    ) -> CaseFold {
        let probed = match self.project_owning(path) {
            None => path.parent().and_then(|dir| probes.get(dir)),
            Some(_) => None,
        };
        match probed {
            Some(&ignores_case) => CaseFold::from_probe(ignores_case),
            None => self.fold_for_path(path),
        }
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
        for (dir, outcome) in &undecided {
            crate::diagnostics::log_warning(undecided_message(
                outcome,
                dir,
                CaseFold::host_default(),
            ));
        }
        let (context, paths, rows, probes) = (context.as_str(), &paths[..], &rows[..], &probes);
        let mut writes = Vec::new();
        // One file reached under several spellings is written once.
        let mut begun = std::collections::HashSet::new();
        let mut case_variants = Vec::new();
        for path in paths {
            let fold = self.fold_for_save(path, probes);
            if !begun.insert(path_key_with(path, fold)) {
                continue;
            }
            let held = self.held_spellings(path, fold);
            let target = self.write_target(&held).clone();
            match self.begin_held_save(&held, &target) {
                Ok(task) => {
                    // Only a write that began can make the other spellings
                    // clean; a refused one leaves every marker (#1423). Each
                    // other is pinned now: one edited while the write runs
                    // is not on disk and stays dirty.
                    if held.len() > 1 {
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
                    writes.extend(task);
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
        for address in rows {
            let label = self.library.editor_label(address);
            if let Err(err) = self.try_save_row(address) {
                crate::diagnostics::log_error(&format!("{context}: could not save {label}"), &err);
                failed.push((label, format!("{err:#}")));
            }
        }
        let settled = Message::File(FileMsg::SaveAllSettled(Box::new(SaveAllSettle {
            next,
            paths: paths.to_vec(),
            failed,
            case_variants,
        })));
        Task::batch(writes).chain(Task::done(settled))
    }

    /// Every write of a Save All completed: continue into the close / quit,
    /// or stay open and name what did not save.
    pub(crate) fn handle_save_all_settled(&mut self, settle: SaveAllSettle) -> Task<Message> {
        let SaveAllSettle {
            next,
            paths,
            failed: rows_failed,
            case_variants,
        } = settle;
        self.clear_saved_case_variants(&case_variants);
        // Drop the dirty dots of everything that DID save.
        self.refresh_panel_ctx();
        let mut failed: Vec<(String, String)> = Vec::new();
        for path in paths
            .iter()
            .filter(|path| self.document_state.dirty_paths.contains(*path))
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
            };
        }
        let listing = Self::save_all_failure_listing(&failed);
        let message = match next {
            SaveAllNext::CloseProject { .. } => format!(
                "Could not save {} item(s) — project not closed:\n  {listing}",
                failed.len()
            ),
            SaveAllNext::Quit => format!(
                "Could not save {} item(s) — Signex stayed open so nothing is lost. \
                 Save them manually, or choose Discard All to exit anyway:\n  {listing}",
                failed.len()
            ),
        };
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::save(message));
        Task::none()
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
            net_color_revision: self.net_color_revision_of(path),
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
            if self.document_state.dirty_paths.contains(written) {
                continue;
            }
            let unchanged: Vec<_> = others
                .iter()
                .filter(|pinned| self.pin_spelling(&pinned.path) == **pinned)
                .collect();
            for pinned in unchanged {
                for tab in self
                    .document_state
                    .tabs
                    .iter_mut()
                    .filter(|tab| tab.path == pinned.path)
                {
                    tab.dirty = false;
                }
                self.document_state.dirty_paths.remove(&pinned.path);
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
        self.sync_editor_dirty_paths();
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
        crate::diagnostics::log_warning(
            "Close Project: documents changed while Save All ran; review the list and confirm again",
        );
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
