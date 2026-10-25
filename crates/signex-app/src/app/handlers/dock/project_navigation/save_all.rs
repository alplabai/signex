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
use crate::app::contracts::{SaveAllNext, SaveAllSettle};

impl Signex {
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
    pub(super) fn begin_dirty_path_save(
        &mut self,
        path: &std::path::Path,
    ) -> anyhow::Result<Option<Task<Message>>> {
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

    /// Start the Save-All pass over dirty files and Component Preview rows.
    /// Failures to even start a write, and each write's own failure, are
    /// logged to the Messages panel. The returned task ends with
    /// [`SaveAllSettle`], which continues into `next`.
    pub(super) fn begin_save_all(
        &mut self,
        context: &str,
        paths: &[std::path::PathBuf],
        rows: &[crate::library::state::EditorAddress],
        next: SaveAllNext,
    ) -> Task<Message> {
        let mut writes = Vec::new();
        for path in paths {
            match self.begin_dirty_path_save(path) {
                Ok(task) => writes.extend(task),
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
        } = settle;
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
            return match next {
                SaveAllNext::CloseProject {
                    project_id,
                    sheet_refs,
                    kept_paths,
                } => self.close_project_after_save_all(project_id, *sheet_refs, &kept_paths),
                // The UI stayed live during the writes, so anything edited
                // meanwhile is dirty now; the quit gate prompts for it.
                SaveAllNext::Quit => self.handle_app_quit_requested(),
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
fn same_paths(a: &[std::path::PathBuf], b: &[std::path::PathBuf]) -> bool {
    use std::collections::HashSet;
    a.iter().collect::<HashSet<_>>() == b.iter().collect::<HashSet<_>>()
}
