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
        self.queue_project_git_commit(file_path, default_message, true, None, None);
    }

    /// Like [`Self::commit_save_to_project_git`], but the commit records
    /// exactly `content`: the bytes a save wrote or a restore read back. A
    /// later write to the file (an external editor, a History restore) then
    /// cannot change what the commit says it saved.
    pub fn commit_bytes_to_project_git(
        &mut self,
        file_path: &std::path::Path,
        default_message: &str,
        content: std::sync::Arc<[u8]>,
    ) {
        self.queue_project_git_commit(file_path, default_message, true, None, Some(content));
    }

    /// Queue one commit that records `old_path` moving to `new_path`: the
    /// old path is removed and the new one added in the same commit, so
    /// the history shows a rename and `git status` has no stray deletion
    /// (#1219). Both paths are absolute and lie under the owning project's
    /// directory (as it is named now). Shares the in-flight tracking and
    /// per-repo ordering of every other project commit.
    pub fn commit_project_rename_to_git(
        &mut self,
        old_path: &std::path::Path,
        new_path: &std::path::Path,
        message: &str,
    ) {
        self.queue_project_git_commit(new_path, message, true, Some(old_path), None);
    }

    /// Like [`Self::commit_save_to_project_git`], but never replaces the
    /// message of a commit of this file that is already pending: it (a
    /// History "Restore X") is not this save's to replace. A commit of this
    /// file that is already running does not hide this one; it is queued
    /// behind it.
    pub fn commit_save_to_project_git_if_unqueued(
        &mut self,
        file_path: &std::path::Path,
        default_message: &str,
    ) {
        self.queue_project_git_commit(file_path, default_message, false, None, None);
    }

    fn queue_project_git_commit(
        &mut self,
        file_path: &std::path::Path,
        default_message: &str,
        replace_message: bool,
        removed_path: Option<&std::path::Path>,
        content: Option<std::sync::Arc<[u8]>>,
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
        // While the folder moves, `data.dir` still names the old folder and
        // opening a repository there could `git init` a fresh one.
        if self
            .document_state
            .project_folder_renames
            .contains(&project.id)
        {
            crate::diagnostics::log_warning(format!(
                "Skipped a version-control commit of {} while its project folder is being renamed",
                file_path.display()
            ));
            return;
        }
        let project_root = std::path::PathBuf::from(&project.data.dir);
        let rel_path = match file_path.strip_prefix(&project_root) {
            Ok(p) => p.to_path_buf(),
            Err(_) => return,
        };
        let removed_rel_path = removed_path.and_then(|p| p.strip_prefix(&project_root).ok());
        let removed_rel_path = removed_rel_path.map(std::path::Path::to_path_buf);

        // A (project_root, rel_path) still queued has not started, so the
        // newest save takes its message and its bytes. A commit already
        // running is not touched: this one queues behind it (commits of one
        // repo run in order) and holds its own bytes, so nothing landing
        // after the running commit read the file is left without a version.
        if let Some(queued) = self
            .document_state
            .pending_git_commits
            .iter_mut()
            .find(|c| c.project_root == project_root && c.rel_path == rel_path)
        {
            if replace_message {
                queued.message = default_message.to_string();
                queued.content = content;
            }
            if removed_rel_path.is_some() {
                queued.removed_rel_path = removed_rel_path;
            }
            if queued.content.is_none() {
                self.document_state
                    .unbound_git_commits
                    .insert((project_root, rel_path));
            }
            return;
        }
        let key = (project_root.clone(), rel_path.clone());
        if content.is_none() {
            self.document_state.unbound_git_commits.insert(key.clone());
        }
        self.document_state.inflight_git_commits.insert(key);
        self.document_state
            .pending_git_commits
            .push(crate::app::state::PendingGitCommit {
                project_root,
                rel_path,
                removed_rel_path,
                message: default_message.to_string(),
                content,
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
                let removed_rel_path = pending.removed_rel_path.clone();
                let message = pending.message.clone();
                let content = pending.content.clone();
                let response_root = project_root.clone();
                let response_rel = rel_path.clone();
                iced::Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let adapter = signex_library::adapters::local_git_project::LocalGitProjectAdapter::open_or_init(
                                project_root.clone(),
                            )
                            .map_err(|e| match e {
                                signex_library::LibraryError::VersionControlRefused(reason) => {
                                    crate::app::GitCommitFailure::Refused(reason)
                                }
                                e => crate::app::GitCommitFailure::Failed(format!(
                                    "open_or_init({}) failed: {e}",
                                    project_root.display()
                                )),
                            })?;
                            let committed = match (&removed_rel_path, &content) {
                                (Some(removed), _) => {
                                    adapter.commit_rename(removed, &rel_path, &message)
                                }
                                (None, Some(bytes)) => {
                                    adapter.commit_path_with_bytes(&rel_path, bytes, &message)
                                }
                                (None, None) => adapter.commit_path(&rel_path, &message),
                            };
                            committed
                                .map(|oid| oid.map(|oid| oid.to_string()))
                                .map_err(|e| {
                                    crate::app::GitCommitFailure::Failed(format!(
                                        "commit_path({}) failed: {e}",
                                        rel_path.display()
                                    ))
                                })
                        })
                        .await
                        .unwrap_or_else(|e| {
                            Err(crate::app::GitCommitFailure::Failed(format!(
                                "spawn_blocking: {e}"
                            )))
                        })
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
        result: Result<Option<String>, crate::app::GitCommitFailure>,
    ) -> iced::Task<crate::app::Message> {
        let key = (project_root.clone(), rel_path.clone());
        self.document_state.inflight_git_commits.remove(&key);
        self.document_state.unbound_git_commits.remove(&key);
        self.document_state.active_git_repos.remove(&project_root);
        // A save that landed while this commit ran is queued behind it and
        // still counts as in flight (and as unbound if it holds no bytes).
        if let Some(queued) = self
            .document_state
            .pending_git_commits
            .iter()
            .find(|c| c.project_root == project_root && c.rel_path == rel_path)
        {
            if queued.content.is_none() {
                self.document_state.unbound_git_commits.insert(key.clone());
            }
            self.document_state.inflight_git_commits.insert(key);
        }
        // A commit that worked means the folder is fine now: a later refusal
        // is news again.
        if result.is_ok() {
            self.document_state
                .paused_git_projects
                .remove(&project_root);
        }
        let reload = if matches!(result, Ok(Some(_))) {
            self.reload_history_if_showing(&project_root.join(&rel_path))
        } else {
            iced::Task::none()
        };
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
            Err(crate::app::GitCommitFailure::Refused(reason)) => {
                self.report_git_paused(&project_root, &rel_path, &reason);
            }
            Err(crate::app::GitCommitFailure::Failed(e)) => {
                crate::diagnostics::log_warning(format!("[git] {e}"));
            }
        }
        iced::Task::batch([reload, self.drain_pending_git_commits()])
    }

    /// Auto-commit was refused for a save. Every refusal is logged; the
    /// notice card is raised once per project until a commit succeeds or the
    /// project closes (#1205).
    fn report_git_paused(
        &mut self,
        project_root: &std::path::Path,
        rel_path: &std::path::Path,
        reason: &str,
    ) {
        crate::diagnostics::log_info(format!(
            "[git] {} in {} was not committed: {reason}",
            rel_path.display(),
            project_root.display()
        ));
        if !self
            .document_state
            .paused_git_projects
            .insert(project_root.to_path_buf())
        {
            return;
        }
        let message = format!(
            "Version control is paused for {}: {reason}",
            project_root.display()
        );
        crate::diagnostics::log_warning(format!("[git] {message}"));
        self.document_state.error_notice = Some(
            crate::app::state::ErrorNotice::version_control_paused(message),
        );
    }
}
