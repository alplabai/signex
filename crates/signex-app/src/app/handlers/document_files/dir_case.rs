//! Identity case rule of a loose document's directory (#1447).
//!
//! Open and Ctrl+S decide "is this the file a document already holds" with
//! [`Signex::identity_fold_for`]; the answer comes from a probe of the
//! document's directory that runs off the update thread. See
//! `state/dir_case.rs` for the cache and `state/project_case.rs` for why
//! identity and netlist keys differ on an unknown volume.

use std::path::{Path, PathBuf};

use iced::Task;

use crate::app::state::project_case::{
    CaseProbeOutcome, probe_case_outcome, undecided_identity_fold, undecided_message,
};
use crate::app::state::scope::{CaseFold, path_key_with};
use crate::app::{FileMsg, Message, Signex};

/// What is known about the identity rule of a path.
enum IdentityRule {
    /// A probe decided it.
    Decided(CaseFold),
    /// A probe ran and could not decide; treated as "one file".
    Assumed,
    /// No probe has answered yet.
    Unknown,
}

/// An identity rule and whether a probe decided it.
#[derive(Clone, Copy)]
pub(crate) struct FoldAnswer {
    pub(crate) fold: CaseFold,
    pub(crate) decided: bool,
}

impl Signex {
    /// The one lookup behind every identity answer: the owning project's
    /// rule once its own probe decided, else the directory's probe.
    fn identity_rule(&self, path: &Path) -> IdentityRule {
        if let Some(project) = self.project_owning(path)
            && project.case_decided
        {
            return IdentityRule::Decided(project.case_fold);
        }
        let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) else {
            return IdentityRule::Assumed;
        };
        match self.document_state.dir_case.answer(dir) {
            None => IdentityRule::Unknown,
            Some(CaseProbeOutcome::Decided(ignores_case)) => {
                IdentityRule::Decided(CaseFold::from_probe(*ignores_case))
            }
            Some(_) => IdentityRule::Assumed,
        }
    }

    /// The identity rule for `path` when it is known: the owning project's
    /// once its own probe landed, else its directory's probe. `None` while
    /// neither has answered. An undecided probe answers "folds".
    pub(crate) fn identity_answer(&self, path: &Path) -> Option<CaseFold> {
        match self.identity_rule(path) {
            IdentityRule::Decided(fold) => Some(fold),
            IdentityRule::Assumed => Some(undecided_identity_fold()),
            IdentityRule::Unknown => None,
        }
    }

    /// The rule that says whether two spellings of `path` are one file. Until
    /// it is known the answer is "yes": two documents for one file can lose
    /// edits, one document for two files only costs a tab. Not the netlist key
    /// rule, which is [`Self::fold_for_path`].
    pub(crate) fn identity_fold_for(&self, path: &Path) -> CaseFold {
        self.identity_decision(path).fold
    }

    /// [`Self::identity_fold_for`] plus whether a probe DECIDED it or it was
    /// only ASSUMED. A save that writes one spelling may clear the other
    /// spellings' dirty markers only when decided: under an assumed fold
    /// they can be separate files that were never written.
    pub(crate) fn identity_decision(&self, path: &Path) -> FoldAnswer {
        match self.identity_rule(path) {
            IdentityRule::Decided(fold) => FoldAnswer {
                fold,
                decided: true,
            },
            _ => FoldAnswer {
                fold: undecided_identity_fold(),
                decided: false,
            },
        }
    }

    /// Store the directory probes a Save All just ran, so the plan's writes
    /// decide identity through [`Self::identity_fold_for`] like open and
    /// Ctrl+S. An undecided one is reported once, as open does.
    pub(crate) fn record_save_all_probes(
        &mut self,
        decided: &std::collections::HashMap<PathBuf, bool>,
        undecided: &[(PathBuf, CaseProbeOutcome)],
    ) {
        let answers = decided
            .iter()
            .map(|(dir, ignores_case)| (dir, CaseProbeOutcome::Decided(*ignores_case)))
            .chain(
                undecided
                    .iter()
                    .map(|(dir, outcome)| (dir, outcome.clone())),
            );
        for (dir, outcome) in answers {
            if self.document_state.dir_case.record(dir, outcome.clone())
                && !matches!(outcome, CaseProbeOutcome::Decided(_))
            {
                crate::diagnostics::log_warning(undecided_message(
                    &outcome,
                    dir,
                    undecided_identity_fold(),
                ));
            }
        }
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

    /// Start the identity probe of every open document's directory that has
    /// none yet, so the routes that cannot wait for it (Navigator, ERC,
    /// project tree, descend) fold only for the milliseconds a probe takes.
    pub(crate) fn warm_document_dir_probes(&mut self) -> Task<Message> {
        let unknown: Vec<PathBuf> = self
            .document_state
            .tabs
            .iter()
            .filter(|tab| {
                use crate::app::TabKind;
                matches!(
                    tab.kind,
                    TabKind::Schematic
                        | TabKind::Pcb
                        | TabKind::SymbolEditor(_)
                        | TabKind::FootprintEditor(_)
                )
            })
            .map(|tab| tab.path.clone())
            .filter(|path| self.identity_answer(path).is_none())
            .collect();
        Task::batch(
            unknown
                .iter()
                .map(|path| self.probe_dir_case_if_unknown(path))
                .collect::<Vec<_>>(),
        )
    }

    /// A directory's probe landed: store it. First answer wins.
    pub(crate) fn handle_dir_case_probed(
        &mut self,
        dir: PathBuf,
        answer: CaseProbeOutcome,
    ) -> Task<Message> {
        if self.document_state.dir_case.record(&dir, answer.clone()) {
            self.report_dir_case_answer(&dir, &answer);
        }
        let mut tasks: Vec<Task<Message>> = self
            .document_state
            .dir_case
            .take_deferred_opens(&dir)
            .into_iter()
            .map(|request| self.reopen_deferred(request))
            .collect();
        for path in self.document_state.dir_case.take_deferred_saves(&dir) {
            tasks.push(self.resume_deferred_save(path));
        }
        Task::batch(tasks)
    }

    /// What Ctrl+S does about other spellings of the file it is about to
    /// write. One spelling held: nothing to guard. Several, and the
    /// directory's rule unknown: probe first and save when it answers, so a
    /// save is not refused for a volume that keeps the spellings apart.
    /// Several under a folding rule: write only if they all encode alike,
    /// else refuse (logged, both documents stay dirty).
    pub(crate) fn guard_spelling_save(&mut self, path: &Path) -> anyhow::Result<SaveGuard> {
        let held = self.held_spellings(path, self.identity_fold_for(path));
        if held.len() < 2 {
            return Ok(SaveGuard::Clear);
        }
        if self.identity_answer(path).is_none() {
            let probe = self.probe_dir_case_if_unknown(path);
            self.document_state.dir_case.defer_save(path.to_path_buf());
            return Ok(SaveGuard::Waiting(probe));
        }
        self.ensure_variants_agree(&held)?;
        Ok(SaveGuard::Clear)
    }

    /// Run a Ctrl+S that waited for its directory's rule, if that document is
    /// still the one in front of the user.
    fn resume_deferred_save(&mut self, path: PathBuf) -> Task<Message> {
        if self.document_state.active_path.as_deref() != Some(path.as_path()) {
            let message = save_skipped_message(&path);
            crate::diagnostics::log_warning(&message);
            if self.document_state.error_notice.is_none() {
                self.document_state.error_notice =
                    Some(crate::app::state::ErrorNotice::save(message));
            }
            return Task::none();
        }
        self.handle_active_document_save_requested()
    }

    /// Say what a first answer for `dir` means for the user: an undecided
    /// probe was assumed to fold, and documents already open as two spellings
    /// of one file are now known to alias. Neither is merged or closed; the
    /// save guard keeps both safe.
    fn report_dir_case_answer(&self, dir: &Path, answer: &CaseProbeOutcome) {
        if !matches!(answer, CaseProbeOutcome::Decided(_)) {
            crate::diagnostics::log_warning(undecided_message(
                answer,
                dir,
                undecided_identity_fold(),
            ));
        }
        let mut by_file: std::collections::BTreeMap<String, Vec<&Path>> = Default::default();
        for held in self
            .document_state
            .engines
            .keys()
            .filter(|held| held.parent() == Some(dir))
        {
            let key = path_key_with(held, self.identity_fold_for(held));
            by_file.entry(key).or_default().push(held);
        }
        for mut spellings in by_file.into_values().filter(|group| group.len() > 1) {
            spellings.sort();
            crate::diagnostics::log_warning(aliased_documents_message(&spellings));
        }
    }

    /// Run an open that waited for its directory's rule.
    fn reopen_deferred(&mut self, request: PathBuf) -> Task<Message> {
        self.document_state.pending_opens.remove_untracked(&request);
        match self.open_document_path(request) {
            Ok(task) => task,
            Err(error) => {
                crate::diagnostics::log_error("Failed to open document path", &error);
                self.document_state.error_notice =
                    Some(crate::app::state::ErrorNotice::open(format!("{error:#}")));
                Task::none()
            }
        }
    }
}

/// Messages keeps about 160 bytes of a line, so the advice leads and the
/// paths, which can be long, come last.
pub(super) fn aliased_documents_message(spellings: &[&Path]) -> String {
    let names = spellings
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(" and ");
    format!(
        "One file open as two documents; saving is refused while they differ, close one: {names}"
    )
}

/// A Ctrl+S that waited for its folder's probe and found another document
/// focused: nothing was written.
pub(super) fn save_skipped_message(path: &Path) -> String {
    format!(
        "Nothing was saved: focus moved while the folder was checked, press Ctrl+S again: {}",
        path.display()
    )
}

/// The verdict of [`Signex::guard_spelling_save`].
pub(crate) enum SaveGuard {
    /// Write the active document.
    Clear,
    /// Hold the write until the directory's probe (this task) has answered.
    Waiting(Task<Message>),
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
