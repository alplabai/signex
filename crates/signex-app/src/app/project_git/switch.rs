//! The one branch switch both surfaces run (mockup rows 35 and 39): what
//! blocks it, the save it asks for, the job on the blocking pool and what
//! follows. See `docs/PROJECT_TAB.md`, "Version control".

use std::path::{Path, PathBuf};

use iced::Task;
use signex_i18n::{ids, t};
use signex_library::adapters::local_git::project_repo::SwitchRefusal;
use signex_library::repo_branches::{self, BranchTarget};

use super::view::refusal_detail;
use super::{ProjectGitMsg, SwitchBlock, SwitchDone, SwitchRequest, SwitchSurface};
use crate::app::contracts::SaveAllNext;
use crate::app::handlers::dock::project_navigation::add_io::on_blocking_pool;
use crate::app::state::{ErrorNotice, ProjectId};
use crate::app::{Message, Signex, TabKind, UiMsg};
use crate::panels::history::HistoryRepoEvent;

/// Everything the blocking half of a switch needs.
struct SwitchJob {
    root: PathBuf,
    target: BranchTarget,
    /// Held paths the switch may not change; project files are not among them.
    held: Vec<PathBuf>,
    /// The open projects' `.snxprj` files, relative to the root.
    project_files: Vec<PathBuf>,
}

/// Blocking. A project file that differs must parse on the target branch
/// first: once the tree has moved, the project has to reopen from it.
fn run_switch(job: SwitchJob) -> Result<SwitchDone, SwitchRefusal> {
    let changed = repo_branches::changed_paths(&job.root, &job.target)?;
    let reopen: Vec<PathBuf> = job
        .project_files
        .iter()
        .filter(|file| changed.contains(file))
        .cloned()
        .collect();
    for rel in &reopen {
        let unreadable =
            || SwitchRefusal::ProjectFileUnreadable(rel.to_string_lossy().replace('\\', "/"));
        let bytes =
            repo_branches::file_on_branch(&job.root, &job.target, rel)?.ok_or_else(unreadable)?;
        signex_types::project::parse_project_bytes(&job.root.join(rel), &bytes)
            .map_err(|_| unreadable())?;
    }
    let branch = repo_branches::switch_branch(&job.root, &job.target, &job.held)?;
    Ok(SwitchDone {
        root: job.root,
        branch,
        changed,
        reopen,
    })
}

/// Blocking: whether a switch to `target` changes one of `project_files`.
fn project_file_differs(
    root: &Path,
    target: &BranchTarget,
    project_files: &[PathBuf],
) -> Result<bool, SwitchRefusal> {
    let changed = repo_branches::changed_paths(root, target)?;
    Ok(project_files.iter().any(|file| changed.contains(file)))
}

fn switch_finished(surface: SwitchSurface, result: Result<SwitchDone, SwitchRefusal>) -> Message {
    match surface {
        SwitchSurface::ProjectTab(project_id) => {
            Message::from(ProjectGitMsg::BranchSwitched { project_id, result })
        }
        SwitchSurface::History => Message::Ui(UiMsg::HistoryRepo {
            window: None,
            event: HistoryRepoEvent::SwitchFinished(result),
        }),
    }
}

impl Signex {
    /// Why a switch of the repository at `root` cannot start now. Unsaved
    /// edits come first: they are what the user can act on.
    pub(crate) fn branch_switch_block(&self, root: &Path) -> Option<SwitchBlock> {
        let d = &self.document_state;
        if let Some(path) = self.unsaved_paths_for_switch(root).into_iter().next() {
            let name = path.file_name().map_or_else(
                || path.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            );
            return Some(SwitchBlock::UnsavedEdits(name));
        }
        let overlaps = |repo: &PathBuf| repo.starts_with(root) || root.starts_with(repo);
        let committing = d.active_git_repos.iter().any(overlaps)
            || d.inflight_git_commits
                .iter()
                .any(|(repo, _)| overlaps(repo));
        let mut projects = d.projects.iter().filter(|p| p.dir().starts_with(root));
        let project_busy = projects.any(|project| {
            d.deferred_git_commits
                .iter()
                .any(|commit| commit.project == project.id)
                || d.project_folder_renames.contains(&project.id)
                || d.inflight_git_commits
                    .iter()
                    .any(|(repo, _)| repo.as_path() == Path::new(&project.data.dir))
        });
        let busy = committing
            || project_busy
            || d.document_saves.is_any_saving()
            || !d.pending_restores.is_empty()
            || d.branch_switch.is_some();
        busy.then_some(SwitchBlock::WorkRunning)
    }

    /// The unsaved documents a switch of `root` puts at risk, sorted: those in
    /// the repository, and those a project in it reaches from outside (a
    /// `../` child sheet), which its reopen would close.
    fn unsaved_paths_for_switch(&self, root: &Path) -> Vec<PathBuf> {
        let d = &self.document_state;
        let (mut paths, outside): (Vec<PathBuf>, Vec<PathBuf>) = d
            .dirty_document_paths()
            .into_iter()
            .partition(|path| path.starts_with(root));
        if !outside.is_empty() {
            for (index, project) in d.projects.iter().enumerate() {
                if !project.dir().starts_with(root) {
                    continue;
                }
                let reach = d.lexical_reach_sets(project.case_fold);
                let reached = outside.iter().filter(|path| reach.reached_by(index, path));
                paths.extend(reached.cloned());
            }
        }
        paths.sort();
        paths.dedup();
        paths
    }

    /// A project closed: a waiting switch or Save and switch offer that only
    /// it could answer goes with it, so its repository is not checked out.
    pub(crate) fn forget_switch_requests_of(&mut self, closed: ProjectId) {
        let docs = &self.document_state;
        let Some(dir) = docs.project_by_id(closed).map(|p| p.dir().to_path_buf()) else {
            return;
        };
        let is_orphaned = |request: &SwitchRequest| match request.surface {
            SwitchSurface::ProjectTab(id) => id == closed,
            SwitchSurface::History => {
                dir.starts_with(&request.root)
                    && !docs
                        .projects
                        .iter()
                        .any(|p| p.id != closed && p.dir().starts_with(&request.root))
            }
        };
        let git = &self.ui_state.project_git;
        let drops_parked = git.parked.as_ref().is_some_and(is_orphaned);
        let drops_offer = git.save_offer.as_ref().is_some_and(is_orphaned);
        if drops_parked {
            self.ui_state.project_git.parked = None;
        }
        if drops_offer {
            self.clear_save_offer();
        }
    }

    /// Keep the History switcher's open notice true as saves start and finish.
    pub(in crate::app) fn sync_switch_block(&mut self) {
        if !self.document_state.history.ui.switcher_open {
            return;
        }
        let block = self
            .document_state
            .history
            .repo
            .as_ref()
            .map(|repo| repo.root.clone())
            .and_then(|root| self.branch_switch_block(&root));
        let ui = &mut self.document_state.history.ui;
        if !matches!(block, Some(SwitchBlock::UnsavedEdits(_))) {
            ui.save_offered = false;
        }
        ui.switch_block = block;
    }

    /// Paths under `root`, relative to it, that the app holds in memory with
    /// no check against the disk: the project files, the open libraries and
    /// every open document other than a sheet or board, which the disk poll
    /// watches. A library that is the repository itself gives an empty path,
    /// which holds every file. A Layer Stack tab holds no document: its path
    /// is its board's.
    pub(crate) fn branch_switch_held_paths(&self, root: &Path) -> Vec<PathBuf> {
        let d = &self.document_state;
        let projects = d.projects.iter().map(|project| project.path.as_path());
        let tabs = d
            .tabs
            .iter()
            .filter(|tab| {
                !matches!(
                    tab.kind,
                    TabKind::Schematic
                        | TabKind::Pcb
                        | TabKind::LayerStack
                        | TabKind::PolygonManager
                )
            })
            .map(|tab| tab.path.as_path());
        let editors = d
            .footprint_editors
            .keys()
            .chain(d.symbol_editors.keys())
            .chain(d.block_diagrams.editors.keys())
            .map(PathBuf::as_path);
        let libraries = self
            .library
            .open_libraries
            .iter()
            .filter_map(|library| library.root_dir());
        projects
            .chain(tabs)
            .chain(editors)
            .chain(libraries)
            .filter_map(|path| path.strip_prefix(root).ok())
            .map(Path::to_path_buf)
            .collect()
    }

    /// A save or restore of a file in the repository being switched would
    /// race the checkout.
    pub(crate) fn refuse_during_branch_switch(&self, path: &Path) -> anyhow::Result<()> {
        match &self.document_state.branch_switch {
            Some(root) if path.starts_with(root) => anyhow::bail!(
                "A branch switch of {} is still running, so {} was left alone",
                root.display(),
                path.display()
            ),
            _ => Ok(()),
        }
    }

    fn project_files_under(&self, root: &Path) -> Vec<PathBuf> {
        self.document_state
            .projects
            .iter()
            .filter_map(|project| project.path.strip_prefix(root).ok())
            .map(Path::to_path_buf)
            .collect()
    }

    pub(super) fn projects_under(&self, root: &Path) -> Vec<ProjectId> {
        self.document_state
            .projects
            .iter()
            .filter(|project| project.dir().starts_with(root))
            .map(|project| project.id)
            .collect()
    }

    fn show_switch_block(&mut self, surface: SwitchSurface, block: SwitchBlock) {
        match surface {
            SwitchSurface::ProjectTab(id) => {
                self.ui_state.project_git.repo_mut(id).block = Some(block);
            }
            SwitchSurface::History => {
                self.document_state.history.ui.switch_block = Some(block);
            }
        }
    }

    /// Drop a Save and switch offer: it names no branch, so it must not
    /// outlive the pick it answers.
    pub(in crate::app) fn clear_save_offer(&mut self) {
        let git = &mut self.ui_state.project_git;
        git.save_offer = None;
        git.pick += 1;
        self.document_state.history.ui.save_offered = false;
    }

    /// A branch was picked on either surface: it replaces a switch still
    /// waiting. Pending drafts become edits first, so the block below sees
    /// them.
    pub(crate) fn request_branch_switch(&mut self, request: SwitchRequest) -> Task<Message> {
        self.ui_state.project_git.parked = None;
        self.commit_pending_title_block_drafts();
        if !self.settle_board_sketch_of(None) {
            return Task::none();
        }
        self.clear_save_offer();
        match self.branch_switch_block(&request.root) {
            None => self.start_branch_switch(request),
            Some(block @ SwitchBlock::UnsavedEdits(_)) => {
                self.show_switch_block(request.surface, block);
                self.check_project_file_task(request)
            }
            Some(block) => {
                self.show_switch_block(request.surface, block);
                Task::none()
            }
        }
    }

    /// With unsaved edits, read whether the branch has another project file:
    /// that switch offers Save and switch.
    fn check_project_file_task(&self, request: SwitchRequest) -> Task<Message> {
        let files = self.project_files_under(&request.root);
        if files.is_empty() {
            return Task::none();
        }
        let (root, target) = (request.root.clone(), request.target.clone());
        let pick = self.ui_state.project_git.pick;
        Task::perform(
            on_blocking_pool(move || Ok(project_file_differs(&root, &target, &files))),
            move |result| {
                Message::from(ProjectGitMsg::SwitchChecked {
                    request: Box::new(request),
                    pick,
                    result: result.unwrap_or_else(|why| Err(SwitchRefusal::Failed(why))),
                })
            },
        )
    }

    /// The read came back. A failed read keeps the plain unsaved-edits notice;
    /// the read of a pick since replaced or dismissed offers nothing.
    pub(super) fn finish_switch_check(
        &mut self,
        request: SwitchRequest,
        pick: u64,
        result: Result<bool, SwitchRefusal>,
    ) {
        if pick != self.ui_state.project_git.pick {
            return;
        }
        let is_still_unsaved = matches!(
            self.branch_switch_block(&request.root),
            Some(SwitchBlock::UnsavedEdits(_))
        );
        if result != Ok(true) || !is_still_unsaved {
            return;
        }
        if request.surface == SwitchSurface::History {
            self.document_state.history.ui.save_offered = true;
            self.publish_history();
        }
        self.ui_state.project_git.save_offer = Some(request);
    }

    /// Save and switch was pressed: the user's own save of the repository's
    /// edits, through Save All, then the switch.
    pub(crate) fn save_then_switch(&mut self) -> Task<Message> {
        let Some(request) = self.ui_state.project_git.save_offer.take() else {
            return Task::none();
        };
        self.clear_save_offer();
        self.commit_pending_title_block_drafts();
        let paths = self.unsaved_paths_for_switch(&request.root);
        if paths.is_empty() {
            return self.request_branch_switch(request);
        }
        let next = SaveAllNext::SwitchBranch(Box::new(request));
        self.begin_save_all("Switch branch", &paths, &[], next)
    }

    /// The save landed. Its commit may still be running: then the switch
    /// waits for the poll tick.
    pub(crate) fn resume_branch_switch(&mut self, request: SwitchRequest) -> Task<Message> {
        match self.branch_switch_block(&request.root) {
            None => self.start_branch_switch(request),
            Some(SwitchBlock::WorkRunning) => {
                self.ui_state.project_git.parked = Some(request);
                Task::none()
            }
            Some(_) => self.request_branch_switch(request),
        }
    }

    /// Poll tick: run a switch that was waiting for its save's commit.
    pub(crate) fn resume_parked_branch_switch(&mut self) -> Task<Message> {
        match self.ui_state.project_git.parked.take() {
            Some(request) => self.resume_branch_switch(request),
            None => Task::none(),
        }
    }

    fn start_branch_switch(&mut self, request: SwitchRequest) -> Task<Message> {
        let SwitchRequest {
            surface,
            root,
            target,
        } = request;
        if let SwitchSurface::ProjectTab(id) = surface
            && self.document_state.project_by_id(id).is_none()
        {
            return Task::none();
        }
        let project_files = self.project_files_under(&root);
        let mut held = self.branch_switch_held_paths(&root);
        held.retain(|path| !project_files.contains(path));
        self.document_state.branch_switch = Some(root.clone());
        self.document_state.history.ui.switch_block = None;
        match surface {
            SwitchSurface::ProjectTab(id) => {
                let repo = self.ui_state.project_git.repo_mut(id);
                repo.block = None;
                let (BranchTarget::Local(name) | BranchTarget::Remote(name)) = &target;
                repo.switching = Some(name.clone());
            }
            SwitchSurface::History => self.document_state.history.ui.switcher_open = false,
        }
        let job = SwitchJob {
            root,
            target,
            held,
            project_files,
        };
        Task::perform(
            on_blocking_pool(move || Ok(run_switch(job))),
            move |result| {
                let result = result.unwrap_or_else(|why| Err(SwitchRefusal::Failed(why)));
                switch_finished(surface, result)
            },
        )
    }

    /// The job reported back, from either surface.
    pub(crate) fn finish_branch_switch(
        &mut self,
        surface: SwitchSurface,
        result: Result<SwitchDone, SwitchRefusal>,
    ) -> Task<Message> {
        if let SwitchSurface::ProjectTab(id) = surface
            && self.document_state.project_by_id(id).is_none()
        {
            self.ui_state.project_git.repos.remove(&id);
        }
        match result {
            Err(refusal) => self.report_switch_refusal(surface, refusal),
            Ok(done) => {
                // The branch moved under a sheet of this repository.
                let ended = self.end_pause_on_sheet(|sheet| sheet.starts_with(&done.root));
                Task::batch([ended, self.land_branch_switch(done)])
            }
        }
    }

    /// Release the guard and say why, where the user asked. Nothing moved,
    /// unless the refusal is `NotRestored`, which names what did.
    fn report_switch_refusal(
        &mut self,
        surface: SwitchSurface,
        refusal: SwitchRefusal,
    ) -> Task<Message> {
        self.document_state.branch_switch = None;
        crate::diagnostics::log_warning(format!("[git] branch switch: {refusal:?}"));
        match surface {
            SwitchSurface::ProjectTab(id) => {
                if self.document_state.project_by_id(id).is_some() {
                    let repo = self.ui_state.project_git.repo_mut(id);
                    repo.switching = None;
                    repo.block = Some(SwitchBlock::Refused(refusal));
                }
            }
            SwitchSurface::History => {
                self.document_state.error_notice = Some(ErrorNotice::new(
                    ids::PANELS_HISTORY_SWITCH_FAILED,
                    refusal_detail(&refusal),
                ));
            }
        }
        let poll = self.start_document_disk_poll();
        Task::batch([self.reload_all_histories(), poll])
    }

    fn land_branch_switch(&mut self, done: SwitchDone) -> Task<Message> {
        let git = &mut self.ui_state.project_git;
        git.switcher = None;
        for id in self.projects_under(&done.root) {
            let repo = self.ui_state.project_git.repo_mut(id);
            repo.switching = None;
            repo.block = None;
            // Any read still running saw the old branch.
            repo.is_reading = false;
        }
        crate::diagnostics::log_info(
            t!(self.ui_state.i18n, ids::PROJECT_VCS_SWITCHED, "branch" => done.branch.as_str())
                .into_string(),
        );
        if done.reopen.is_empty() {
            self.document_state.branch_switch = None;
            return self.refresh_after_switch(&done);
        }
        // The guard stays until the project has reopened.
        self.reopen_refs_task(Box::new(done))
    }

    /// Read everything again. A clean document takes the branch's file; one
    /// with edits keeps them and gets the File Changed card from the poll.
    pub(super) fn refresh_after_switch(&mut self, done: &SwitchDone) -> Task<Message> {
        let reloads = self.reload_clean_documents(&done.root, &done.changed);
        let probe = self.start_project_fs_probe(true).unwrap_or_else(Task::none);
        let ids = self.projects_under(&done.root);
        let repos = Task::batch(ids.into_iter().map(|id| self.read_project_repo(id)));
        let poll = self.start_document_disk_poll();
        Task::batch([reloads, repos, probe, poll, self.reload_all_histories()])
    }

    /// Reload the open clean sheets and boards the switch changed. A clean
    /// engine with no tab is dropped: the next open reads the file.
    fn reload_clean_documents(&mut self, root: &Path, changed: &[PathBuf]) -> Task<Message> {
        let dirty = self.document_state.dirty_document_paths();
        let stale: Vec<PathBuf> = changed
            .iter()
            .map(|rel| root.join(rel))
            .filter(|path| !dirty.contains(path))
            .collect();
        let mut reloads = Vec::new();
        for path in stale {
            let has_tab = self.document_state.tabs.iter().any(|tab| {
                tab.path == path && matches!(tab.kind, TabKind::Schematic | TabKind::Pcb)
            });
            if has_tab {
                reloads.push(self.start_disk_reload(path));
            } else if self.document_state.engines.contains_key(&path) {
                self.document_state.remove_engine(&path);
                self.document_state
                    .document_saves
                    .forget_disk_records(&path);
            }
        }
        Task::batch(reloads)
    }
}
