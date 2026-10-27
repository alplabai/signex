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
use super::super::super::*;
use crate::rule_profiles::{self, ProfileMigration, ResolvedProfile};

type Loaded = Arc<Result<Option<ResolvedProfile>, String>>;

/// Run blocking `work` off the update thread and map its result to a message.
fn blocking<T: Send + 'static>(
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
            Some(rules) => signex_output::ErcReportRuleSet {
                profile_name: rules
                    .profile
                    .as_ref()
                    .map(|r| r.name.clone())
                    .filter(|n| !n.is_empty()),
                sha256: rules.erc.sha256_hex(),
            },
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

    pub(crate) fn handle_profile_save_requested(&mut self, name: String) -> Task<Message> {
        let name = name.trim().to_string();
        let Some(rules) = self
            .document_state
            .active_document_project()
            .and_then(|p| p.data.rules.as_ref())
        else {
            crate::diagnostics::log_warning(
                "Save as profile: this project stores no ERC rules yet. Store them first.",
            );
            return Task::none();
        };
        if name.is_empty() {
            crate::diagnostics::log_warning("Save as profile: the profile needs a name.");
            return Task::none();
        }
        let erc = rules.erc.clone();
        let Some(dir) = rule_profiles::profiles_dir() else {
            crate::diagnostics::log_warning("Save as profile: no config folder on this platform.");
            return Task::none();
        };
        blocking(
            move || {
                rule_profiles::save_new_profile(&dir, &name, erc)
                    .map(|(profile, path)| {
                        let note = if profile.name == name {
                            String::new()
                        } else {
                            format!(" (\"{name}\" was already taken)")
                        };
                        format!("\"{}\"{note} at {}", profile.name, path.display())
                    })
                    .map_err(|e| format!("Could not save profile \"{name}\": {e}"))
            },
            |joined| Message::Erc(ErcMsg::ProfileSaved(Arc::new(joined.and_then(|r| r)))),
        )
    }

    pub(crate) fn handle_profile_saved(
        &mut self,
        result: &Result<String, String>,
    ) -> Task<Message> {
        match result {
            Ok(what) => crate::diagnostics::log_info(format!("Saved ERC rules as profile {what}.")),
            Err(why) => crate::diagnostics::log_warning(why),
        }
        Task::none()
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

    pub(crate) fn handle_profile_update_cancelled(&mut self) -> Task<Message> {
        self.ui_state.pending_profile_update = None;
        Task::none()
    }

    /// Apply the confirmed update as a single undo step. The rule set is
    /// re-read now, not from the diff: edits made while the diff was open are
    /// not silently lost, only superseded by the profile the user chose.
    pub(crate) fn handle_profile_update_confirmed(&mut self) -> Task<Message> {
        let Some(pending) = self.ui_state.pending_profile_update.take() else {
            return Task::none();
        };
        let Some(project) = self.document_state.active_document_project() else {
            return Task::none();
        };
        let Some(mut rules) = project
            .data
            .rules
            .clone()
            .filter(|_| project.id == pending.project)
        else {
            return Task::none();
        };
        rules.erc = adopt_profile(&rules.erc, &pending.profile.erc);
        rules.profile = Some(profile_ref(&pending.profile, pending.source));
        self.replace_project_rules(Some(rules), true);
        Task::none()
    }

    /// The lines the confirmation panel lists.
    pub(crate) fn pending_profile_update_lines(&self) -> Option<(String, Vec<String>)> {
        let pending = self.ui_state.pending_profile_update.as_ref()?;
        let lines = pending.changes.iter().map(RuleChange::describe).collect();
        Some((pending.profile.name.clone(), lines))
    }

    // ---- Drift ------------------------------------------------------------

    /// Check, off the update thread, whether the profile the active project
    /// was seeded from has changed locally. Cheap no-op for a project that
    /// records no profile or was already told.
    pub(crate) fn check_profile_drift_task(&self) -> Task<Message> {
        let Some((project, reference)) = self.active_profile_ref() else {
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
        let Ok(Some((local, _))) = result.as_ref() else {
            return Task::none();
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
                rule_profiles::migrate_prefs_to_default_profile(
                    &dir,
                    &crate::fonts::read_erc_severity_overrides(),
                    &crate::fonts::read_pin_matrix_overrides(),
                )
                .map_err(|e| format!("Could not create the default rule profile: {e}"))
            },
            |joined| {
                Message::Erc(ErcMsg::DefaultProfileMigrated(Arc::new(
                    joined.and_then(|r| r),
                )))
            },
        )
    }

    pub(crate) fn handle_default_profile_migrated(
        result: &Result<ProfileMigration, String>,
    ) -> Task<Message> {
        match result {
            Ok(ProfileMigration::Migrated) => crate::diagnostics::log_info(format!(
                "Your ERC preferences were saved as the rule profile \"{}\".",
                rule_profiles::USER_DEFAULT_PROFILE_NAME
            )),
            Ok(_) => {}
            Err(why) => crate::diagnostics::log_warning(why),
        }
        Task::none()
    }
}
