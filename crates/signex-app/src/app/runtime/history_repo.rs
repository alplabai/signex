//! The History panel's repository work (mockup row 39): the graph and file
//! loads, the filters, New branch / Tag, and the branch switch. Every git
//! call runs on a worker and comes back as a [`HistoryRepoEvent`].

use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_i18n::ids;
use signex_library::repo_branches::{self, BranchTarget};
use signex_library::repo_graph;

use super::super::*;
use super::history::{locate_in_repo, tracked_file};
use crate::app::project_git::{SwitchRequest, SwitchSurface};
use crate::app::state::{ErrorNotice, verbatim};
use crate::panels::history::{
    HistoryCommitMsg, HistoryFilterMsg, HistoryPanelMsg, HistoryRepoEvent, HistoryRepoView,
    HistorySwitcherMsg, RefPrompt, RefPromptKind, can_create_branch, changed_files,
    visible_commits,
};

/// Blocking: the repository graph around `target`, `None` outside a repository.
fn load_repo_view(target: &Path) -> Result<Option<Arc<HistoryRepoView>>, String> {
    let full = tracked_file(target);
    let Some((root, rel)) = locate_in_repo(&full) else {
        return Ok(None);
    };
    let tracked = rel.to_string_lossy().replace('\\', "/");
    let snapshot = repo_graph::repo_snapshot(&root, &tracked).map_err(|e| e.to_string())?;
    Ok(Some(Arc::new(HistoryRepoView::from_snapshot(
        root, tracked, snapshot,
    ))))
}

/// Run `job` on the blocking pool; a failed join reads as `failed`.
async fn on_worker<T: Send + 'static>(
    job: impl FnOnce() -> T + Send + 'static,
    failed: impl FnOnce(String) -> T,
) -> T {
    match tokio::task::spawn_blocking(job).await {
        Ok(value) => value,
        Err(e) => failed(format!("spawn_blocking: {e}")),
    }
}

fn create_ref_task(
    root: PathBuf,
    kind: RefPromptKind,
    name: String,
    sha: Option<String>,
) -> Task<Message> {
    let job = move || {
        let created = match kind {
            RefPromptKind::Branch => repo_branches::create_branch(&root, &name, sha.as_deref()),
            RefPromptKind::Tag => {
                repo_branches::create_tag(&root, &name, sha.as_deref().unwrap_or_default())
            }
        };
        created.map_err(|e| e.to_string())
    };
    Task::perform(on_worker(job, Err), |result| {
        Message::Ui(UiMsg::HistoryRepo {
            window: None,
            event: HistoryRepoEvent::RefCreated(result),
        })
    })
}

impl Signex {
    pub(in crate::app) fn publish_history(&mut self) {
        self.document_state.panel_ctx.history = self.document_state.history.clone();
    }

    fn history_repo_root(&self) -> Option<PathBuf> {
        let repo = self.document_state.history.repo.as_ref()?;
        Some(repo.root.clone())
    }

    /// Recompute the rows the graph draws from the filters.
    fn refilter_history(&mut self) {
        let history = &mut self.document_state.history;
        let rows = history
            .repo
            .as_deref()
            .map(|repo| visible_commits(repo, &history.ui.filter))
            .unwrap_or_default();
        history.visible = Arc::new(rows);
    }

    /// Start the graph load of `target` for the current generation.
    pub(super) fn load_history_repo_task(&self, target: PathBuf) -> Task<Message> {
        let generation = self.document_state.history.generation;
        let window = self.interaction_state.acting_window;
        Task::perform(
            on_worker(move || load_repo_view(&target), Err),
            move |result| {
                Message::Ui(UiMsg::HistoryRepo {
                    window,
                    event: HistoryRepoEvent::ViewLoaded { generation, result },
                })
            },
        )
    }

    fn load_commit_files_task(&self, sha: String) -> Task<Message> {
        let Some(root) = self.history_repo_root() else {
            return Task::none();
        };
        let generation = self.document_state.history.generation;
        let window = self.interaction_state.acting_window;
        let reply_sha = sha.clone();
        let job = move || {
            repo_graph::commit_file_changes(&root, &sha)
                .map(changed_files)
                .map_err(|e| e.to_string())
        };
        Task::perform(on_worker(job, Err), move |result| {
            Message::Ui(UiMsg::HistoryRepo {
                window,
                event: HistoryRepoEvent::FilesLoaded {
                    generation,
                    sha: reply_sha.clone(),
                    result,
                },
            })
        })
    }

    /// A press in the History panel.
    pub(crate) fn handle_history_panel(&mut self, msg: HistoryPanelMsg) -> Task<Message> {
        let task = match msg {
            HistoryPanelMsg::RefreshPressed => {
                self.document_state.history.active_path = None;
                self.refresh_history_panel()
            }
            HistoryPanelMsg::Filter(msg) => {
                self.apply_history_filter(msg);
                Task::none()
            }
            HistoryPanelMsg::Commit(msg) => self.handle_history_commit(msg),
            HistoryPanelMsg::Switcher(msg) => self.handle_history_switcher(msg),
        };
        self.publish_history();
        task
    }

    fn apply_history_filter(&mut self, msg: HistoryFilterMsg) {
        let ui = &mut self.document_state.history.ui;
        match msg {
            HistoryFilterMsg::ScopePicked(scope) => ui.filter.scope = scope,
            HistoryFilterMsg::ThisFileToggled => ui.filter.this_file_only ^= true,
            HistoryFilterMsg::SearchToggled => {
                ui.search_open ^= true;
                if !ui.search_open {
                    ui.filter.search.clear();
                }
            }
            HistoryFilterMsg::SearchEdited(text) => ui.filter.search = text,
        }
        self.refilter_history();
    }

    fn handle_history_commit(&mut self, msg: HistoryCommitMsg) -> Task<Message> {
        let ui = &mut self.document_state.history.ui;
        match msg {
            HistoryCommitMsg::Selected(sha) => return self.select_history_commit(sha),
            HistoryCommitMsg::CommitPressed => return self.handle_active_document_save_requested(),
            HistoryCommitMsg::ShowDocumentPressed(rel) => {
                let Some(root) = self.history_repo_root() else {
                    return Task::none();
                };
                return Task::done(Message::File(FileMsg::Opened(Some(root.join(rel)))));
            }
            HistoryCommitMsg::MoreToggled => {
                ui.more_open ^= true;
                ui.prompt = None;
            }
            HistoryCommitMsg::NewBranchPressed => self.open_ref_prompt(RefPromptKind::Branch),
            HistoryCommitMsg::TagPressed => self.open_ref_prompt(RefPromptKind::Tag),
            HistoryCommitMsg::CopyShaPressed => {
                ui.more_open = false;
                if let Some(sha) = ui.selected.clone() {
                    return iced::clipboard::write(sha).discard();
                }
            }
            HistoryCommitMsg::PromptEdited(name) => {
                if let Some(prompt) = &mut ui.prompt {
                    prompt.name = name;
                }
            }
            HistoryCommitMsg::PromptSubmitted => return self.submit_ref_prompt(),
            HistoryCommitMsg::PromptCancelled => ui.prompt = None,
        }
        Task::none()
    }

    fn select_history_commit(&mut self, sha: String) -> Task<Message> {
        let ui = &mut self.document_state.history.ui;
        ui.more_open = false;
        ui.prompt = None;
        ui.files.clear();
        if ui.selected.as_deref() == Some(sha.as_str()) {
            ui.selected = None;
            return Task::none();
        }
        ui.selected = Some(sha.clone());
        self.load_commit_files_task(sha)
    }

    fn open_ref_prompt(&mut self, kind: RefPromptKind) {
        let ui = &mut self.document_state.history.ui;
        ui.more_open = false;
        ui.prompt = ui.selected.clone().map(|sha| RefPrompt {
            kind,
            sha,
            name: String::new(),
        });
    }

    /// Create what the name field names. An invalid name keeps the field open.
    fn submit_ref_prompt(&mut self) -> Task<Message> {
        let Some(root) = self.history_repo_root() else {
            return Task::none();
        };
        let ui = &mut self.document_state.history.ui;
        let Some(prompt) = ui.prompt.clone() else {
            return Task::none();
        };
        let name = prompt.name.trim().to_owned();
        let is_valid = match prompt.kind {
            RefPromptKind::Branch => repo_graph::is_valid_branch_name(&name),
            RefPromptKind::Tag => repo_graph::is_valid_tag_name(&name),
        };
        if !is_valid {
            return Task::none();
        }
        ui.prompt = None;
        create_ref_task(root, prompt.kind, name, Some(prompt.sha))
    }

    fn handle_history_switcher(&mut self, msg: HistorySwitcherMsg) -> Task<Message> {
        match msg {
            HistorySwitcherMsg::Toggled => {
                self.clear_save_offer();
                let ui = &mut self.document_state.history.ui;
                ui.switcher_open ^= true;
                ui.switcher_query.clear();
                self.sync_switch_block();
            }
            HistorySwitcherMsg::Dismissed => {
                self.clear_save_offer();
                self.document_state.history.ui.switcher_open = false;
            }
            HistorySwitcherMsg::QueryEdited(query) => {
                self.document_state.history.ui.switcher_query = query;
            }
            HistorySwitcherMsg::Picked { name, is_remote } => {
                let target = if is_remote {
                    BranchTarget::Remote(name)
                } else {
                    BranchTarget::Local(name)
                };
                return self.pick_history_branch(target);
            }
            HistorySwitcherMsg::SaveAndSwitchPressed => return self.save_then_switch(),
            HistorySwitcherMsg::CreatePressed => return self.create_branch_from_query(),
        }
        Task::none()
    }

    /// New branch here…: create the find field's name at HEAD. HEAD and the
    /// working tree stay where they are.
    fn create_branch_from_query(&mut self) -> Task<Message> {
        let Some(repo) = self.document_state.history.repo.clone() else {
            return Task::none();
        };
        let ui = &mut self.document_state.history.ui;
        if !can_create_branch(&repo.branches, &ui.switcher_query) {
            return Task::none();
        }
        let name = ui.switcher_query.trim().to_owned();
        ui.switcher_query.clear();
        create_ref_task(repo.root.clone(), RefPromptKind::Branch, name, None)
    }

    /// A branch row of the History switcher was pressed.
    fn pick_history_branch(&mut self, target: BranchTarget) -> Task<Message> {
        let Some(root) = self.history_repo_root() else {
            return Task::none();
        };
        self.request_branch_switch(SwitchRequest {
            surface: SwitchSurface::History,
            root,
            target,
        })
    }

    /// Reload the History of the main window and of every undocked one.
    pub(in crate::app) fn reload_all_histories(&mut self) -> Task<Message> {
        for history in self.document_state.window_history.values_mut() {
            history.active_path = None;
        }
        self.document_state.history.active_path = None;
        let main = self.refresh_history_panel();
        Task::batch([main, self.refresh_window_histories()])
    }

    /// A finished repository job.
    pub(crate) fn apply_history_repo_event(&mut self, event: HistoryRepoEvent) -> Task<Message> {
        let task = match event {
            HistoryRepoEvent::ViewLoaded { generation, result } => {
                self.apply_history_view(generation, result)
            }
            HistoryRepoEvent::FilesLoaded {
                generation,
                sha,
                result,
            } => {
                self.apply_history_files(generation, &sha, result);
                Task::none()
            }
            HistoryRepoEvent::RefCreated(result) => self.finish_ref_created(result),
            HistoryRepoEvent::SwitchFinished(result) => {
                self.finish_branch_switch(SwitchSurface::History, result)
            }
        };
        self.publish_history();
        task
    }

    fn apply_history_view(
        &mut self,
        generation: u32,
        result: Result<Option<Arc<HistoryRepoView>>, String>,
    ) -> Task<Message> {
        if generation != self.document_state.history.generation {
            return Task::none();
        }
        let commits_on_save = self
            .document_state
            .history
            .active_path
            .as_deref()
            .is_some_and(|path| self.git_project_for(path).is_some());
        let history = &mut self.document_state.history;
        history.repo_loading = false;
        history.ui.commits_on_save = commits_on_save;
        // A failed read is reported by the file-history load beside this one.
        history.repo = result.unwrap_or_else(|error| {
            tracing::warn!(target: "signex::history", %error, "repository graph not read");
            None
        });
        self.refilter_history();
        self.sync_switch_block();
        let history = &mut self.document_state.history;
        let kept = history.ui.selected.clone().filter(|sha| {
            history
                .repo
                .as_ref()
                .is_some_and(|repo| repo.commits.iter().any(|c| &c.sha == sha))
        });
        if kept.is_none() {
            history.ui.selected = None;
            history.ui.files.clear();
        }
        kept.map_or_else(Task::none, |sha| self.load_commit_files_task(sha))
    }

    fn apply_history_files(
        &mut self,
        generation: u32,
        sha: &str,
        result: Result<Vec<signex_widgets::history_graph::ChangedFile>, String>,
    ) {
        let history = &mut self.document_state.history;
        if generation != history.generation || history.ui.selected.as_deref() != Some(sha) {
            return;
        }
        history.ui.files = result.unwrap_or_else(|error| {
            tracing::warn!(target: "signex::history", %error, "commit files not read");
            Vec::new()
        });
    }

    fn finish_ref_created(&mut self, result: Result<(), String>) -> Task<Message> {
        if let Err(error) = result {
            crate::diagnostics::log_warning(format!("[git] {error}"));
            self.document_state.error_notice = Some(ErrorNotice::new(
                ids::PANELS_HISTORY_REF_FAILED,
                verbatim(error),
            ));
        }
        self.reload_all_histories()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::project_git::SwitchBlock;
    use crate::panels::history::HistoryFilter;
    use signex_library::adapters::local_git::project_repo::SwitchRefusal;

    fn app_with_repo(root: &Path) -> Signex {
        let (mut app, _boot) = Signex::new();
        app.document_state.history.repo = Some(Arc::new(HistoryRepoView {
            root: root.to_path_buf(),
            ..HistoryRepoView::default()
        }));
        app
    }

    /// The mockup's rule: unsaved edits block a switch with a plain message
    /// and Signex never saves on its own.
    #[test]
    fn unsaved_edits_in_the_repository_block_a_switch() {
        let root = PathBuf::from("/work/board");
        let mut app = app_with_repo(&root);
        app.document_state
            .mark_document_dirty(&root.join("power.snxsch"));

        let task = app.pick_history_branch(BranchTarget::Local("side".into()));
        drop(task);

        assert!(
            app.document_state.branch_switch.is_none(),
            "nothing started"
        );
        assert_eq!(
            app.document_state.history.ui.switch_block,
            Some(SwitchBlock::UnsavedEdits("power.snxsch".into()))
        );
    }

    #[test]
    fn unsaved_edits_in_another_repository_do_not_block() {
        let root = PathBuf::from("/work/board");
        let mut app = app_with_repo(&root);
        app.document_state
            .mark_document_dirty(Path::new("/work/other/a.snxsch"));
        assert_eq!(app.branch_switch_block(&root), None);
    }

    #[test]
    fn a_running_switch_refuses_saves_in_its_repository_only() {
        let root = PathBuf::from("/work/board");
        let mut app = app_with_repo(&root);
        app.document_state.branch_switch = Some(root.clone());
        assert!(
            app.refuse_during_branch_switch(&root.join("a.snxsch"))
                .is_err()
        );
        assert!(
            app.refuse_during_branch_switch(Path::new("/work/other/a.snxsch"))
                .is_ok()
        );
        assert_eq!(
            app.branch_switch_block(&root),
            Some(SwitchBlock::WorkRunning)
        );
    }

    /// A refused switch still releases the guard, or saves would stay
    /// blocked for the rest of the session.
    #[test]
    fn a_refused_switch_releases_the_guard_and_says_why() {
        let root = PathBuf::from("/work/board");
        let mut app = app_with_repo(&root);
        app.document_state.branch_switch = Some(root);

        let task =
            app.finish_branch_switch(SwitchSurface::History, Err(SwitchRefusal::WouldOverwrite));
        drop(task);

        assert!(app.document_state.branch_switch.is_none());
        assert!(app.document_state.error_notice.is_some());
    }

    #[test]
    fn the_project_file_is_held_and_a_sheet_is_not() {
        let root = PathBuf::from("/work/board");
        let (mut app, _boot) = Signex::new();
        app.document_state.tabs.push(crate::app::TabInfo {
            title: "s".into(),
            path: root.join("power.snxsch"),
            cached_document: None,
            project_id: None,
            kind: TabKind::Schematic,
        });
        app.document_state.tabs.push(crate::app::TabInfo {
            title: "lib".into(),
            path: root.join("parts.snxlib"),
            cached_document: None,
            project_id: None,
            kind: TabKind::LibraryBrowser(root.join("parts.snxlib")),
        });
        assert_eq!(
            app.branch_switch_held_paths(&root),
            vec![PathBuf::from("parts.snxlib")]
        );
    }

    #[test]
    fn a_stale_graph_load_is_dropped() {
        let root = PathBuf::from("/work/board");
        let mut app = app_with_repo(&root);
        app.document_state.history.generation = 7;
        let old = app.document_state.history.repo.clone();

        let task = app.apply_history_view(6, Ok(None));
        drop(task);

        assert_eq!(app.document_state.history.repo, old);
    }

    #[test]
    fn a_filter_press_recomputes_the_rows() {
        let root = PathBuf::from("/work/board");
        let mut app = app_with_repo(&root);
        let task =
            app.handle_history_panel(HistoryPanelMsg::Filter(HistoryFilterMsg::ThisFileToggled));
        drop(task);
        let ui = &app.document_state.panel_ctx.history.ui;
        assert_eq!(
            ui.filter,
            HistoryFilter {
                this_file_only: true,
                ..HistoryFilter::default()
            }
        );
    }
}
