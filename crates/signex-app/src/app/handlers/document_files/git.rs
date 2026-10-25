//! Project git-commit handlers. Split from `handlers/document_files.rs`.

use anyhow::Result;

use super::super::super::*;

impl Signex {
    /// v0.22 Phase 8.4 — auto-commit a saved file into the owning
    /// project's local Git repo when `enable_git` is on.
    ///
    /// Walks `document_state.projects` looking for the project whose
    /// `data.dir` is a prefix of `file_path`. If found AND
    /// `data.enable_git == true`, opens
    /// [`signex_library::adapters::local_git::project::LocalGitProjectAdapter`] and runs `commit_path`.
    ///
    /// Failure is best-effort: logged + surfaced as a non-modal
    /// status warning, never blocks the save. The user's data is on
    /// disk regardless of whether git captures it.
    ///
    /// v0.23 — Async pipeline. The save-handler synchronously
    /// resolves the owning project + relative path (cheap — just
    /// walks `DocumentState.projects`), then pushes a
    /// [`crate::app::state::PendingGitCommit`] onto
    /// `pending_git_commits` and adds the pair to
    /// `inflight_git_commits` so the status bar's "Saving…" pill
    /// shows immediately. The actual `git2` work runs in
    /// `finish_update`'s [`Self::drain_pending_git_commits`] which
    /// emits one `Task::perform` per queued commit. Result lands as
    /// `Message::Project(ProjectMsg::GitCommitDone)`; the handler clears
    /// the inflight entry.
    pub fn commit_save_to_project_git(
        &mut self,
        file_path: &std::path::Path,
        default_message: &str,
    ) {
        let owning = self.document_state.projects.iter().find(|p| {
            if !p.data.enable_git {
                return false;
            }
            let dir = std::path::Path::new(&p.data.dir);
            file_path.starts_with(dir)
        });
        let Some(project) = owning else {
            return;
        };
        let project_root = std::path::PathBuf::from(&project.data.dir);
        let rel_path = match file_path.strip_prefix(&project_root) {
            Ok(p) => p.to_path_buf(),
            Err(_) => return,
        };

        // Idempotent: an enqueue for the same (project_root, rel_path) is
        // DROPPED while the previous one is still inflight, not re-queued.
        // A change made in that window stays uncommitted until a later
        // enqueue; a clean PCB has none, so it waits for the next save.
        let key = (project_root.clone(), rel_path.clone());
        if !self.document_state.inflight_git_commits.insert(key) {
            return;
        }
        self.document_state
            .pending_git_commits
            .push(crate::app::state::PendingGitCommit {
                project_root,
                rel_path,
                message: default_message.to_string(),
            });
    }

    /// v0.23 — Drain the pending-commit queue. Returns a
    /// `Task::batch` of `Task::perform` calls that each open the
    /// project's git adapter and run `commit_path` on a tokio
    /// `spawn_blocking`. Each completion routes through
    /// `Message::Project(ProjectMsg::GitCommitDone)` which clears the
    /// matching `inflight_git_commits` entry. Returns `Task::none()` when the
    /// queue is empty.
    ///
    /// **Ordering:** commits are serialised per repository (#833). Each
    /// task opens its own `LocalGitProjectAdapter`, so the adapter's
    /// mutex never contends across tasks and two concurrent commits
    /// would race on `.git/index.lock`. Instead the dispatcher keeps
    /// at most one commit in flight per repo root
    /// (`active_git_repos`); later commits for that repo stay queued,
    /// in save order, until [`Self::handle_project_git_commit_done`]
    /// releases the repo and drains again. Different repos still run
    /// in parallel.
    pub(crate) fn drain_pending_git_commits(&mut self) -> iced::Task<crate::app::Message> {
        if self.document_state.pending_git_commits.is_empty() {
            return iced::Task::none();
        }
        let queued = std::mem::take(&mut self.document_state.pending_git_commits);
        let mut drained: Vec<crate::app::state::PendingGitCommit> = Vec::new();
        for pending in queued {
            if self
                .document_state
                .active_git_repos
                .insert(pending.project_root.clone())
            {
                drained.push(pending);
            } else {
                self.document_state.pending_git_commits.push(pending);
            }
        }
        let tasks: Vec<iced::Task<crate::app::Message>> = drained
            .into_iter()
            .map(|pending| {
                let project_root = pending.project_root.clone();
                let rel_path = pending.rel_path.clone();
                let message = pending.message.clone();
                let response_root = project_root.clone();
                let response_rel = rel_path.clone();
                iced::Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let adapter = signex_library::adapters::local_git_project::LocalGitProjectAdapter::open_or_init(
                                project_root.clone(),
                            )
                            .map_err(|e| {
                                format!("open_or_init({}) failed: {e}", project_root.display())
                            })?;
                            adapter
                                .commit_path(&rel_path, &message)
                                .map(|oid| oid.map(|oid| oid.to_string()))
                                .map_err(|e| {
                                    format!(
                                        "commit_path({}) failed: {e}",
                                        rel_path.display()
                                    )
                                })
                        })
                        .await
                        .unwrap_or_else(|e| Err(format!("spawn_blocking: {e}")))
                    },
                    move |result| {
                        crate::app::Message::Project(crate::app::ProjectMsg::GitCommitDone {
                            project_root: response_root.clone(),
                            rel_path: response_rel.clone(),
                            result,
                        })
                    },
                )
            })
            .collect();
        iced::Task::batch(tasks)
    }

    /// v0.23 — Handler for [`ProjectMsg::GitCommitDone`]. Clears
    /// the matching `inflight_git_commits` entry, logs the result, and
    /// releases the repo so its next queued commit (if any) starts —
    /// the returned task.
    pub(crate) fn handle_project_git_commit_done(
        &mut self,
        project_root: std::path::PathBuf,
        rel_path: std::path::PathBuf,
        result: Result<Option<String>, String>,
    ) -> iced::Task<crate::app::Message> {
        self.document_state
            .inflight_git_commits
            .remove(&(project_root.clone(), rel_path.clone()));
        self.document_state.active_git_repos.remove(&project_root);
        match result {
            Ok(Some(oid)) => crate::diagnostics::log_info(format!(
                "[git] committed {} in {} ({})",
                rel_path.display(),
                project_root.display(),
                oid
            )),
            Ok(None) => crate::diagnostics::log_info(format!(
                "[git] {} unchanged in {}; nothing to commit",
                rel_path.display(),
                project_root.display(),
            )),
            Err(e) => crate::diagnostics::log_warning(format!("[git] {e}")),
        }
        self.drain_pending_git_commits()
    }
}
