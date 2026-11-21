//! Version control section of the Project tab (#1693): whether the project
//! folder has a Git repository (from the last filesystem probe), Enable
//! Version Control… (the project tree's dialog: `git init`, the first
//! commit), and the "Commit every save" switch, which is the `.snxprj`
//! `enable_git` flag and a General step. Turning it off keeps the repository.
//! The look (mockup row 35) is in `view`.

mod view;

#[cfg(test)]
mod tests;

use std::path::Path;

use crate::app::Signex;
use crate::app::state::project_fs::git_marker;
use crate::app::state::{LoadedProject, ProjectId};

pub(super) use view::view;

/// Whether the project folder holds a Git repository, as last probed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Repository {
    Present,
    Absent,
    /// Not probed yet.
    Unknown,
}

impl Repository {
    /// The token of the `project_settings` answer.
    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Absent => "absent",
            Self::Unknown => "unknown",
        }
    }
}

/// What the section shows, read from the Model.
pub(crate) struct VersionControlFacts {
    pub repository: Repository,
    /// Commits of this project's saves queued or running.
    pub pending: usize,
}

/// Whether the Commit every save switch may be flipped from `on`: turning it
/// on needs a repository; turning it off never does.
pub(crate) fn may_toggle_commit_on_save(on: bool, repository: Repository) -> bool {
    on || repository != Repository::Absent
}

impl Signex {
    /// The section's facts for `project`, read from the Model: the last
    /// probe of its folder and the commit queue.
    pub(crate) fn version_control_facts(&self, project: &LoadedProject) -> VersionControlFacts {
        // The folder the tree's Enable Version Control probes and inits.
        let folder = project.path.parent().unwrap_or(Path::new(""));
        let dir = Path::new(&project.data.dir);
        let facts = &self.document_state.project_fs.facts;
        let repository = if facts.lacks_git(folder) {
            Repository::Absent
        } else if facts.knows_present(&git_marker(folder)) {
            Repository::Present
        } else {
            Repository::Unknown
        };
        let pending = self
            .document_state
            .inflight_git_commits
            .iter()
            .filter(|(root, _)| root.as_path() == dir)
            .count();
        VersionControlFacts {
            repository,
            pending,
        }
    }

    /// Why "Commit every save" may not be turned on for `project_id` now.
    pub(crate) fn version_control_refusal(
        &self,
        project_id: ProjectId,
        on: bool,
    ) -> Option<String> {
        let project = self.document_state.project_by_id(project_id)?;
        let lacks = self.version_control_facts(project).repository == Repository::Absent;
        (on && lacks && !project.data.enable_git).then(|| {
            "the project folder has no Git repository: run Enable Version Control first".to_string()
        })
    }

    /// The Commit every save check: one General step.
    pub(super) fn toggle_project_version_control(&mut self, project_id: ProjectId, on: bool) {
        if let Some(reason) = self.version_control_refusal(project_id, on) {
            crate::diagnostics::log_warning(format!("Project tab: {reason}"));
            return;
        }
        let command = signex_engine::ProjectSectionCommand::SetVersionControl { on };
        if let Err(problem) = self.apply_project_section(project_id, &command) {
            crate::diagnostics::log_warning(format!("Project tab: {problem}"));
        }
    }

    /// Enable Version Control…: the project tree's dialog for this project.
    pub(super) fn open_version_control_from_tab(&mut self, project_id: ProjectId) {
        match self
            .document_state
            .projects
            .iter()
            .position(|p| p.id == project_id)
        {
            Some(idx) => self.open_enable_version_control_dialog(vec![idx]),
            None => crate::diagnostics::log_warning(
                "Project tab: the project was closed; nothing to put under version control",
            ),
        }
    }
}
