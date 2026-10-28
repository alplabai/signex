//! #1608 — rule profiles end to end: the messages are driven through
//! `update` and every Task is run to completion (file IO included), so these
//! tests read the same files a user's disk would hold.
//!
//! The profile library lives under the per-process test config root shared by
//! every test in this binary, so each test takes the module-wide lock and
//! starts from an empty library ([`ProfilesDirGuard`]).

use std::path::PathBuf;
use std::sync::Arc;

use signex_app::app::{ErcMsg, FileMsg, Message, PreferencesMsg, Signex};
use signex_app::preferences::{PrefMsg, PrefNav, ProfilesMsg};
use signex_app::rule_profiles::{self, SaveOutcome};
use signex_erc::{RuleKind, Severity};
use signex_types::project::{ErcRules, ProjectRules, RuleProfile, RuleProfileRef};
use tempfile::TempDir;

use crate::preferences_prefs_recovery::{PrefsPathGuard, serial};
use crate::settle_support::{settle, settle_task};

/// Empties the shared profile library on entry and exit, so one test's
/// profiles never reach another's `New Project` seeding or listing.
pub(crate) struct ProfilesDirGuard(PathBuf);

impl ProfilesDirGuard {
    pub(crate) fn empty() -> Self {
        let dir = rule_profiles::profiles_dir().expect("test config root");
        let _ = std::fs::remove_dir_all(&dir);
        Self(dir)
    }
}

impl Drop for ProfilesDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn library() -> PathBuf {
    rule_profiles::profiles_dir().expect("test config root")
}

/// What the `User Default` file holds now.
pub(crate) fn stored_machine_rules() -> ErcRules {
    rule_profiles::load_user_default(&library())
        .expect("readable")
        .map(|p| p.erc)
        .unwrap_or_default()
}

/// Send `message` and run its Tasks to completion.
pub(crate) fn drive(app: &mut Signex, message: Message) {
    settle(app, message);
}

/// Run an already-returned Task to completion.
pub(crate) fn finish(app: &mut Signex, task: iced::Task<Message>) {
    settle_task(app, task);
}

fn erc(msg: ErcMsg) -> Message {
    Message::Erc(msg)
}

fn pane(msg: ProfilesMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(PrefMsg::Profiles(msg)))
}

fn rules(rule: &str, severity: &str) -> ErcRules {
    let mut erc = ErcRules::default();
    erc.severity.insert(rule.into(), severity.into());
    erc
}

fn read_profile(file: &str) -> RuleProfile {
    RuleProfile::from_json_bytes(&std::fs::read(library().join(file)).unwrap()).unwrap()
}

fn project_rules(app: &Signex) -> ProjectRules {
    app.document_state
        .projects
        .last()
        .expect("a project is open")
        .data
        .rules
        .clone()
        .expect("the project stores rules")
}

fn new_project(app: &mut Signex, tmp: &TempDir, name: &str) {
    drive(
        app,
        Message::File(FileMsg::NewProject(Some(
            tmp.path().join(format!("{name}.snxprj")),
        ))),
    );
    assert!(app.document_state.error_notice.is_none());
}

// ---- 1. seeding ----

#[test]
fn a_new_project_starts_with_a_copy_of_the_chosen_default_profile() {
    let _serial = serial();
    let _prefs = PrefsPathGuard::capture();
    let _lib = ProfilesDirGuard::empty();
    let tmp = TempDir::new().unwrap();
    rule_profiles::save_named_profile(
        &library(),
        "Company",
        "4",
        rules("unused_pin", "off"),
        false,
    )
    .unwrap();
    let (mut app, _t) = Signex::new();

    drive(
        &mut app,
        pane(ProfilesMsg::DefaultChosen(Some("Company".into()))),
    );
    new_project(&mut app, &tmp, "Seeded");

    let stored = project_rules(&app);
    assert_eq!(stored.erc, rules("unused_pin", "off"));
    let reference = stored.profile.expect("provenance");
    assert_eq!(
        (
            reference.name.as_str(),
            reference.source.as_str(),
            reference.version.as_str()
        ),
        ("Company", "user", "4")
    );
    // On disk too, not just in memory.
    let on_disk = signex_types::project::parse_project(&tmp.path().join("Seeded/Seeded.snxprj"))
        .unwrap()
        .rules
        .expect("the .snxprj carries the rules");
    assert_eq!(on_disk.erc, stored.erc);
}

#[test]
fn the_default_choice_survives_a_restart_and_automatic_falls_back_to_user_default() {
    let _serial = serial();
    let _prefs = PrefsPathGuard::capture();
    let _lib = ProfilesDirGuard::empty();
    let tmp = TempDir::new().unwrap();
    rule_profiles::save_named_profile(
        &library(),
        "Company",
        "1.0",
        rules("unused_pin", "off"),
        false,
    )
    .unwrap();
    let (mut app, _t) = Signex::new();
    drive(
        &mut app,
        pane(ProfilesMsg::DefaultChosen(Some("Company".into()))),
    );

    // A fresh start reads the choice back from prefs.json.
    let (mut restarted, _t) = Signex::new();
    assert_eq!(
        restarted.ui_state.default_rule_profile.as_deref(),
        Some("Company")
    );
    drive(&mut restarted, pane(ProfilesMsg::DefaultChosen(None)));
    let (mut again, _t) = Signex::new();
    assert_eq!(
        again.ui_state.default_rule_profile, None,
        "automatic is remembered too"
    );

    // Automatic: the machine's rules when there are some.
    rule_profiles::write_user_default(&library(), rules("dangling_wire", "info")).unwrap();
    new_project(&mut again, &tmp, "Auto");
    let stored = project_rules(&again);
    assert_eq!(stored.erc.severity["dangling_wire"], "info");
    assert_eq!(stored.profile.unwrap().name, "User Default");
}

#[test]
fn a_new_project_with_no_profiles_at_all_gets_the_shipped_one() {
    let _serial = serial();
    let _prefs = PrefsPathGuard::capture();
    let _lib = ProfilesDirGuard::empty();
    let tmp = TempDir::new().unwrap();
    let (mut app, _t) = Signex::new();
    new_project(&mut app, &tmp, "Plain");
    let stored = project_rules(&app);
    let reference = stored.profile.unwrap();
    assert_eq!(
        (reference.name.as_str(), reference.source.as_str()),
        ("Signex Default", "builtin")
    );
}

// ---- 2. picker and Preferences editor ----

fn open_profiles_pane(app: &mut Signex) {
    drive(app, Message::Preferences(PreferencesMsg::Open));
    drive(
        app,
        Message::Preferences(PreferencesMsg::Nav(PrefNav::RuleProfiles)),
    );
    assert!(
        app.ui_state.profile_manager.loaded,
        "the pane read the library"
    );
}

#[test]
fn profile_crud_round_trips_on_disk() {
    let _serial = serial();
    let _prefs = PrefsPathGuard::capture();
    let _lib = ProfilesDirGuard::empty();
    let (mut app, _t) = Signex::new();
    open_profiles_pane(&mut app);
    let names = |app: &Signex| -> Vec<String> {
        app.ui_state
            .profile_manager
            .listing
            .profiles
            .iter()
            .map(|l| l.profile.name.clone())
            .collect()
    };
    assert_eq!(names(&app), ["Signex Default"]);

    // New: from the shipped rules.
    drive(
        &mut app,
        pane(ProfilesMsg::NameInputChanged("Alpha".into())),
    );
    drive(&mut app, pane(ProfilesMsg::CreateRequested));
    assert_eq!(names(&app), ["Signex Default", "Alpha"]);
    assert_eq!(
        read_profile("alpha.snxrules").erc,
        signex_erc::project_rules::profile::builtin_profile().erc
    );
    assert_eq!(
        app.ui_state.profile_manager.selected.as_deref(),
        Some("Alpha")
    );

    // A taken name is refused and the file is untouched.
    let before = std::fs::read(library().join("alpha.snxrules")).unwrap();
    drive(
        &mut app,
        pane(ProfilesMsg::NameInputChanged("alpha".into())),
    );
    drive(&mut app, pane(ProfilesMsg::CreateRequested));
    let (is_error, text) = app.ui_state.profile_manager.status.clone().unwrap();
    assert!(is_error && text.contains("already exists"), "{text}");
    assert_eq!(
        std::fs::read(library().join("alpha.snxrules")).unwrap(),
        before
    );

    // Edit with the rule editor: written at once, dialog not dirtied.
    drive(
        &mut app,
        pane(ProfilesMsg::SeverityChanged(
            RuleKind::UnusedPin,
            Severity::Off,
        )),
    );
    drive(
        &mut app,
        pane(ProfilesMsg::PinCellCycled { row: 1, col: 1 }),
    );
    let edited = read_profile("alpha.snxrules");
    assert_eq!(edited.erc.severity["unused_pin"], "off");
    assert_eq!(edited.erc.pin_matrix.len(), 1);
    assert!(!app.ui_state.preferences_dirty);

    // Duplicate carries the edit.
    drive(&mut app, pane(ProfilesMsg::NameInputChanged("Beta".into())));
    drive(&mut app, pane(ProfilesMsg::DuplicateRequested));
    assert_eq!(read_profile("beta.snxrules").erc, edited.erc);

    // Rename, with the default following it.
    drive(
        &mut app,
        pane(ProfilesMsg::DefaultChosen(Some("Beta".into()))),
    );
    drive(
        &mut app,
        pane(ProfilesMsg::NameInputChanged("Gamma".into())),
    );
    drive(&mut app, pane(ProfilesMsg::RenameRequested));
    assert!(!library().join("beta.snxrules").exists());
    assert_eq!(read_profile("gamma.snxrules").name, "Gamma");
    assert_eq!(app.ui_state.default_rule_profile.as_deref(), Some("Gamma"));

    // Delete asks first; confirming removes only that file and clears the default.
    drive(&mut app, pane(ProfilesMsg::DeleteRequested));
    assert!(
        library().join("gamma.snxrules").exists(),
        "asking deletes nothing"
    );
    drive(&mut app, pane(ProfilesMsg::DeleteConfirmed));
    assert!(!library().join("gamma.snxrules").exists());
    assert!(library().join("alpha.snxrules").exists());
    assert_eq!(app.ui_state.default_rule_profile, None);
    assert_eq!(names(&app), ["Signex Default", "Alpha"]);
}

// ---- 4. the no-project dialog edits the User Default profile ----

#[test]
fn the_no_project_erc_dialog_writes_user_default_and_not_prefs() {
    let _serial = serial();
    let _prefs = PrefsPathGuard::capture();
    let _lib = ProfilesDirGuard::empty();
    let (mut app, _t) = Signex::new();
    assert!(app.document_state.active_document_project().is_none());
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();

    drive(
        &mut app,
        erc(ErcMsg::SeverityChanged(RuleKind::UnusedPin, Severity::Off)),
    );
    drive(
        &mut app,
        erc(ErcMsg::PinMatrixCellCycled { row: 1, col: 1 }),
    );

    let stored = read_profile("user-default.snxrules");
    assert_eq!(stored.name, "User Default");
    assert_eq!(stored.erc.severity["unused_pin"], "off");
    assert_eq!(stored.erc.pin_matrix.len(), 1);
    let prefs = std::fs::read_to_string(signex_app::fonts::prefs_file_path()).unwrap_or_default();
    assert!(
        !prefs.contains("erc_severity") && !prefs.contains("pin_matrix"),
        "prefs.json no longer holds ERC rules: {prefs}"
    );

    // The same store through Preferences: the editor sees the dialog's edit and
    // an edit there reaches the dialog's rules.
    open_profiles_pane(&mut app);
    drive(&mut app, pane(ProfilesMsg::Selected("User Default".into())));
    let shown = app
        .ui_state
        .profile_manager
        .selected_profile()
        .unwrap()
        .profile
        .erc
        .clone();
    assert_eq!(shown.severity["unused_pin"], "off");
    drive(
        &mut app,
        pane(ProfilesMsg::SeverityChanged(
            RuleKind::DanglingWire,
            Severity::Info,
        )),
    );
    assert_eq!(
        app.ui_state.erc_severity_override[&RuleKind::DanglingWire],
        Severity::Info
    );
    assert_eq!(
        read_profile("user-default.snxrules").erc.severity["dangling_wire"],
        "info"
    );

    // A restart reads it back.
    let (mut restarted, _t) = Signex::new();
    let loaded = rule_profiles::load_user_default(&library()).unwrap();
    drive(
        &mut restarted,
        erc(ErcMsg::UserDefaultLoaded(Arc::new(Ok(loaded)))),
    );
    assert_eq!(
        restarted.ui_state.erc_severity_override[&RuleKind::UnusedPin],
        Severity::Off
    );
    assert_eq!(
        restarted.ui_state.erc_severity_override[&RuleKind::DanglingWire],
        Severity::Info
    );
}

// ---- 3. Save as profile ----

#[test]
fn save_as_profile_asks_before_overwriting_and_honours_cancel() {
    let _serial = serial();
    let _prefs = PrefsPathGuard::capture();
    let _lib = ProfilesDirGuard::empty();
    let tmp = TempDir::new().unwrap();
    let (mut app, _t) = Signex::new();
    new_project(&mut app, &tmp, "Demo");
    let save = |app: &mut Signex, name: &str, version: &str| {
        drive(app, erc(ErcMsg::ProfileSaveNameChanged(name.into())));
        drive(app, erc(ErcMsg::ProfileSaveVersionChanged(version.into())));
        drive(app, erc(ErcMsg::ProfileSaveSubmitted));
    };

    drive(&mut app, erc(ErcMsg::ProfileSaveDialogOpened));
    assert_eq!(
        app.ui_state.save_profile_dialog.as_ref().unwrap().name,
        "Demo rules"
    );
    save(&mut app, "Team", "2.5");
    assert!(
        app.ui_state.save_profile_dialog.is_none(),
        "a free name saves at once"
    );
    let first = read_profile("team.snxrules");
    assert_eq!(
        (first.name.as_str(), first.version.as_str()),
        ("Team", "2.5")
    );
    let recorded = project_rules(&app).profile.unwrap();
    assert_eq!(
        (recorded.name.as_str(), recorded.version.as_str()),
        ("Team", "2.5")
    );

    // Change the project's rules, then try the same name again.
    drive(
        &mut app,
        erc(ErcMsg::SeverityChanged(RuleKind::UnusedPin, Severity::Off)),
    );
    let changed = project_rules(&app).erc;
    let on_disk = std::fs::read(library().join("team.snxrules")).unwrap();
    drive(&mut app, erc(ErcMsg::ProfileSaveDialogOpened));
    save(&mut app, "team", "3.0");
    let dialog = app
        .ui_state
        .save_profile_dialog
        .as_ref()
        .expect("still asking");
    assert!(dialog.confirm_overwrite && !dialog.busy);
    assert_eq!(
        std::fs::read(library().join("team.snxrules")).unwrap(),
        on_disk,
        "nothing written yet"
    );

    // Cancel: back to the inputs, then out; the file is still the first one.
    drive(&mut app, erc(ErcMsg::ProfileSaveCancelled));
    assert!(
        !app.ui_state
            .save_profile_dialog
            .as_ref()
            .unwrap()
            .confirm_overwrite
    );
    drive(&mut app, erc(ErcMsg::ProfileSaveOverwriteConfirmed));
    assert_eq!(
        std::fs::read(library().join("team.snxrules")).unwrap(),
        on_disk,
        "no stray confirm"
    );
    drive(&mut app, erc(ErcMsg::ProfileSaveCancelled));
    assert!(app.ui_state.save_profile_dialog.is_none());
    assert_eq!(
        std::fs::read(library().join("team.snxrules")).unwrap(),
        on_disk
    );

    // Ask again and confirm: now it is replaced, and the project records it.
    drive(&mut app, erc(ErcMsg::ProfileSaveDialogOpened));
    save(&mut app, "team", "3.0");
    drive(&mut app, erc(ErcMsg::ProfileSaveOverwriteConfirmed));
    assert!(app.ui_state.save_profile_dialog.is_none());
    let replaced = read_profile("team.snxrules");
    assert_eq!(replaced.version, "3.0");
    assert_eq!(replaced.erc, changed);
    let recorded = project_rules(&app).profile.unwrap();
    assert_eq!(recorded.sha256, changed.sha256_hex());
    assert_eq!(
        std::fs::read_dir(library()).unwrap().count(),
        1,
        "still one profile file"
    );

    // The explicit outcome type is public for the worker's callers.
    assert!(matches!(
        rule_profiles::save_named_profile(&library(), "Team", "3.0", changed, false).unwrap(),
        SaveOutcome::Exists
    ));
}

// ---- 5. drift on open ----

#[test]
fn opening_a_project_reports_a_changed_profile_without_touching_the_project() {
    let _serial = serial();
    let _prefs = PrefsPathGuard::capture();
    let _lib = ProfilesDirGuard::empty();
    let tmp = TempDir::new().unwrap();
    let name = format!("Drift {}", uuid::Uuid::new_v4().simple());
    rule_profiles::save_named_profile(
        &library(),
        &name,
        "1.0",
        rules("unused_pin", "error"),
        false,
    )
    .unwrap();

    // A project recorded against an older state of that profile.
    let project = tmp.path().join("Old.snxprj");
    std::fs::write(&project, b"").unwrap();
    let mut data = signex_types::project::parse_project(&project).unwrap();
    data.rules = Some(ProjectRules {
        erc: rules("unused_pin", "info"),
        profile: Some(RuleProfileRef {
            name: name.clone(),
            source: "user".into(),
            sha256: rules("unused_pin", "info").sha256_hex(),
            ..Default::default()
        }),
        ..ProjectRules::default()
    });
    signex_types::project::write_project(&project, &data).unwrap();
    let before = std::fs::read(&project).unwrap();

    let _ = signex_app::diagnostics::init_logging();
    let (mut app, _t) = Signex::new();
    let note = |needle: &str| {
        signex_app::diagnostics::recent_entries()
            .iter()
            .filter(|e| e.message.contains(needle))
            .count()
    };
    let announced = note(&name);
    drive(
        &mut app,
        Message::File(FileMsg::Opened(Some(project.clone()))),
    );

    assert_eq!(note(&name), announced + 1, "one note, from the open itself");
    assert_eq!(
        std::fs::read(&project).unwrap(),
        before,
        "the project file is untouched"
    );
    assert_eq!(
        project_rules(&app).erc,
        rules("unused_pin", "info"),
        "rules are not overwritten"
    );
    assert!(
        !app.document_state.dirty_paths.contains(&project),
        "the project is not dirtied by the check"
    );
}

#[test]
fn opening_a_project_whose_profile_is_unchanged_says_nothing() {
    let _serial = serial();
    let _prefs = PrefsPathGuard::capture();
    let _lib = ProfilesDirGuard::empty();
    let tmp = TempDir::new().unwrap();
    let name = format!("Calm {}", uuid::Uuid::new_v4().simple());
    let current = rules("unused_pin", "error");
    rule_profiles::save_named_profile(&library(), &name, "1.0", current.clone(), false).unwrap();

    let project = tmp.path().join("Calm.snxprj");
    std::fs::write(&project, b"").unwrap();
    let mut data = signex_types::project::parse_project(&project).unwrap();
    data.rules = Some(ProjectRules {
        erc: current.clone(),
        profile: Some(RuleProfileRef {
            name: name.clone(),
            source: "user".into(),
            sha256: current.sha256_hex(),
            ..Default::default()
        }),
        ..ProjectRules::default()
    });
    signex_types::project::write_project(&project, &data).unwrap();

    let _ = signex_app::diagnostics::init_logging();
    let (mut app, _t) = Signex::new();
    drive(&mut app, Message::File(FileMsg::Opened(Some(project))));
    assert!(
        !signex_app::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains(&name)),
        "no drift, no note"
    );
}
