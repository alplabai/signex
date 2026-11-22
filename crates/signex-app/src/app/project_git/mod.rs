//! A project's repository as the app shows it (mockup rows 35 and 39): the
//! branch, the last commit, the commit author and the branch switcher the
//! Project tab and the History panel share. The state and message live here,
//! so both surfaces show and drive one switcher.
//!
//! All git work runs off the update thread. `switch` is the one branch switch
//! both surfaces call. Unsaved edits stop it: Signex saves only when the user
//! presses Save and switch, which a branch with another project file offers.

mod reopen;
mod switch;
mod update;
pub(crate) mod view;

#[cfg(test)]
mod switch_tests;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::PathBuf;

use signex_library::adapters::local_git::project_repo::{RepoFacts, SwitchRefusal};

pub use crate::app::core::branch_switch::{SwitchRequest, SwitchSurface};
use crate::app::state::ProjectId;
use crate::app::state::sheet_refs::SheetRefs;

/// The branch glyph, 24 x 24 stroke art tinted when shown; the Project tab's
/// glyph set reads the same source.
pub(crate) const BRANCH_GLYPH: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="7" cy="5" r="2"/><circle cx="7" cy="19" r="2"/><circle cx="17" cy="7" r="2"/><path d="M7 7v10M17 9c0 4-10 2-10 6"/></svg>"##;

/// Why the last branch switch did not happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwitchBlock {
    /// A document of the project has unsaved edits; the file's name.
    UnsavedEdits(String),
    /// A save, restore, commit or folder move is still running.
    WorkRunning,
    /// The repository refused it.
    Refused(SwitchRefusal),
}

/// A switch that landed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchDone {
    pub root: PathBuf,
    pub branch: String,
    /// Every path it changed, relative to `root`.
    pub changed: Vec<PathBuf>,
    /// The open projects' `.snxprj` files among them, still to reopen.
    pub reopen: Vec<PathBuf>,
}

/// Why the typed author was not written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorProblem {
    /// Not `Name <email>`.
    Unreadable,
    /// git failed; the system's reason.
    WriteFailed(String),
}

/// One project's repository as last read, and the edits open on it.
#[derive(Debug, Clone, Default)]
pub struct RepoView {
    pub(crate) facts: Option<RepoFacts>,
    /// Why the last read failed; cleared by the next good one.
    pub(crate) read_error: Option<String>,
    /// The newest read started; an older one's result is dropped.
    generation: u64,
    is_reading: bool,
    /// The branch a running switch is checking out.
    pub(crate) switching: Option<String>,
    pub(crate) block: Option<SwitchBlock>,
    /// The author as typed, until Enter writes it.
    pub(crate) author_draft: Option<String>,
    pub(crate) author_problem: Option<AuthorProblem>,
}

impl RepoView {
    /// What the Author field shows: the text being typed, else the identity
    /// git resolves for this repository.
    pub(crate) fn author_text(&self) -> &str {
        self.author_draft
            .as_deref()
            .or_else(|| self.facts.as_ref()?.author.as_deref())
            .unwrap_or_default()
    }
}

/// Every project's repository view, and the one open switcher.
#[derive(Debug, Clone, Default)]
pub struct ProjectGitState {
    repos: HashMap<ProjectId, RepoView>,
    switcher: Option<ProjectId>,
    /// A switch waiting on Save and switch.
    save_offer: Option<SwitchRequest>,
    /// A switch whose save landed and whose commit is still running.
    parked: Option<SwitchRequest>,
}

impl ProjectGitState {
    /// Whether the Project tab of `project` offers Save and switch.
    pub(crate) fn offers_save_and_switch(&self, project: ProjectId) -> bool {
        self.save_offer
            .as_ref()
            .is_some_and(|offer| offer.surface == SwitchSurface::ProjectTab(project))
    }

    /// `project`'s repository as last read, if it has been.
    pub(crate) fn repo(&self, project: ProjectId) -> Option<&RepoView> {
        self.repos.get(&project)
    }

    /// Whether the branch switcher is open for `project`.
    pub(crate) fn is_switcher_open(&self, project: ProjectId) -> bool {
        self.switcher == Some(project)
    }

    /// Close the switcher, whichever project it is open for.
    pub(crate) fn close_switcher(&mut self) {
        self.switcher = None;
    }

    fn repo_mut(&mut self, project: ProjectId) -> &mut RepoView {
        self.repos.entry(project).or_default()
    }
}

/// What happened to a project's repository view.
#[derive(Debug, Clone)]
pub enum ProjectGitMsg {
    /// Read the repository again.
    RepoReadRequested { project_id: ProjectId },
    /// A read finished.
    RepoRead {
        project_id: ProjectId,
        generation: u64,
        result: Result<RepoFacts, String>,
    },
    /// The branch button was pressed: the switcher opens or closes.
    SwitcherToggled { project_id: ProjectId },
    /// A branch was picked in the switcher.
    BranchPicked { project_id: ProjectId, name: String },
    /// A switch finished.
    BranchSwitched {
        project_id: ProjectId,
        result: Result<SwitchDone, SwitchRefusal>,
    },
    /// The read of whether the picked branch has another project file.
    SwitchChecked {
        request: Box<SwitchRequest>,
        result: Result<bool, SwitchRefusal>,
    },
    /// Save and switch was pressed.
    SaveAndSwitchPressed,
    /// The sheet references for reopening a switched project were read.
    ReopenRefsResolved {
        done: Box<SwitchDone>,
        result: Result<Box<SheetRefs>, String>,
    },
    /// The Author field as typed.
    AuthorEdited { project_id: ProjectId, text: String },
    /// Enter in the Author field: the typed author goes to git config.
    AuthorSubmitted { project_id: ProjectId },
    /// The git config write finished.
    AuthorWritten {
        project_id: ProjectId,
        result: Result<(), String>,
    },
}

impl From<ProjectGitMsg> for crate::app::Message {
    fn from(msg: ProjectGitMsg) -> Self {
        Self::Project(crate::app::ProjectMsg::Git(msg))
    }
}
