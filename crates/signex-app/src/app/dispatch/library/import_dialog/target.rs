//! The folder check behind Next and Import (#3117, #3123): a folder that is
//! absent or empty takes a new import; one that holds an imported library
//! with a clean history is offered as an update; anything else is refused
//! with its reason; before a staged preview is moved in (#3171), anything
//! but a free folder keeps the preview on step 2. The check reads only;
//! the run itself takes the folder (`library_import::output::claim_output`)
//! or, for an update, holds its write lock.

use std::path::{Path, PathBuf};

use super::*;
use crate::library::import_dialog::source::UpdateOffer;
use crate::library_import::ImportError;
use crate::library_import::snapshot::{check_update, find_library_file};

/// What `location` is to an import. Blocking.
pub(crate) fn check_target(location: &Path) -> TargetCheck {
    let path = || location.display().to_string();
    let unreadable = |e: std::io::Error| {
        TargetCheck::Refused(
            Translatable::new(ids::LIBRARY_IMPORT_DIALOG_TARGET_UNREADABLE)
                .arg("path", path())
                .arg("error", e.to_string()),
        )
    };
    let meta = match std::fs::symlink_metadata(location) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return TargetCheck::Free,
        Err(e) => return unreadable(e),
        Ok(meta) => meta,
    };
    if meta.file_type().is_symlink() {
        return TargetCheck::Refused(
            Translatable::new(ids::LIBRARY_IMPORT_DIALOG_TARGET_LINK).arg("path", path()),
        );
    }
    if !meta.is_dir() {
        return TargetCheck::Refused(
            Translatable::new(ids::LIBRARY_IMPORT_DIALOG_TARGET_NOT_FOLDER).arg("path", path()),
        );
    }
    match std::fs::read_dir(location).map(|mut entries| entries.next().is_none()) {
        Ok(true) => TargetCheck::Free,
        Ok(false) => holding_files(location),
        Err(e) => unreadable(e),
    }
}

/// A folder that holds files: an imported library an update can run over
/// (a `.snxlib` in a clean repository), else refused. A library with no
/// history or with uncommitted changes is refused with that reason.
fn holding_files(location: &Path) -> TargetCheck {
    if find_library_file(location).is_err() {
        return TargetCheck::Refused(
            Translatable::new(ids::LIBRARY_IMPORT_DIALOG_TARGET_NOT_EMPTY)
                .arg("path", location.display().to_string()),
        );
    }
    match check_update(location) {
        Ok(check) => TargetCheck::Imported {
            local_commits: check.local_commits,
        },
        Err(e) => TargetCheck::Refused(e.to_translatable()),
    }
}

impl Signex {
    /// The folder check finished. A refused folder sends the dialog back to
    /// step 1 with the reason; an imported library is offered as an update
    /// until the offer is taken, then the run starts as one; a free folder
    /// starts the run as asked.
    pub(super) fn import_target_checked(
        &mut self,
        location: PathBuf,
        check: TargetCheck,
    ) -> Task<Message> {
        let Some(d) = self.library.import_dialog.as_deref_mut() else {
            return Task::none();
        };
        d.checking_target = false;
        let mode = d.pending_run.take();
        // A check the user has moved on from (an edit, Back) starts nothing.
        let from = match mode {
            Some(RunMode::Preview) => Step::Source,
            _ => Step::Preview,
        };
        if location != d.location_path() || d.step != from {
            return Task::none();
        }
        let Some(mode) = mode else {
            return Task::none();
        };
        // Import of a staged preview (#3171): a folder that is no longer
        // free keeps the dialog on step 2 with the preview, for another try.
        if mode == RunMode::Import && d.staged.is_some() {
            let problem = match check {
                TargetCheck::Free => return self.start_dialog_run(mode),
                TargetCheck::Refused(problem) => problem,
                TargetCheck::Imported { .. } => ImportError::TargetChanged {
                    path: location.clone(),
                }
                .to_translatable(),
            };
            d.move_problem = Some(problem);
            return Task::none();
        }
        match check {
            TargetCheck::Refused(problem) => {
                d.update_offer = None;
                self.send_import_back(problem);
                Task::none()
            }
            TargetCheck::Free => {
                // An update taken over a folder that has gone meanwhile is
                // a new import.
                d.update_offer = None;
                self.start_dialog_run(mode)
            }
            TargetCheck::Imported { local_commits } => {
                let accepted = d.update_offer.is_some_and(|offer| offer.accepted);
                d.update_offer = Some(UpdateOffer {
                    local_commits,
                    accepted,
                });
                if !accepted {
                    d.step = Step::Source;
                    d.preview = None;
                    return Task::none();
                }
                if let Some(what) = self.unsaved_in_library(&location) {
                    self.send_import_back(ImportError::UnsavedInApp { what }.to_translatable());
                    return Task::none();
                }
                let run = match mode {
                    RunMode::Preview => RunMode::Preview,
                    RunMode::Import | RunMode::Update | RunMode::Stage => RunMode::Update,
                };
                self.start_dialog_run(run)
            }
        }
    }

    /// Back to step 1 with why the folder cannot take the library.
    fn send_import_back(&mut self, problem: Translatable) {
        self.with_import_dialog(|d| {
            d.target_problem = Some(problem);
            d.step = Step::Source;
            d.preview = None;
        });
    }

    /// Update this library: the offer is taken and Next runs at once.
    pub(super) fn import_update_accepted(&mut self) -> Task<Message> {
        let offered = self.library.import_dialog.as_deref_mut().and_then(|d| {
            let offer = d.update_offer.as_mut()?;
            offer.accepted = true;
            Some(())
        });
        if offered.is_none() {
            return Task::none();
        }
        self.check_import_target(Step::Source, RunMode::Preview)
    }
}
