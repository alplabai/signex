//! The Rule Profiles page's own state (mockup rows 92 and 148): the editor
//! tab, the one rule open in the editor and the name field of New, Duplicate
//! and Rename. The library work stays in `profiles`.

use iced::Task;

use super::super::super::*;
use crate::app::rule_profile_ui::ProfileNameAction;
use crate::preferences::{ProfilePageMsg, ProfilesMsg};
use crate::rule_profiles::ProfileKind;

impl Signex {
    pub(super) fn handle_profile_page(&mut self, msg: ProfilePageMsg) -> Task<Message> {
        let manager = &mut self.ui_state.profile_manager;
        match msg {
            ProfilePageMsg::EditorTabPicked(tab) => {
                manager.editor_tab = tab;
                manager.open_rule = None;
            }
            ProfilePageMsg::RuleOpened(rule) => manager.open_rule = Some(rule),
            ProfilePageMsg::RuleClosed => manager.open_rule = None,
            ProfilePageMsg::NamePromptOpened(action) => {
                let selected = manager.selected_profile();
                let seed = match action {
                    ProfileNameAction::New => Some(String::new()),
                    ProfileNameAction::Duplicate => selected.map(|_| String::new()),
                    ProfileNameAction::Rename => selected
                        .filter(|l| l.kind == ProfileKind::User)
                        .map(|l| l.profile.name.clone()),
                };
                if let Some(seed) = seed {
                    manager.name_input = seed;
                    manager.name_prompt = Some(action);
                    manager.confirm_delete = None;
                }
            }
            ProfilePageMsg::NamePromptCancelled => manager.name_prompt = None,
            ProfilePageMsg::NamePromptConfirmed => {
                if manager.name_input.trim().is_empty() {
                    return Task::none();
                }
                // The prompt closes when the action reports success, so a
                // refused name stays in the field to correct.
                let request = match manager.name_prompt {
                    Some(ProfileNameAction::New) => ProfilesMsg::CreateRequested,
                    Some(ProfileNameAction::Duplicate) => ProfilesMsg::DuplicateRequested,
                    Some(ProfileNameAction::Rename) => ProfilesMsg::RenameRequested,
                    None => return Task::none(),
                };
                return self.handle_profiles_message(request);
            }
        }
        Task::none()
    }
}
