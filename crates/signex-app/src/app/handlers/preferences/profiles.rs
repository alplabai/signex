//! Preferences ▸ Rule Profiles (#1608): list the library, edit a profile with
//! the shared rule editor, create / duplicate / rename / delete, and choose the
//! default profile for new projects.
//!
//! All file IO runs in a `Task`. Edits are written to the profile's own file at
//! once through the shared write queue; the machine's rules (`User Default`)
//! are edited through the same path the ERC dialog uses, so the two cannot
//! disagree.

use std::sync::Arc;

use iced::Task;

use super::super::super::state::rule_profile_ui::QueuedWrite;
use super::super::super::*;
use super::super::erc::blocking;
use crate::preferences::{PrefMsg, PrefNav, ProfileAction, ProfilesMsg};
use crate::rule_profiles::{self, ProfileKind, RuleEdit};

fn message(msg: ProfilesMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(PrefMsg::Profiles(msg)))
}

impl Signex {
    /// Read the library when the Rule Profiles pane is the one showing.
    pub(super) fn profiles_pane_task(&self) -> Task<Message> {
        if self.ui_state.preferences_nav != PrefNav::RuleProfiles {
            return Task::none();
        }
        Self::list_profiles_task()
    }

    /// Read the profile library off the update loop.
    pub(crate) fn list_profiles_task() -> Task<Message> {
        list_task()
    }

    pub(super) fn handle_profiles_message(&mut self, msg: ProfilesMsg) -> Task<Message> {
        match msg {
            ProfilesMsg::Listed(listing) => {
                // A listing read before a pending write landed is older than
                // the screen: applying it would show (and the next edit build
                // on) a state the user already changed. Read again after.
                if !self.ui_state.profile_writes.is_idle() {
                    self.ui_state.profile_manager.relist_pending = true;
                    return Task::none();
                }
                self.ui_state
                    .profile_manager
                    .apply_listing((*listing).clone());
                Task::none()
            }
            ProfilesMsg::Selected(name) => {
                let manager = &mut self.ui_state.profile_manager;
                manager.name_input = name.clone();
                manager.selected = Some(name);
                manager.confirm_delete = None;
                manager.confirm_reset = None;
                Task::none()
            }
            ProfilesMsg::SeverityChanged(rule, severity) => {
                self.edit_selected_profile(RuleEdit::Severity(rule, severity))
            }
            ProfilesMsg::PinCellCycled { row, col } => {
                self.edit_selected_profile(RuleEdit::PinCell(row, col))
            }
            ProfilesMsg::ResetRequested => {
                let manager = &mut self.ui_state.profile_manager;
                manager.confirm_reset = manager
                    .selected_profile()
                    .filter(|l| l.kind != ProfileKind::Builtin)
                    .map(|l| l.profile.name.clone());
                Task::none()
            }
            ProfilesMsg::ResetCancelled => {
                self.ui_state.profile_manager.confirm_reset = None;
                Task::none()
            }
            ProfilesMsg::ResetConfirmed => {
                if self.ui_state.profile_manager.confirm_reset.take().is_none() {
                    return Task::none();
                }
                self.edit_selected_profile(RuleEdit::ResetToShipped)
            }
            ProfilesMsg::NameInputChanged(name) => {
                self.ui_state.profile_manager.name_input = name;
                Task::none()
            }
            ProfilesMsg::DefaultChosen(name) => self.choose_default_profile(name),
            ProfilesMsg::CreateRequested => self.profile_action(|dir, _, name| {
                let source = signex_erc::project_rules::profile::builtin_profile();
                rule_profiles::create_profile_from(dir, &source, name)
                    .map(|_| ProfileAction::Created(name.to_string()))
            }),
            ProfilesMsg::DuplicateRequested => self.profile_action(|dir, manager, name| {
                let source = manager
                    .selected_profile()
                    .ok_or_else(|| "Select the profile to duplicate first.".to_string())?
                    .profile
                    .clone();
                rule_profiles::create_profile_from(dir, &source, name)
                    .map(|_| ProfileAction::Created(name.to_string()))
            }),
            ProfilesMsg::RenameRequested => {
                let renamable = self
                    .ui_state
                    .profile_manager
                    .selected_profile()
                    .is_some_and(|l| l.kind == ProfileKind::User);
                if !renamable {
                    self.ui_state.profile_manager.status =
                        Some((true, "Only your own profiles can be renamed.".into()));
                    return Task::none();
                }
                self.rename_selected_profile()
            }
            ProfilesMsg::DeleteRequested => {
                let manager = &mut self.ui_state.profile_manager;
                manager.confirm_delete = manager
                    .selected_profile()
                    .filter(|l| l.kind == ProfileKind::User)
                    .map(|l| l.profile.name.clone());
                Task::none()
            }
            ProfilesMsg::DeleteCancelled => {
                self.ui_state.profile_manager.confirm_delete = None;
                Task::none()
            }
            ProfilesMsg::DeleteConfirmed => {
                let Some(name) = self.ui_state.profile_manager.confirm_delete.take() else {
                    return Task::none();
                };
                self.library_action(move |dir| {
                    rule_profiles::delete_profile(dir, &name).map(|()| ProfileAction::Deleted(name))
                })
            }
            ProfilesMsg::ActionFinished(result) => self.profile_action_finished(&result),
        }
    }

    /// Apply an editor click to the selected profile. The machine's rules go
    /// through the same path as the ERC dialog; a library profile is changed in
    /// memory at once and the same one change is queued for its file, where it
    /// is applied to what the file holds when the write runs. The shipped
    /// profile ignores the click.
    fn edit_selected_profile(&mut self, edit: RuleEdit) -> Task<Message> {
        let manager = &mut self.ui_state.profile_manager;
        if manager.busy {
            manager.status = Some((
                true,
                "A profile is being created, renamed or deleted; try the edit again in a \
                 moment."
                    .into(),
            ));
            return Task::none();
        }
        let Some(kind) = manager.selected_profile().map(|l| l.kind) else {
            return Task::none();
        };
        match (kind, edit) {
            (ProfileKind::Builtin, _) => Task::none(),
            (ProfileKind::UserDefault, RuleEdit::Severity(rule, severity)) => {
                self.set_machine_severity(rule, severity)
            }
            (ProfileKind::UserDefault, RuleEdit::PinCell(row, col)) => {
                self.cycle_machine_pin_cell(row, col)
            }
            (ProfileKind::UserDefault, RuleEdit::ResetToShipped) => self.reset_machine_rules(),
            (ProfileKind::User, edit) => {
                let Some(entry) = self.ui_state.profile_manager.selected_profile_mut() else {
                    return Task::none();
                };
                edit.apply(&mut entry.profile.erc);
                let name = entry.profile.name.clone();
                let key = name.clone();
                self.submit_profile_write(&key, QueuedWrite::ProfileEdit { name, edit })
            }
        }
    }

    fn rename_selected_profile(&mut self) -> Task<Message> {
        self.profile_action(|dir, manager, name| {
            let old = manager
                .selected_profile()
                .filter(|l| l.kind == ProfileKind::User)
                .ok_or_else(|| "Only your own profiles can be renamed.".to_string())?
                .profile
                .name
                .clone();
            rule_profiles::rename_profile(dir, &old, name).map(|()| ProfileAction::Renamed {
                old,
                new: name.to_string(),
            })
        })
    }

    fn choose_default_profile(&mut self, name: Option<String>) -> Task<Message> {
        self.ui_state.default_rule_profile = name.clone();
        crate::fonts::background_pref_write("default_rule_profile", move || {
            crate::fonts::write_default_rule_profile_pref(name.as_deref())
        })
    }

    /// Run `work` on a worker against the library folder with the name field's
    /// text. Refused while an edit is still being written, because a rename or
    /// copy reads the file and would miss it.
    fn profile_action(
        &mut self,
        work: impl FnOnce(
            &std::path::Path,
            &crate::app::rule_profile_ui::ProfileManager,
            &str,
        ) -> Result<ProfileAction, String>
        + Send
        + 'static,
    ) -> Task<Message> {
        let name = self.ui_state.profile_manager.name_input.trim().to_string();
        let manager = self.ui_state.profile_manager.clone();
        self.library_action(move |dir| work(dir, &manager, &name))
    }

    fn library_action(
        &mut self,
        work: impl FnOnce(&std::path::Path) -> Result<ProfileAction, String> + Send + 'static,
    ) -> Task<Message> {
        if !self.ui_state.profile_writes.is_idle() || self.ui_state.profile_manager.busy {
            self.ui_state.profile_manager.status = Some((
                true,
                "A change is still being saved; try again in a moment.".into(),
            ));
            return Task::none();
        }
        let Some(dir) = rule_profiles::profiles_dir() else {
            self.ui_state.profile_manager.status = Some((
                true,
                "This platform has no config folder for rule profiles.".into(),
            ));
            return Task::none();
        };
        self.ui_state.profile_manager.busy = true;
        blocking(
            move || work(&dir),
            |joined| {
                message(ProfilesMsg::ActionFinished(Arc::new(
                    joined.and_then(|r| r),
                )))
            },
        )
    }

    fn profile_action_finished(&mut self, result: &Result<ProfileAction, String>) -> Task<Message> {
        self.ui_state.profile_manager.busy = false;
        let mut follow = vec![list_task()];
        match result {
            Err(why) => {
                crate::diagnostics::log_warning(why);
                self.ui_state.profile_manager.status = Some((true, why.clone()));
            }
            Ok(action) => {
                let (text, selected) = match action {
                    ProfileAction::Created(name) => {
                        (format!("Created profile \"{name}\"."), Some(name.clone()))
                    }
                    ProfileAction::Renamed { old, new } => {
                        if self.ui_state.default_rule_profile.as_deref() == Some(old.as_str()) {
                            follow.push(self.choose_default_profile(Some(new.clone())));
                        }
                        (
                            format!(
                                "Renamed \"{old}\" to \"{new}\". Projects that recorded \"{old}\" \
                                 keep their rules but can no longer find it for Update from profile."
                            ),
                            Some(new.clone()),
                        )
                    }
                    ProfileAction::Deleted(name) => {
                        let mut text = format!(
                            "Deleted profile \"{name}\". Projects that recorded it keep their \
                             rules but can no longer find it for Update from profile."
                        );
                        if self.ui_state.default_rule_profile.as_deref() == Some(name.as_str()) {
                            text.push_str(" New projects now use the automatic default.");
                            follow.push(self.choose_default_profile(None));
                        }
                        (text, None)
                    }
                };
                self.ui_state.profile_manager.status = Some((false, text));
                self.ui_state.profile_manager.selected = selected;
            }
        }
        Task::batch(follow)
    }
}

fn list_task() -> Task<Message> {
    let dir = rule_profiles::profiles_dir();
    blocking(
        move || rule_profiles::list_library(dir.as_deref()),
        |joined| {
            let listing = joined.unwrap_or_else(|why| rule_profiles::ProfileListing {
                profiles: Vec::new(),
                problems: vec![why],
            });
            message(ProfilesMsg::Listed(Arc::new(listing)))
        },
    )
}
