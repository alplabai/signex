//! Row 148: Preferences > Rule Profiles, the library side: New, Duplicate,
//! Rename, Delete and the default radio. The rule editor itself is row 92 in
//! `g5`. Edits save to the profile's file at once (decision 2026-11-22).

use std::path::PathBuf;

use signex_app::preferences::{PrefMsg, PrefNav, ProfileNameAction, ProfilePageMsg, ProfilesMsg};
use signex_app::rule_profiles;
use signex_erc::{RuleKind, Severity};
use signex_types::project::RuleProfile;

use super::support::{Prefs, opened};

const SHIPPED: &str = "Signex Default";

fn file_of(name: &str) -> PathBuf {
    rule_profiles::profiles_dir()
        .expect("test config root")
        .join(rule_profiles::file_name_for(name))
}

/// The Rule Profiles page with a new profile selected; the files named by
/// the test are removed when it ends.
struct Rig {
    prefs: Prefs,
    files: Vec<PathBuf>,
}

impl Drop for Rig {
    fn drop(&mut self) {
        for file in &self.files {
            let _ = std::fs::remove_file(file);
        }
    }
}

impl Rig {
    fn new(names: &[&str]) -> Self {
        let prefs = opened();
        let files: Vec<PathBuf> = names.iter().map(|name| file_of(name)).collect();
        for file in &files {
            let _ = std::fs::remove_file(file);
        }
        let mut rig = Self { prefs, files };
        rig.prefs.settle(PrefMsg::Nav(PrefNav::RuleProfiles));
        rig.named(ProfileNameAction::New, names[0]);
        assert_eq!(rig.selected(), Some(names[0]), "fixture: created");
        rig
    }

    fn pane(&mut self, msg: ProfilesMsg) {
        self.prefs.settle(PrefMsg::Profiles(msg));
    }

    fn page(&mut self, msg: ProfilePageMsg) {
        self.pane(ProfilesMsg::Page(msg));
    }

    fn named(&mut self, action: ProfileNameAction, name: &str) {
        self.page(ProfilePageMsg::NamePromptOpened(action));
        self.pane(ProfilesMsg::NameInputChanged(name.to_string()));
        self.page(ProfilePageMsg::NamePromptConfirmed);
    }

    fn selected(&self) -> Option<&str> {
        self.prefs.app.ui_state.profile_manager.selected.as_deref()
    }

    fn default_profile(&self) -> Option<&str> {
        self.prefs.app.ui_state.default_rule_profile.as_deref()
    }

    fn severity_on_disk(&self, name: &str) -> String {
        let bytes = std::fs::read(file_of(name)).expect("the profile's file");
        RuleProfile::from_json_bytes(&bytes)
            .expect("a readable profile")
            .erc
            .severity
            .get("unused_pin")
            .cloned()
            .unwrap_or_default()
    }
}

#[test]
fn new_creates_the_profile_file_and_selects_it() {
    let rig = Rig::new(&["C Design New"]);

    assert!(file_of("C Design New").exists());
    assert_eq!(rig.selected(), Some("C Design New"));
    assert_eq!(rig.prefs.app.ui_state.profile_manager.name_prompt, None);
}

#[test]
fn cancelling_the_name_field_creates_nothing() {
    let mut rig = Rig::new(&["C Design Keep", "C Design Never"]);

    rig.page(ProfilePageMsg::NamePromptOpened(ProfileNameAction::New));
    rig.pane(ProfilesMsg::NameInputChanged("C Design Never".to_string()));
    rig.page(ProfilePageMsg::NamePromptCancelled);

    assert!(!file_of("C Design Never").exists());
    assert_eq!(rig.prefs.app.ui_state.profile_manager.name_prompt, None);
    assert_eq!(rig.selected(), Some("C Design Keep"));
}

#[test]
fn a_blank_name_is_refused_and_the_field_stays_open() {
    let mut rig = Rig::new(&["C Design Blank"]);

    rig.named(ProfileNameAction::New, "   ");

    let manager = &rig.prefs.app.ui_state.profile_manager;
    assert_eq!(manager.name_prompt, Some(ProfileNameAction::New));
    assert_eq!(manager.name_input, "   ", "the typed text stays");
    assert_eq!(rig.selected(), Some("C Design Blank"));
}

#[test]
fn a_taken_name_is_refused_with_the_text_kept() {
    let mut rig = Rig::new(&["C Design Taken"]);

    rig.named(ProfileNameAction::New, "c design taken");

    let manager = &rig.prefs.app.ui_state.profile_manager;
    assert_eq!(manager.name_prompt, Some(ProfileNameAction::New));
    assert_eq!(manager.name_input, "c design taken");
    let (is_error, why) = manager.status.clone().expect("the refusal is shown");
    assert!(is_error && why.contains("already exists"), "{why}");
}

#[test]
fn duplicate_copies_the_rules_of_the_selected_profile_into_a_new_file() {
    let mut rig = Rig::new(&["C Design Source", "C Design Copy"]);
    rig.pane(ProfilesMsg::SeverityChanged(
        RuleKind::UnusedPin,
        Severity::Off,
    ));

    rig.named(ProfileNameAction::Duplicate, "C Design Copy");

    assert_eq!(rig.selected(), Some("C Design Copy"));
    assert_eq!(rig.severity_on_disk("C Design Copy"), "off");
    assert_eq!(rig.severity_on_disk("C Design Source"), "off");
}

#[test]
fn rename_moves_the_file_and_keeps_the_rules() {
    let mut rig = Rig::new(&["C Design Before", "C Design After"]);
    rig.pane(ProfilesMsg::SeverityChanged(
        RuleKind::UnusedPin,
        Severity::Off,
    ));

    rig.named(ProfileNameAction::Rename, "C Design After");

    assert!(!file_of("C Design Before").exists());
    assert_eq!(rig.severity_on_disk("C Design After"), "off");
    assert_eq!(rig.selected(), Some("C Design After"));
}

#[test]
fn the_shipped_profile_offers_no_rename_or_delete() {
    let mut rig = Rig::new(&["C Design Shipped"]);
    rig.pane(ProfilesMsg::Selected(SHIPPED.to_string()));

    rig.page(ProfilePageMsg::NamePromptOpened(ProfileNameAction::Rename));
    rig.pane(ProfilesMsg::DeleteRequested);

    let manager = &rig.prefs.app.ui_state.profile_manager;
    assert_eq!(manager.name_prompt, None);
    assert_eq!(manager.confirm_delete, None);
}

#[test]
fn delete_asks_first_and_backing_out_keeps_the_file() {
    let mut rig = Rig::new(&["C Design Ask"]);

    rig.pane(ProfilesMsg::DeleteRequested);
    assert_eq!(
        rig.prefs
            .app
            .ui_state
            .profile_manager
            .confirm_delete
            .as_deref(),
        Some("C Design Ask")
    );
    assert!(file_of("C Design Ask").exists(), "asking deletes nothing");

    rig.pane(ProfilesMsg::DeleteCancelled);

    assert_eq!(rig.prefs.app.ui_state.profile_manager.confirm_delete, None);
    assert!(file_of("C Design Ask").exists());
}

#[test]
fn a_confirmed_delete_removes_the_file_and_the_row() {
    let mut rig = Rig::new(&["C Design Gone"]);

    rig.pane(ProfilesMsg::DeleteRequested);
    rig.pane(ProfilesMsg::DeleteConfirmed);

    assert!(!file_of("C Design Gone").exists());
    // The fresh listing moves the selection to its first profile.
    let selected = rig.selected();
    assert!(
        selected.is_some() && selected != Some("C Design Gone"),
        "{selected:?}"
    );
    assert!(
        !rig.prefs
            .app
            .ui_state
            .profile_manager
            .listing
            .profiles
            .iter()
            .any(|listed| listed.profile.name == "C Design Gone")
    );
}

#[test]
fn the_default_radio_marks_the_profile_for_new_projects() {
    let mut rig = Rig::new(&["C Design Default"]);

    rig.pane(ProfilesMsg::DefaultChosen(Some(
        "C Design Default".to_string(),
    )));
    assert_eq!(rig.default_profile(), Some("C Design Default"));

    rig.pane(ProfilesMsg::DefaultChosen(None));
    assert_eq!(rig.default_profile(), None, "Automatic");
}

#[test]
fn renaming_the_default_profile_keeps_it_the_default() {
    let mut rig = Rig::new(&["C Design Default A", "C Design Default B"]);
    rig.pane(ProfilesMsg::DefaultChosen(Some(
        "C Design Default A".to_string(),
    )));

    rig.named(ProfileNameAction::Rename, "C Design Default B");

    assert_eq!(rig.default_profile(), Some("C Design Default B"));
}

#[test]
fn deleting_the_default_profile_falls_back_to_automatic() {
    let mut rig = Rig::new(&["C Design Default Gone"]);
    rig.pane(ProfilesMsg::DefaultChosen(Some(
        "C Design Default Gone".to_string(),
    )));

    rig.pane(ProfilesMsg::DeleteRequested);
    rig.pane(ProfilesMsg::DeleteConfirmed);

    assert_eq!(rig.default_profile(), None);
}

#[test]
fn a_profile_edit_saves_at_once_and_leaves_apply_with_nothing_to_do() {
    let mut rig = Rig::new(&["C Design Direct"]);

    rig.pane(ProfilesMsg::SeverityChanged(
        RuleKind::UnusedPin,
        Severity::Off,
    ));

    assert_eq!(rig.severity_on_disk("C Design Direct"), "off");
    assert!(!rig.prefs.is_dirty());
}
