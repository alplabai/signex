//! Identity case rule of a loose document's directory (#1447).
//!
//! Open and Ctrl+S decide "is this the file a document already holds" with
//! [`Signex::identity_fold_for`]; the answer comes from a probe of the
//! document's directory that runs off the update thread. See
//! `state/dir_case.rs` for the cache and `state/project_case.rs` for why
//! identity and netlist keys differ on an unknown volume.

use std::path::{Path, PathBuf};

use iced::Task;

use crate::app::state::project_case::{CaseProbeOutcome, probe_case_outcome};
use crate::app::state::scope::CaseFold;
use crate::app::{FileMsg, Message, Signex};

impl Signex {
    /// The identity rule for `path` when it is known: the owning project's
    /// once its own probe landed, else its directory's probe. `None` while
    /// neither has answered. An undecided probe answers "folds".
    pub(crate) fn identity_answer(&self, path: &Path) -> Option<CaseFold> {
        if let Some(project) = self.project_owning(path)
            && project.case_probed
        {
            return Some(project.case_fold);
        }
        let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) else {
            return Some(CaseFold::from_probe(true));
        };
        match self.document_state.dir_case.answer(dir)? {
            CaseProbeOutcome::Decided(ignores_case) => Some(CaseFold::from_probe(*ignores_case)),
            _ => Some(CaseFold::from_probe(true)),
        }
    }

    /// The rule that says whether two spellings of `path` are one file. Until
    /// it is known the answer is "yes": two documents for one file can lose
    /// edits, one document for two files only costs a tab. Not the netlist key
    /// rule, which is [`Self::fold_for_path`].
    pub(crate) fn identity_fold_for(&self, path: &Path) -> CaseFold {
        self.identity_answer(path)
            .unwrap_or_else(|| CaseFold::from_probe(true))
    }

    /// Start the probe of `path`'s directory unless its rule is known or a
    /// probe is already running.
    pub(crate) fn probe_dir_case_if_unknown(&mut self, path: &Path) -> Task<Message> {
        if self.identity_answer(path).is_some() {
            return Task::none();
        }
        match path.parent() {
            Some(dir) if self.document_state.dir_case.begin_probe(dir) => {
                spawn_dir_case_probe(dir.to_path_buf())
            }
            _ => Task::none(),
        }
    }

    /// A directory's probe landed: store it. First answer wins.
    pub(crate) fn handle_dir_case_probed(
        &mut self,
        dir: PathBuf,
        answer: CaseProbeOutcome,
    ) -> Task<Message> {
        self.document_state.dir_case.record(&dir, answer);
        Task::none()
    }
}

/// Probe `dir`'s volume off the update thread.
fn spawn_dir_case_probe(dir: PathBuf) -> Task<Message> {
    let reported = dir.clone();
    Task::perform(probe_case_outcome(dir), move |answer| {
        Message::File(FileMsg::DirCaseProbed {
            dir: reported,
            answer,
        })
    })
}
