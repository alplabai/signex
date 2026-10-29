//! Rule profiles in the ERC flow (#1085): "Save as profile", "Update from
//! profile" with a confirmed diff, the drift note, and the first-run move of
//! the preferences into the user's default profile.
//!
//! All file IO runs in a `Task` (`spawn_blocking`); the handlers here only
//! read and mutate memory. Nothing replaces a project's rules except
//! [`Signex::handle_profile_update_confirmed`], after the user has seen the
//! diff.

use std::sync::Arc;

use iced::Task;
use signex_erc::project_rules::profile::{
    RuleChange, adopt_profile, diff_against_profile, has_drifted, profile_ref,
};
use signex_types::project::RuleProfileRef;

use super::super::super::state::PendingProfileUpdate;
use super::super::super::state::rule_profile_ui::SaveProfileDialog;
use super::super::super::*;
use crate::rule_profiles::{self, ProfileMigration, ResolvedProfile, SaveOutcome};

type Loaded = Arc<Result<Option<ResolvedProfile>, String>>;

/// Run blocking `work` off the update thread and map its result to a message.
pub(crate) fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
    done: impl FnOnce(Result<T, String>) -> Message + Send + 'static,
) -> Task<Message> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(work)
                .await
                .map_err(|e| format!("profile worker failed: {e}"))
        },
        done,
    )
}

impl Signex {
    /// The active project's profile provenance record, when it stores rules.
    fn active_profile_ref(&self) -> Option<(ProjectId, RuleProfileRef)> {
        let project = self.document_state.active_document_project()?;
        let reference = project.data.rules.as_ref()?.profile.clone()?;
        Some((project.id, reference))
    }

    /// Whether the ERC dialog can offer "Update from profile": the project
    /// stores rules that record the profile they came from.
    pub(crate) fn active_project_has_profile(&self) -> bool {
        self.active_profile_ref().is_some()
    }

    /// Default name offered by "Save as profile".
    pub(crate) fn default_profile_name(&self) -> String {
        let project = self
            .document_state
            .active_document_project()
            .map_or("Project", |p| p.data.name.as_str());
        format!("{project} rules")
    }

    /// Which rule set an ERC run started now would use: the project's own
    /// (with its recorded profile name) or this machine's preferences.
    pub(crate) fn current_rule_set_identity(&self) -> signex_output::ErcReportRuleSet {
        let stored = self
            .document_state
            .active_document_project()
            .and_then(|p| p.data.rules.as_ref());
        match stored {
            Some(rules) => {
                let sha256 = rules.erc.sha256_hex();
                let profile_name = rules
                    .profile
                    .as_ref()
                    .filter(|r| !r.name.is_empty())
                    .map(|r| {
                        // The record names where the rules came from; only a
                        // matching hash says they still are that profile.
                        if r.sha256.is_empty() || r.sha256 == sha256 {
                            r.name.clone()
                        } else {
                            format!("{} (modified)", r.name)
                        }
                    });
                signex_output::ErcReportRuleSet {
                    profile_name,
                    sha256,
                }
            }
            None => signex_output::ErcReportRuleSet {
                profile_name: None,
                sha256: signex_erc::project_rules::seed_from_overrides(
                    &self.ui_state.erc_severity_override,
                    &self.ui_state.pin_matrix_overrides,
                )
                .sha256_hex(),
            },
        }
    }

    // ---- Save as profile -------------------------------------------------

    /// Open the Save-as dialog with the default name and version. Writes
    /// nothing.
    pub(crate) fn handle_profile_save_dialog_opened(&mut self) -> Task<Message> {
        let Some((project, true)) = self
            .document_state
            .active_document_project()
            .map(|p| (p.id, p.data.rules.is_some()))
        else {
            crate::diagnostics::log_warning(
                "Save as profile: this project stores no ERC rules yet. Store them first.",
            );
            return Task::none();
        };
        self.ui_state.save_profile_dialog =
            Some(SaveProfileDialog::new(project, self.default_profile_name()));
        Task::none()
    }

    pub(crate) fn handle_profile_save_name_changed(&mut self, name: String) -> Task<Message> {
        if let Some(dialog) = self
            .ui_state
            .save_profile_dialog
            .as_mut()
            .filter(|d| !d.busy)
        {
            dialog.name = name;
            dialog.error = None;
            dialog.confirm_overwrite = false;
        }
        Task::none()
    }

    pub(crate) fn handle_profile_save_version_changed(&mut self, version: String) -> Task<Message> {
        if let Some(dialog) = self
            .ui_state
            .save_profile_dialog
            .as_mut()
            .filter(|d| !d.busy)
        {
            dialog.version = version;
            dialog.error = None;
            dialog.confirm_overwrite = false;
        }
        Task::none()
    }

    /// Esc / Cancel: from the overwrite question back to the inputs, from the
    /// inputs out of the dialog. Nothing is written either way.
    pub(crate) fn handle_profile_save_cancelled(&mut self) -> Task<Message> {
        match self.ui_state.save_profile_dialog.as_mut() {
            Some(dialog) if dialog.confirm_overwrite => dialog.confirm_overwrite = false,
            _ => self.ui_state.save_profile_dialog = None,
        }
        Task::none()
    }

    pub(crate) fn handle_profile_save_submitted(&mut self) -> Task<Message> {
        self.start_profile_save(false)
    }

    pub(crate) fn handle_profile_save_overwrite_confirmed(&mut self) -> Task<Message> {
        self.start_profile_save(true)
    }

    /// Validate the dialog and write on a worker. `overwrite` is honoured only
    /// after the dialog has asked, so a stray message cannot replace a file.
    fn start_profile_save(&mut self, overwrite: bool) -> Task<Message> {
        let Some(dialog) = self.ui_state.save_profile_dialog.as_mut() else {
            return Task::none();
        };
        if dialog.busy || (overwrite && !dialog.confirm_overwrite) {
            return Task::none();
        }
        let (name, version) =
            match rule_profiles::validate_profile_input(&dialog.name, &dialog.version) {
                Ok(valid) => valid,
                Err(why) => {
                    dialog.error = Some(why);
                    return Task::none();
                }
            };
        let project = dialog.project;
        let Some(dir) = rule_profiles::profiles_dir() else {
            dialog.error = Some("This platform has no config folder for rule profiles.".into());
            return Task::none();
        };
        let Some(erc) = self
            .document_state
            .active_document_project()
            .filter(|p| p.id == project)
            .and_then(|p| p.data.rules.as_ref())
            .map(|r| r.erc.clone())
        else {
            self.ui_state.save_profile_dialog = None;
            crate::diagnostics::log_warning(
                "Save as profile: the project it was opened for no longer stores ERC rules.",
            );
            return Task::none();
        };
        if let Some(dialog) = self.ui_state.save_profile_dialog.as_mut() {
            dialog.busy = true;
            dialog.error = None;
        }
        blocking(
            move || rule_profiles::save_named_profile(&dir, &name, &version, erc, overwrite),
            move |joined| {
                Message::Erc(ErcMsg::ProfileSaved {
                    project,
                    result: Arc::new(joined.and_then(|r| r)),
                })
            },
        )
    }

    /// Show the outcome. A taken name opens the overwrite question; a failure
    /// stays in the dialog; a write closes it and records the written profile
    /// as the project's provenance in one undoable edit, so Update from
    /// profile and the drift note have something to compare against.
    pub(crate) fn handle_profile_saved(
        &mut self,
        project: ProjectId,
        result: &Result<SaveOutcome, String>,
    ) -> Task<Message> {
        if let Some(dialog) = self.ui_state.save_profile_dialog.as_mut() {
            dialog.busy = false;
        }
        let saved = match result {
            Ok(SaveOutcome::Saved(saved)) => saved,
            Ok(SaveOutcome::Exists) => {
                if let Some(dialog) = self.ui_state.save_profile_dialog.as_mut() {
                    dialog.confirm_overwrite = true;
                }
                return Task::none();
            }
            Err(why) => {
                if let Some(dialog) = self.ui_state.save_profile_dialog.as_mut() {
                    dialog.error = Some(why.clone());
                }
                crate::diagnostics::log_warning(why);
                return Task::none();
            }
        };
        self.ui_state.save_profile_dialog = None;
        crate::diagnostics::log_info(format!("Saved ERC rules as profile {}.", saved.summary));
        // An open Rule Profiles pane holds a listing from before this write
        // (an overwrite changed a profile it shows); read it again.
        let relist = if self.ui_state.profile_manager.loaded {
            Self::list_profiles_task()
        } else {
            Task::none()
        };
        let Some(mut rules) = self
            .document_state
            .active_document_project()
            .filter(|p| p.id == project)
            .and_then(|p| p.data.rules.clone())
        else {
            crate::diagnostics::log_warning(
                "The profile was saved, but the project it came from is no longer the active \
                 one, so the project does not record it.",
            );
            return relist;
        };
        if rules.profile.as_ref() != Some(&saved.reference) {
            rules.profile = Some(saved.reference.clone());
            self.replace_project_rules(Some(rules), true);
        }
        relist
    }

    // ---- Update from profile ----------------------------------------------

    fn load_profile_task(
        &self,
        reference: RuleProfileRef,
        done: impl FnOnce(Loaded) -> Message + Send + 'static,
    ) -> Task<Message> {
        let dir = rule_profiles::profiles_dir();
        blocking(
            move || rule_profiles::resolve_profile(dir.as_deref(), &reference),
            move |joined| done(Arc::new(joined.and_then(|r| r))),
        )
    }

    pub(crate) fn handle_profile_update_requested(&mut self) -> Task<Message> {
        let Some((project, reference)) = self.active_profile_ref() else {
            return Task::none();
        };
        self.load_profile_task(reference, move |result| {
            Message::Erc(ErcMsg::ProfileLoadedForUpdate { project, result })
        })
    }

    pub(crate) fn handle_profile_loaded_for_update(
        &mut self,
        project: ProjectId,
        result: &Loaded,
    ) -> Task<Message> {
        // The user may have switched projects while the file was read.
        let Some(current) = self.document_state.active_document_project() else {
            return Task::none();
        };
        let Some(rules) = current
            .data
            .rules
            .as_ref()
            .filter(|_| current.id == project)
        else {
            return Task::none();
        };
        let (profile, source) = match result.as_ref() {
            Ok(Some(found)) => found.clone(),
            Ok(None) => {
                crate::diagnostics::log_warning(
                    "Update from profile: the profile this project was seeded from is not in \
                     this machine's profile library.",
                );
                return Task::none();
            }
            Err(why) => {
                crate::diagnostics::log_warning(format!("Update from profile: {why}"));
                return Task::none();
            }
        };
        let changes = diff_against_profile(&rules.erc, &profile.erc);
        let recorded = rules.profile.as_ref().map(|r| r.sha256.as_str());
        if changes.is_empty() && recorded == Some(profile.content_sha256().as_str()) {
            crate::diagnostics::log_info(format!(
                "Project ERC rules already match profile \"{}\".",
                profile.name
            ));
            return Task::none();
        }
        self.ui_state.pending_profile_update = Some(PendingProfileUpdate {
            project,
            profile,
            source,
            changes,
        });
        self.refresh_panel_ctx();
        Task::none()
    }

    /// Drop the pending update when it was prepared for a project other than
    /// the active one (a project switch), so the panel never shows project
    /// A's diff while B is active.
    pub(crate) fn drop_pending_profile_update_for_other_project(&mut self) {
        let active = self.document_state.active_document_project().map(|p| p.id);
        if self
            .ui_state
            .pending_profile_update
            .as_ref()
            .is_some_and(|pending| Some(pending.project) != active)
        {
            self.ui_state.pending_profile_update = None;
        }
        if self
            .ui_state
            .save_profile_dialog
            .as_ref()
            .is_some_and(|dialog| Some(dialog.project) != active)
        {
            self.ui_state.save_profile_dialog = None;
        }
    }

    /// Drop the pending update because the rules changed under it: its diff
    /// no longer describes what Apply would do. Tells the user, since the
    /// panel disappears.
    pub(super) fn invalidate_pending_profile_update(&mut self) {
        if let Some(pending) = self.ui_state.pending_profile_update.take() {
            crate::diagnostics::log_info(format!(
                "The comparison with profile \"{}\" was closed because the project's ERC \
                 rules changed. Choose Update from profile again to review the new differences.",
                pending.profile.name
            ));
        }
    }

    pub(crate) fn handle_profile_update_cancelled(&mut self) -> Task<Message> {
        self.ui_state.pending_profile_update = None;
        Task::none()
    }

    /// Apply the confirmed update as a single undo step. Any rules change,
    /// undo or project switch since the diff was shown has already dropped
    /// the pending update, so what is applied here is exactly what was shown.
    pub(crate) fn handle_profile_update_confirmed(&mut self) -> Task<Message> {
        let Some(pending) = self.ui_state.pending_profile_update.take() else {
            return Task::none();
        };
        let Some(mut rules) = self
            .document_state
            .active_document_project()
            .filter(|p| p.id == pending.project)
            .and_then(|p| p.data.rules.clone())
        else {
            crate::diagnostics::log_warning(format!(
                "Update from profile \"{}\" was not applied: the project it was prepared for \
                 is no longer the active one.",
                pending.profile.name
            ));
            return Task::none();
        };
        // Belt and braces: whatever changed the rules, Apply never writes a
        // result the user was not shown.
        if diff_against_profile(&rules.erc, &pending.profile.erc) != pending.changes {
            crate::diagnostics::log_warning(format!(
                "Update from profile \"{}\" was not applied: the project's ERC rules changed \
                 after the differences were shown. Choose Update from profile again.",
                pending.profile.name
            ));
            return Task::none();
        }
        rules.erc = adopt_profile(&rules.erc, &pending.profile.erc);
        // Apply refreshes the content hash; it does not reclassify where the
        // project's rules came from, so a recorded source (a hand-written
        // `"file"`, say) is kept. Only a record without one takes the
        // resolved profile's.
        let source = rules
            .profile
            .as_ref()
            .map(|r| r.source.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(pending.source);
        rules.profile = Some(profile_ref(&pending.profile, source));
        self.replace_project_rules(Some(rules), true);
        Task::none()
    }

    /// The lines the confirmation panel lists.
    pub(crate) fn pending_profile_update_lines(&self) -> Option<(String, Vec<String>)> {
        let pending = self.ui_state.pending_profile_update.as_ref()?;
        let active = self.document_state.active_document_project().map(|p| p.id);
        if Some(pending.project) != active {
            return None;
        }
        let lines = pending.changes.iter().map(RuleChange::describe).collect();
        Some((pending.profile.name.clone(), lines))
    }

    // ---- Drift ------------------------------------------------------------

    /// Check, off the update thread, whether the profile the active project
    /// was seeded from has changed locally. Cheap no-op for a project that
    /// records no profile or was already told.
    pub(crate) fn check_profile_drift_task(&self) -> Task<Message> {
        match self.document_state.active_document_project() {
            Some(project) => self.check_profile_drift_task_for(project.id),
            None => Task::none(),
        }
    }

    /// [`Self::check_profile_drift_task`] for a named project, not the one the
    /// active tab happens to belong to (a project opened from another's tab).
    pub(crate) fn check_profile_drift_task_for(&self, project: ProjectId) -> Task<Message> {
        let Some(reference) = self
            .document_state
            .project_by_id(project)
            .and_then(|p| p.data.rules.as_ref())
            .and_then(|r| r.profile.clone())
        else {
            return Task::none();
        };
        if self.document_state.profile_drift_noted.contains(&project) {
            return Task::none();
        }
        self.load_profile_task(reference, move |result| {
            Message::Erc(ErcMsg::ProfileDriftChecked { project, result })
        })
    }

    /// One info note when the local profile differs from what the project
    /// recorded. Never touches the project's rules.
    pub(crate) fn handle_profile_drift_checked(
        &mut self,
        project: ProjectId,
        result: &Loaded,
    ) -> Task<Message> {
        let local = match result.as_ref() {
            Ok(Some((local, _))) => local,
            Ok(None) => return Task::none(),
            Err(why) => {
                crate::diagnostics::log_warning(format!(
                    "Cannot check this project's rule profile for changes: {why}"
                ));
                return Task::none();
            }
        };
        let Some(reference) = self
            .document_state
            .project_by_id(project)
            .and_then(|p| p.data.rules.as_ref())
            .and_then(|r| r.profile.as_ref())
        else {
            return Task::none();
        };
        if !has_drifted(reference, local)
            || !self.document_state.profile_drift_noted.insert(project)
        {
            return Task::none();
        }
        crate::diagnostics::log_info(format!(
            "The rule profile \"{}\" has changed since this project's ERC rules were taken from \
             it. Open the ERC settings and choose Update from profile to review the differences; \
             the project's rules are unchanged.",
            local.name
        ));
        Task::none()
    }

    // ---- First-run migration ------------------------------------------------

    /// Boot task: move the preferences' ERC rules into the user's default
    /// profile once.
    pub(crate) fn migrate_default_profile_task() -> Task<Message> {
        let Some(dir) = rule_profiles::profiles_dir() else {
            return Task::none();
        };
        blocking(
            move || {
                let severity = crate::fonts::read_erc_severity_overrides();
                let pin_matrix = crate::fonts::read_pin_matrix_overrides();
                let folded = crate::fonts::read_erc_rules_in_profile_from(
                    crate::fonts::load_prefs_json().as_ref(),
                );
                rule_profiles::migrate_prefs_to_default_profile(
                    &dir,
                    &rule_profiles::PrefsRules {
                        severity: &severity,
                        pin_matrix: &pin_matrix,
                        folded,
                    },
                    &crate::fonts::write_erc_rules_in_profile_marker,
                )
                .map_err(|e| {
                    format!(
                        "Could not merge the ERC preferences into the default rule profile: {e}"
                    )
                })
            },
            |joined| {
                Message::Erc(ErcMsg::DefaultProfileMigrated(Arc::new(
                    joined.and_then(|r| r),
                )))
            },
        )
    }

    /// Log the migration, then read the (possibly just created) `User Default`
    /// profile into the in-memory machine rules.
    ///
    /// A migration that was refused locks the machine's rules for the session:
    /// the old preferences would fold over an edit made now on a later launch.
    pub(crate) fn handle_default_profile_migrated(
        &mut self,
        result: &Result<ProfileMigration, String>,
    ) -> Task<Message> {
        match result {
            Ok(ProfileMigration::Migrated) => crate::diagnostics::log_info(format!(
                "Your ERC preferences were saved as the rule profile \"{}\".",
                rule_profiles::USER_DEFAULT_PROFILE_NAME
            )),
            Ok(ProfileMigration::Folded) => crate::diagnostics::log_info(format!(
                "Your newer ERC preferences were merged into the rule profile \"{}\", which is \
                 now where this machine's ERC rules are kept.",
                rule_profiles::USER_DEFAULT_PROFILE_NAME
            )),
            Ok(_) => {}
            Err(why) => {
                crate::diagnostics::log_warning(why);
                self.ui_state.machine_rules_locked = Some(why.clone());
            }
        }
        Self::load_user_default_task()
    }
}
