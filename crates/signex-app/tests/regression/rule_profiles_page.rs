//! Mockup rows 92 and 148, #2765: the Preferences Rule Profiles page. New,
//! Duplicate and Rename open a name field instead of an always-visible one,
//! the rule editor shows one half at a time, and a rule opens alone. Driven
//! through `Signex::update`, every Task run, against the real library folder.

use signex_app::app::{Message, PreferencesMsg, Signex};
use signex_app::preferences::{
    PrefMsg, PrefNav, ProfileEditorTab, ProfileNameAction, ProfilePageMsg, ProfilesMsg,
};
use signex_app::rule_profiles;
use signex_erc::{RuleKind, Severity};
use signex_types::project::RuleProfile;

use crate::preferences_prefs_recovery::{PrefsPathGuard, serial};
use crate::rule_profiles_ui::{ProfilesDirGuard, drive};

fn pane(msg: ProfilesMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(PrefMsg::Profiles(msg)))
}

fn page(msg: ProfilePageMsg) -> Message {
    pane(ProfilesMsg::Page(msg))
}

fn open_page(app: &mut Signex) {
    drive(app, Message::Preferences(PreferencesMsg::Open));
    drive(
        app,
        Message::Preferences(PreferencesMsg::Nav(PrefNav::RuleProfiles)),
    );
}

fn profile_file(name: &str) -> std::path::PathBuf {
    rule_profiles::profiles_dir()
        .expect("test config root")
        .join(format!("{name}.snxrules"))
}

/// New, the typed name, confirmed.
fn create(app: &mut Signex, name: &str) {
    drive(
        app,
        page(ProfilePageMsg::NamePromptOpened(ProfileNameAction::New)),
    );
    drive(app, pane(ProfilesMsg::NameInputChanged(name.into())));
    drive(app, page(ProfilePageMsg::NamePromptConfirmed));
}

#[test]
fn new_opens_a_blank_name_field_and_a_confirmed_name_creates_the_profile() {
    let _serial = serial();
    let _prefs = PrefsPathGuard::capture();
    let _lib = ProfilesDirGuard::empty();
    let (mut app, _t) = Signex::new();
    open_page(&mut app);
    assert_eq!(app.ui_state.profile_manager.name_prompt, None);

    drive(
        &mut app,
        page(ProfilePageMsg::NamePromptOpened(ProfileNameAction::New)),
    );
    let manager = &app.ui_state.profile_manager;
    assert_eq!(manager.name_prompt, Some(ProfileNameAction::New));
    assert_eq!(manager.name_input, "", "the selected name is not offered");

    drive(&mut app, page(ProfilePageMsg::NamePromptConfirmed));
    assert_eq!(
        app.ui_state.profile_manager.listing.profiles.len(),
        1,
        "a blank name creates nothing"
    );

    drive(
        &mut app,
        pane(ProfilesMsg::NameInputChanged("Alpha".into())),
    );
    drive(&mut app, page(ProfilePageMsg::NamePromptConfirmed));
    assert!(profile_file("alpha").exists());
    let manager = &app.ui_state.profile_manager;
    assert_eq!(manager.selected.as_deref(), Some("Alpha"));
    assert_eq!(manager.name_prompt, None, "success closes the field");
}

#[test]
fn a_refused_name_stays_in_the_open_field() {
    let _serial = serial();
    let _prefs = PrefsPathGuard::capture();
    let _lib = ProfilesDirGuard::empty();
    let (mut app, _t) = Signex::new();
    open_page(&mut app);
    create(&mut app, "Alpha");

    create(&mut app, "alpha");
    let manager = &app.ui_state.profile_manager;
    assert_eq!(manager.name_prompt, Some(ProfileNameAction::New));
    assert_eq!(manager.name_input, "alpha");
    let (is_error, why) = manager.status.clone().expect("the refusal is shown");
    assert!(is_error && why.contains("already exists"), "{why}");

    drive(&mut app, page(ProfilePageMsg::NamePromptCancelled));
    assert_eq!(app.ui_state.profile_manager.name_prompt, None);
}

#[test]
fn rename_offers_the_current_name_and_only_for_a_users_profile() {
    let _serial = serial();
    let _prefs = PrefsPathGuard::capture();
    let _lib = ProfilesDirGuard::empty();
    let (mut app, _t) = Signex::new();
    open_page(&mut app);

    drive(
        &mut app,
        page(ProfilePageMsg::NamePromptOpened(ProfileNameAction::Rename)),
    );
    assert_eq!(
        app.ui_state.profile_manager.name_prompt, None,
        "the shipped profile cannot be renamed"
    );

    create(&mut app, "Alpha");
    drive(
        &mut app,
        page(ProfilePageMsg::NamePromptOpened(ProfileNameAction::Rename)),
    );
    assert_eq!(app.ui_state.profile_manager.name_input, "Alpha");
    drive(&mut app, pane(ProfilesMsg::NameInputChanged("Beta".into())));
    drive(&mut app, page(ProfilePageMsg::NamePromptConfirmed));
    assert!(profile_file("beta").exists());
    assert!(!profile_file("alpha").exists());
    assert_eq!(app.ui_state.profile_manager.name_prompt, None);
}

#[test]
fn a_rule_opens_alone_saves_at_once_and_closes_with_its_context() {
    let _serial = serial();
    let _prefs = PrefsPathGuard::capture();
    let _lib = ProfilesDirGuard::empty();
    let (mut app, _t) = Signex::new();
    open_page(&mut app);
    create(&mut app, "Alpha");

    drive(
        &mut app,
        page(ProfilePageMsg::RuleOpened(RuleKind::UnusedPin)),
    );
    assert_eq!(
        app.ui_state.profile_manager.open_rule,
        Some(RuleKind::UnusedPin)
    );
    drive(
        &mut app,
        pane(ProfilesMsg::SeverityChanged(
            RuleKind::UnusedPin,
            Severity::Off,
        )),
    );
    let stored =
        RuleProfile::from_json_bytes(&std::fs::read(profile_file("alpha")).unwrap()).unwrap();
    assert_eq!(stored.erc.severity["unused_pin"], "off");
    assert_eq!(
        app.ui_state.profile_manager.open_rule,
        Some(RuleKind::UnusedPin),
        "an edit keeps the rule open"
    );

    drive(&mut app, page(ProfilePageMsg::RuleClosed));
    assert_eq!(app.ui_state.profile_manager.open_rule, None);

    drive(
        &mut app,
        page(ProfilePageMsg::RuleOpened(RuleKind::UnusedPin)),
    );
    drive(
        &mut app,
        page(ProfilePageMsg::EditorTabPicked(ProfileEditorTab::PinMatrix)),
    );
    let manager = &app.ui_state.profile_manager;
    assert_eq!(manager.editor_tab, ProfileEditorTab::PinMatrix);
    assert_eq!(manager.open_rule, None, "a tab shows its whole half");

    drive(
        &mut app,
        page(ProfilePageMsg::RuleOpened(RuleKind::UnusedPin)),
    );
    drive(
        &mut app,
        pane(ProfilesMsg::Selected("Signex Default".into())),
    );
    assert_eq!(
        app.ui_state.profile_manager.open_rule, None,
        "another profile does not inherit the open rule"
    );
}
