//! Mockup row 92 as the 2026-11-22 decision has it: the rule editor on the
//! Preferences Rule Profiles page. Edits save to the profile's file at once,
//! so the editor has Close and no Cancel or Save. Each test owns one uniquely
//! named profile in the test config root and removes its file.

use std::path::PathBuf;

use signex_app::app::{Message, PreferencesMsg, Signex};
use signex_app::preferences::{
    PrefMsg, PrefNav, ProfileEditorTab, ProfileNameAction, ProfilePageMsg, ProfilesMsg,
};
use signex_app::rule_profiles;
use signex_erc::{RuleKind, Severity};
use signex_types::project::{ErcRules, RuleProfile};

use crate::settle_support::settle;

const SHIPPED: &str = "Signex Default";

struct ProfileRig {
    app: Signex,
    name: String,
    file: PathBuf,
}

impl Drop for ProfileRig {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.file);
    }
}

fn pane(msg: ProfilesMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(PrefMsg::Profiles(msg)))
}

fn page(msg: ProfilePageMsg) -> Message {
    pane(ProfilesMsg::Page(msg))
}

impl ProfileRig {
    /// The Rule Profiles page with a new profile `name` selected.
    fn new(name: &str) -> Self {
        let dir = rule_profiles::profiles_dir().expect("test config root");
        let file = dir.join(rule_profiles::file_name_for(name));
        let _ = std::fs::remove_file(&file);
        let (app, _task) = Signex::new();
        let mut rig = ProfileRig {
            app,
            name: name.to_string(),
            file,
        };
        rig.drive(Message::Preferences(PreferencesMsg::Open));
        rig.drive(Message::Preferences(PreferencesMsg::Nav(
            PrefNav::RuleProfiles,
        )));
        rig.create(name);
        assert_eq!(rig.selected(), Some(name), "the new profile is selected");
        rig
    }

    fn drive(&mut self, message: Message) {
        settle(&mut self.app, message);
    }

    fn create(&mut self, name: &str) {
        self.drive(page(ProfilePageMsg::NamePromptOpened(
            ProfileNameAction::New,
        )));
        self.drive(pane(ProfilesMsg::NameInputChanged(name.to_string())));
        self.drive(page(ProfilePageMsg::NamePromptConfirmed));
    }

    fn selected(&self) -> Option<&str> {
        self.app.ui_state.profile_manager.selected.as_deref()
    }

    /// The rules the profile's file holds.
    fn on_disk(&self) -> ErcRules {
        let bytes = std::fs::read(&self.file).expect("the profile's file");
        RuleProfile::from_json_bytes(&bytes)
            .expect("a readable profile")
            .erc
    }

    fn set_unused_pin_off(&mut self) {
        self.drive(pane(ProfilesMsg::SeverityChanged(
            RuleKind::UnusedPin,
            Severity::Off,
        )));
    }
}

fn shipped() -> ErcRules {
    signex_erc::project_rules::profile::builtin_profile().erc
}

fn pin_cell(erc: &ErcRules, row: u8, col: u8) -> Severity {
    let resolved = signex_erc::project_rules::resolve(erc);
    signex_erc::pin_matrix::pair_cell(&resolved.pin_matrix, row, col)
}

#[test]
fn a_severity_click_saves_to_the_profile_file_at_once_and_leaves_preferences_clean() {
    let mut rig = ProfileRig::new("G5 Audit Severity");
    assert_eq!(rig.on_disk(), shipped(), "a new profile is the shipped one");

    rig.set_unused_pin_off();

    assert_eq!(rig.on_disk().severity["unused_pin"], "off");
    assert!(
        !rig.app.ui_state.preferences_dirty,
        "nothing is left to save"
    );
}

#[test]
fn a_pin_matrix_click_moves_the_cell_one_level_in_the_file() {
    let mut rig = ProfileRig::new("G5 Audit Pin Cell");
    let before = pin_cell(&rig.on_disk(), 4, 4);

    rig.drive(pane(ProfilesMsg::PinCellCycled { row: 4, col: 4 }));

    assert_eq!(pin_cell(&rig.on_disk(), 4, 4), before.cycled());
    assert_ne!(rig.on_disk(), shipped());
}

#[test]
fn a_rule_opens_alone_keeps_open_across_an_edit_and_close_returns_to_the_table() {
    let mut rig = ProfileRig::new("G5 Audit Open Rule");
    rig.drive(page(ProfilePageMsg::RuleOpened(RuleKind::UnusedPin)));
    assert_eq!(
        rig.app.ui_state.profile_manager.open_rule,
        Some(RuleKind::UnusedPin)
    );

    rig.set_unused_pin_off();
    assert_eq!(
        rig.app.ui_state.profile_manager.open_rule,
        Some(RuleKind::UnusedPin),
        "an edit keeps the rule open"
    );

    rig.drive(page(ProfilePageMsg::RuleClosed));
    assert_eq!(rig.app.ui_state.profile_manager.open_rule, None);
    assert_eq!(
        rig.on_disk().severity["unused_pin"],
        "off",
        "Close keeps the saved edit: there is no Cancel"
    );
}

#[test]
fn the_pin_matrix_tab_shows_its_whole_half_and_closes_an_open_rule() {
    let mut rig = ProfileRig::new("G5 Audit Editor Tab");
    rig.drive(page(ProfilePageMsg::RuleOpened(RuleKind::UnusedPin)));

    rig.drive(page(ProfilePageMsg::EditorTabPicked(
        ProfileEditorTab::PinMatrix,
    )));

    let manager = &rig.app.ui_state.profile_manager;
    assert_eq!(manager.editor_tab, ProfileEditorTab::PinMatrix);
    assert_eq!(manager.open_rule, None);
    assert_eq!(rig.on_disk(), shipped(), "picking a tab writes nothing");
}

#[test]
fn the_changed_only_chip_and_the_rule_filter_write_nothing() {
    let mut rig = ProfileRig::new("G5 Audit Filter");
    rig.set_unused_pin_off();
    let saved = rig.on_disk();

    rig.drive(pane(ProfilesMsg::ChangedOnlyToggled(true)));
    rig.drive(pane(ProfilesMsg::RuleFilterChanged("unused".to_string())));

    let list = &rig.app.ui_state.profile_manager.rule_list;
    assert!(list.changed_only);
    assert_eq!(list.query, "unused");
    assert_eq!(rig.on_disk(), saved);
}

#[test]
fn reset_asks_first_and_backing_out_keeps_the_edits() {
    let mut rig = ProfileRig::new("G5 Audit Reset Cancel");
    rig.set_unused_pin_off();
    let edited = rig.on_disk();

    rig.drive(pane(ProfilesMsg::ResetRequested));
    assert_eq!(
        rig.app.ui_state.profile_manager.confirm_reset.as_deref(),
        Some(rig.name.as_str())
    );
    assert_eq!(rig.on_disk(), edited, "asking resets nothing");

    rig.drive(pane(ProfilesMsg::ResetCancelled));
    assert_eq!(rig.app.ui_state.profile_manager.confirm_reset, None);
    assert_eq!(rig.on_disk(), edited);
}

#[test]
fn a_confirmed_reset_puts_the_shipped_rules_back_in_the_file() {
    let mut rig = ProfileRig::new("G5 Audit Reset Confirm");
    rig.set_unused_pin_off();
    rig.drive(pane(ProfilesMsg::PinCellCycled { row: 4, col: 4 }));
    assert_ne!(rig.on_disk(), shipped());

    rig.drive(pane(ProfilesMsg::ResetRequested));
    rig.drive(pane(ProfilesMsg::ResetConfirmed));

    assert_eq!(rig.on_disk(), shipped());
    assert_eq!(rig.app.ui_state.profile_manager.confirm_reset, None);
}

#[test]
fn the_shipped_profile_ignores_edits_and_offers_no_reset() {
    let mut rig = ProfileRig::new("G5 Audit Shipped");
    rig.drive(pane(ProfilesMsg::Selected(SHIPPED.to_string())));
    assert_eq!(rig.selected(), Some(SHIPPED));
    let erc = |rig: &ProfileRig| {
        rig.app
            .ui_state
            .profile_manager
            .selected_profile()
            .expect("the shipped profile is listed")
            .profile
            .erc
            .clone()
    };
    let before = erc(&rig);

    rig.set_unused_pin_off();
    rig.drive(pane(ProfilesMsg::PinCellCycled { row: 4, col: 4 }));
    rig.drive(pane(ProfilesMsg::ResetRequested));

    assert_eq!(erc(&rig), before);
    assert_eq!(rig.app.ui_state.profile_manager.confirm_reset, None);
}

#[test]
fn a_taken_profile_name_is_refused_and_stays_in_the_open_field() {
    let mut rig = ProfileRig::new("G5 Audit Taken");
    let before = std::fs::read(&rig.file).expect("the profile's file");

    rig.create("g5 audit taken");

    let manager = &rig.app.ui_state.profile_manager;
    assert_eq!(manager.name_prompt, Some(ProfileNameAction::New));
    assert_eq!(manager.name_input, "g5 audit taken");
    let (is_error, why) = manager.status.clone().expect("the refusal is shown");
    assert!(is_error && why.contains("already exists"), "{why}");
    assert_eq!(std::fs::read(&rig.file).expect("the file"), before);
}

#[test]
fn cancelling_the_name_field_creates_no_profile() {
    let mut rig = ProfileRig::new("G5 Audit Name Cancel");
    let other = rule_profiles::profiles_dir()
        .expect("test config root")
        .join(rule_profiles::file_name_for("G5 Audit Never Made"));

    rig.drive(page(ProfilePageMsg::NamePromptOpened(
        ProfileNameAction::New,
    )));
    rig.drive(pane(ProfilesMsg::NameInputChanged(
        "G5 Audit Never Made".to_string(),
    )));
    rig.drive(page(ProfilePageMsg::NamePromptCancelled));

    assert_eq!(rig.app.ui_state.profile_manager.name_prompt, None);
    assert!(!other.exists(), "Cancel writes no file");
    assert_eq!(rig.selected(), Some(rig.name.as_str()));
}
