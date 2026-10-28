//! The machine's ERC rules (#1608): what a project that stores no rules of its
//! own runs with, and what the ERC dialog edits when no project is open.
//!
//! They live in the `User Default` rule profile, the same store as every other
//! profile, not in `prefs.json`. `UiState::erc_severity_override` and
//! `pin_matrix_overrides` are the in-memory copy the ERC run and the dialog
//! read; every edit updates them and queues a write of the profile. File IO
//! runs in a `Task`, one write at a time (see `ProfileWriteQueue`).

use std::path::Path;
use std::sync::Arc;

use iced::Task;
use signex_erc::project_rules::profile::adopt_profile;
use signex_types::project::RuleProfile;

use super::super::super::state::rule_profile_ui::QueuedWrite;
use super::super::super::*;
use super::rule_profiles::blocking;
use crate::rule_profiles::{self, ProfileKind, USER_DEFAULT_PROFILE_NAME};

/// Run one queued write against `dir`. The body of the worker task, apart
/// from the Task plumbing so tests can run it on a temp folder.
pub(crate) fn execute_profile_write(dir: &Path, write: QueuedWrite) -> Result<(), String> {
    match write {
        QueuedWrite::UserDefault(erc) => rule_profiles::write_user_default(dir, erc),
        QueuedWrite::Profile(profile) => rule_profiles::write_profile_in_place(dir, &profile),
    }
}

impl Signex {
    /// Whether the machine's rules may be edited now. They may not while the
    /// boot task is still reading them: a write would store defaults over the
    /// user's file.
    fn machine_rules_editable(&self) -> bool {
        if self.ui_state.machine_rules_loading {
            crate::diagnostics::log_warning(
                "The machine's ERC rules are still loading from the User Default profile; \
                 try the change again in a moment.",
            );
            return false;
        }
        true
    }

    /// Set one rule's severity in the machine's rules and queue the write.
    pub(crate) fn set_machine_severity(
        &mut self,
        rule: signex_erc::RuleKind,
        severity: signex_erc::Severity,
    ) -> Task<Message> {
        if !self.machine_rules_editable() {
            return Task::none();
        }
        // Store the choice verbatim: a rule that grades its own findings
        // (BadHierSheetPin) must be flattenable to its default level too. The
        // one exception is an on/off-only row (Pin type conflict), whose
        // severities come from the pin matrix.
        self.ui_state
            .erc_severity_override
            .insert(rule, rule.normalize_override(severity));
        self.persist_machine_rules()
    }

    /// Cycle one pin-matrix cell Error -> Warning -> Info -> Off -> Error in
    /// the machine's rules and queue the write. A pair of pin types is
    /// unordered, so the mirrored cell is written with it: the dialog then
    /// never shows two different verdicts for the same pair. A value equal to
    /// the baseline drops the override instead of storing it.
    pub(crate) fn cycle_machine_pin_cell(&mut self, row: u8, col: u8) -> Task<Message> {
        use signex_erc::pin_matrix::baseline_cell;
        if !self.machine_rules_editable() {
            return Task::none();
        }
        let current = self
            .ui_state
            .pin_matrix_overrides
            .get(&(row, col))
            .copied()
            .unwrap_or_else(|| baseline_cell(row, col));
        let next = current.cycled();
        for key in [(row, col), (col, row)] {
            if next == baseline_cell(key.0, key.1) {
                self.ui_state.pin_matrix_overrides.remove(&key);
            } else {
                self.ui_state.pin_matrix_overrides.insert(key, next);
            }
        }
        self.persist_machine_rules()
    }

    /// Queue a write of the in-memory machine rules to the `User Default`
    /// profile, and show them in an open Preferences profile list.
    fn persist_machine_rules(&mut self) -> Task<Message> {
        let erc = signex_erc::project_rules::seed_from_overrides(
            &self.ui_state.erc_severity_override,
            &self.ui_state.pin_matrix_overrides,
        );
        if let Some(entry) = self
            .ui_state
            .profile_manager
            .listing
            .profiles
            .iter_mut()
            .find(|l| l.kind == ProfileKind::UserDefault)
        {
            entry.profile.erc = adopt_profile(&entry.profile.erc, &erc);
        }
        self.submit_profile_write(USER_DEFAULT_PROFILE_NAME, QueuedWrite::UserDefault(erc))
    }

    /// Hand `write` to the queue; the returned Task is the worker when the
    /// queue was idle, else nothing (the write waits its turn).
    pub(crate) fn submit_profile_write(&mut self, key: &str, write: QueuedWrite) -> Task<Message> {
        if rule_profiles::profiles_dir().is_none() {
            crate::diagnostics::log_warning(
                "This platform has no config folder for rule profiles, so the change is kept \
                 for this session only.",
            );
            return Task::none();
        }
        match self.ui_state.profile_writes.submit(key, write) {
            Some(write) => run_profile_write(write),
            None => Task::none(),
        }
    }

    pub(crate) fn handle_profile_write_finished(
        &mut self,
        result: &Result<(), String>,
    ) -> Task<Message> {
        if let Err(why) = result {
            crate::diagnostics::log_warning(why);
        }
        match self.ui_state.profile_writes.finished() {
            Some(next) => run_profile_write(next),
            None => Task::none(),
        }
    }

    /// Boot task, after the migration: read the `User Default` profile into
    /// the in-memory machine rules.
    pub(crate) fn load_user_default_task() -> Task<Message> {
        let Some(dir) = rule_profiles::profiles_dir() else {
            return Task::done(Message::Erc(ErcMsg::UserDefaultLoaded(Arc::new(Ok(None)))));
        };
        blocking(
            move || rule_profiles::load_user_default(&dir),
            |joined| Message::Erc(ErcMsg::UserDefaultLoaded(Arc::new(joined.and_then(|r| r)))),
        )
    }

    pub(crate) fn handle_user_default_loaded(
        &mut self,
        result: &Result<Option<RuleProfile>, String>,
    ) -> Task<Message> {
        self.ui_state.machine_rules_loading = false;
        match result {
            Ok(Some(profile)) => {
                let resolved = signex_erc::project_rules::resolve(&profile.erc);
                self.ui_state.erc_severity_override = resolved.severity;
                self.ui_state.pin_matrix_overrides = resolved.pin_matrix;
                if !resolved.unrecognised.is_empty() {
                    crate::diagnostics::log_info(format!(
                        "The User Default rule profile holds {} entries from a newer Signex; \
                         they are kept in the file and ignored here.",
                        resolved.unrecognised.len()
                    ));
                }
            }
            Ok(None) => {}
            // The file stays as it is: a later edit refuses to replace an
            // unreadable profile, so the user's file is never overwritten.
            Err(why) => crate::diagnostics::log_warning(format!(
                "The machine's ERC rules could not be read, so this session uses the built-in \
                 defaults and the file is left untouched: {why}"
            )),
        }
        Task::none()
    }
}

fn run_profile_write(write: QueuedWrite) -> Task<Message> {
    let Some(dir) = rule_profiles::profiles_dir() else {
        return Task::done(Message::Erc(ErcMsg::ProfileWriteFinished(Arc::new(Ok(())))));
    };
    blocking(
        move || execute_profile_write(&dir, write),
        |joined| {
            Message::Erc(ErcMsg::ProfileWriteFinished(Arc::new(
                joined.and_then(|r| r),
            )))
        },
    )
}
