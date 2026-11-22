//! `update` of the project git slice: reads, the Project tab's branch pick
//! and the author write. Every git call runs on the blocking pool; the
//! switch itself is in `switch`.

use iced::Task;
use signex_library::adapters::local_git::project_repo::{
    RepoFacts, parse_author, read_repo_facts, write_author,
};
use signex_library::repo_branches::BranchTarget;

use super::{AuthorProblem, ProjectGitMsg, SwitchRequest, SwitchSurface};
use crate::app::ProjectSection;
use crate::app::handlers::dock::project_navigation::add_io::on_blocking_pool;
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

impl Signex {
    pub(crate) fn dispatch_project_git_message(&mut self, msg: ProjectGitMsg) -> Task<Message> {
        use ProjectGitMsg as M;
        match msg {
            M::RepoReadRequested { project_id } => self.read_project_repo(project_id),
            M::RepoRead {
                project_id,
                generation,
                result,
            } => {
                self.install_repo_read(project_id, generation, result);
                Task::none()
            }
            M::SwitcherToggled { project_id } => self.toggle_branch_switcher(project_id),
            M::BranchPicked { project_id, name } => self.pick_project_branch(project_id, name),
            M::BranchSwitched { project_id, result } => {
                self.finish_branch_switch(SwitchSurface::ProjectTab(project_id), result)
            }
            M::SwitchChecked { request, result } => {
                self.finish_switch_check(*request, result);
                Task::none()
            }
            M::SaveAndSwitchPressed => self.save_then_switch(),
            M::ReopenRefsResolved { done, result } => self.finish_project_reopen(done, result),
            M::AuthorEdited { project_id, text } => {
                if self.document_state.project_by_id(project_id).is_some() {
                    let repo = self.ui_state.project_git.repo_mut(project_id);
                    repo.author_draft = Some(text);
                    repo.author_problem = None;
                }
                Task::none()
            }
            M::AuthorSubmitted { project_id } => self.write_project_author(project_id),
            M::AuthorWritten { project_id, result } => self.finish_author_write(project_id, result),
        }
    }

    /// Start a read of `project_id`'s repository, unless one is running or
    /// the folder has none.
    pub(crate) fn read_project_repo(&mut self, project_id: ProjectId) -> Task<Message> {
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return Task::none();
        };
        let root = project.dir().to_path_buf();
        if self.document_state.project_fs.facts.lacks_git(&root) {
            return Task::none();
        }
        let repo = self.ui_state.project_git.repo_mut(project_id);
        if repo.is_reading {
            return Task::none();
        }
        repo.is_reading = true;
        repo.generation += 1;
        let generation = repo.generation;
        Task::perform(
            on_blocking_pool(move || read_repo_facts(&root).map_err(|e| e.to_string())),
            move |result| {
                Message::from(ProjectGitMsg::RepoRead {
                    project_id,
                    generation,
                    result,
                })
            },
        )
    }

    /// The poll tick: read the repositories whose Version control section
    /// is showing, so a commit or an outside branch change appears.
    pub(crate) fn reread_shown_project_repos(&mut self) -> Task<Message> {
        let shown: Vec<ProjectId> = self
            .document_state
            .project_tabs
            .iter()
            .filter(|(_, tab)| tab.section == ProjectSection::VersionControl)
            .map(|(id, _)| *id)
            .collect();
        Task::batch(shown.into_iter().map(|id| self.read_project_repo(id)))
    }

    fn install_repo_read(
        &mut self,
        project_id: ProjectId,
        generation: u64,
        result: Result<RepoFacts, String>,
    ) {
        if self.document_state.project_by_id(project_id).is_none() {
            self.ui_state.project_git.repos.remove(&project_id);
            return;
        }
        let repo = self.ui_state.project_git.repo_mut(project_id);
        if repo.generation != generation {
            return;
        }
        repo.is_reading = false;
        match result {
            Ok(facts) => {
                repo.facts = Some(facts);
                repo.read_error = None;
            }
            Err(why) => repo.read_error = Some(why),
        }
    }

    fn toggle_branch_switcher(&mut self, project_id: ProjectId) -> Task<Message> {
        self.clear_save_offer();
        let git = &mut self.ui_state.project_git;
        if git.is_switcher_open(project_id) {
            git.switcher = None;
            return Task::none();
        }
        git.switcher = Some(project_id);
        git.repo_mut(project_id).block = None;
        self.read_project_repo(project_id)
    }

    /// A branch row of the Project tab's switcher was pressed.
    fn pick_project_branch(&mut self, project_id: ProjectId, name: String) -> Task<Message> {
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return Task::none();
        };
        let root = project.dir().to_path_buf();
        let git = &mut self.ui_state.project_git;
        let repo = git.repo_mut(project_id);
        if repo.switching.is_some() {
            return Task::none();
        }
        let is_current = repo
            .facts
            .as_ref()
            .is_some_and(|facts| facts.branch.as_deref() == Some(name.as_str()));
        if is_current {
            git.switcher = None;
            return Task::none();
        }
        self.request_branch_switch(SwitchRequest {
            surface: SwitchSurface::ProjectTab(project_id),
            root,
            target: BranchTarget::Local(name),
        })
    }

    /// Enter in the Author field. Empty removes this repository's own
    /// identity, so git's user-level one applies.
    fn write_project_author(&mut self, project_id: ProjectId) -> Task<Message> {
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return Task::none();
        };
        let root = project.dir().to_path_buf();
        let repo = self.ui_state.project_git.repo_mut(project_id);
        let Some(typed) = repo.author_draft.clone() else {
            return Task::none();
        };
        let typed = typed.trim().to_string();
        let author = if typed.is_empty() {
            None
        } else {
            match parse_author(&typed) {
                Some(pair) => Some(pair),
                None => {
                    repo.author_problem = Some(AuthorProblem::Unreadable);
                    return Task::none();
                }
            }
        };
        repo.author_problem = None;
        Task::perform(
            on_blocking_pool(move || {
                let pair = author.as_ref().map(|(n, e)| (n.as_str(), e.as_str()));
                write_author(&root, pair).map_err(|e| e.to_string())
            }),
            move |result| Message::from(ProjectGitMsg::AuthorWritten { project_id, result }),
        )
    }

    fn finish_author_write(
        &mut self,
        project_id: ProjectId,
        result: Result<(), String>,
    ) -> Task<Message> {
        if self.document_state.project_by_id(project_id).is_none() {
            self.ui_state.project_git.repos.remove(&project_id);
            return Task::none();
        }
        let repo = self.ui_state.project_git.repo_mut(project_id);
        match result {
            Ok(()) => {
                repo.author_draft = None;
                repo.author_problem = None;
                self.read_project_repo(project_id)
            }
            Err(why) => {
                repo.author_problem = Some(AuthorProblem::WriteFailed(why));
                Task::none()
            }
        }
    }
}
