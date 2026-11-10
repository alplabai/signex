//! The New Project dialog's state machine and the headless command (#2232).

use std::path::Path;

use signex_types::project::{PcbField, SetupProfile};

use super::state::{NewProjectState, project_file_in};
use super::target::{TargetProblem, TargetStatus, check_target};
use super::{NewProjectMsg, check_name};
use crate::app::{Message, Signex};
use crate::menu_bar::MenuMessage;
use crate::task_driver::TaskDriver;

fn state_in(dir: &Path, name: &str) -> NewProjectState {
    let mut s = NewProjectState::new(dir.display().to_string());
    s.name = name.to_string();
    s
}

fn settle(app: &mut Signex, msg: Message) {
    let task = app.update(msg);
    TaskDriver::new().settle(app, task).expect("settled");
}

fn dialog(app: &mut Signex, msg: NewProjectMsg) {
    settle(app, Message::from(msg));
}

#[test]
fn a_fresh_dialog_is_standard_4_layer() {
    let s = NewProjectState::new(String::new());
    assert_eq!(s.setup.profile, SetupProfile::Standard);
    assert_eq!(s.setup.pcb.copper_layers, 4);
    assert!(s.make_folder);
    assert!(!s.customize_open, "Customize is folded by default");
}

#[test]
fn picking_a_profile_refills_the_fields_and_custom_opens_them() {
    let mut s = NewProjectState::new(String::new());
    s.pick_profile(SetupProfile::Hdi);
    assert_eq!(s.drafts.clearance, "0.1");
    assert_eq!(s.drafts.via, "0.3");
    assert_eq!(s.setup.pcb.copper_layers, 6);
    assert!(!s.customize_open);
    s.pick_profile(SetupProfile::Custom);
    assert!(s.customize_open);
    assert_eq!(s.setup.pcb.copper_layers, 8);
}

#[test]
fn the_stepper_switches_to_custom_and_stays_in_range() {
    let mut s = NewProjectState::new(String::new());
    s.step_layers(1);
    assert_eq!(
        (s.setup.profile, s.setup.pcb.copper_layers),
        (SetupProfile::Custom, 5)
    );
    assert!(s.customize_open);
    for _ in 0..40 {
        s.step_layers(1);
    }
    assert_eq!(s.setup.pcb.copper_layers, 32);
    for _ in 0..40 {
        s.step_layers(-1);
    }
    assert_eq!(s.setup.pcb.copper_layers, 1);
}

#[test]
fn the_folder_toggle_chooses_between_the_two_paths() {
    let dir = Path::new("/work");
    assert_eq!(
        project_file_in(dir, "Motor", true),
        dir.join("Motor").join("Motor.snxprj")
    );
    assert_eq!(
        project_file_in(dir, "Motor", false),
        dir.join("Motor.snxprj")
    );
    let mut s = state_in(dir, "  Motor Driver Rev A ");
    assert_eq!(
        s.folder_label_name(),
        "Motor Driver Rev A",
        "the label tracks the name"
    );
    assert_eq!(
        s.project_file().unwrap(),
        dir.join("Motor Driver Rev A")
            .join("Motor Driver Rev A.snxprj")
    );
    s.make_folder = false;
    assert_eq!(
        s.project_file().unwrap(),
        dir.join("Motor Driver Rev A.snxprj")
    );
}

#[test]
fn names_that_cannot_be_files_are_refused() {
    for bad in [
        "", "  ", "a/b", "a:b", "CON", "con.txt", "x.", "a\u{7}b", "q?",
    ] {
        assert!(check_name(bad).is_err(), "{bad:?}");
    }
    assert_eq!(check_name(" Motor Driver Rev A "), Ok("Motor Driver Rev A"));
}

#[test]
fn create_is_off_until_the_target_is_checked_clear() {
    let tmp = tempfile::tempdir().unwrap();
    let mut s = state_in(tmp.path(), "Fresh");
    assert!(!s.can_create(), "unchecked");
    s.target = check_target(&s.name, &s.location, s.make_folder);
    assert_eq!(s.target, TargetStatus::Clear);
    assert!(s.can_create());
    s.edit_rule(PcbField::Clearance, "oops".into());
    assert!(!s.can_create(), "a bad value switches Create off");
    s.edit_rule(PcbField::Clearance, "0.2".into());
    assert!(s.can_create());
    assert_eq!(s.setup.pcb.clearance_mm, 0.2);
}

#[test]
fn a_bad_value_keeps_the_last_good_one_and_says_why() {
    let mut s = NewProjectState::new(String::new());
    s.edit_rule(PcbField::ViaDrill, "0.9".into());
    assert!(
        s.drafts.problem(PcbField::ViaDrill).is_some(),
        "drill >= via"
    );
    s.edit_rule(PcbField::ViaSize, "1.2".into());
    assert!(s.drafts.problem(PcbField::ViaDrill).is_none());
    s.edit_rule(PcbField::TrackWidth, "99".into());
    assert!(s.drafts.problem(PcbField::TrackWidth).is_some());
    assert_eq!(s.setup.pcb.track_width_mm, 0.2);
}

#[test]
fn an_existing_project_file_disables_create() {
    let tmp = tempfile::tempdir().unwrap();
    let folder = tmp.path().join("Taken");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("Taken.snxprj"), b"").unwrap();
    // The folder is not empty, so the first thing the check sees is the file.
    assert_eq!(
        check_target("Taken", &tmp.path().display().to_string(), true),
        TargetStatus::Problem(TargetProblem::Exists(folder.join("Taken.snxprj")))
    );
    // Without the folder the target is `<loc>/Taken.snxprj`.
    std::fs::write(tmp.path().join("Taken.snxprj"), b"").unwrap();
    assert_eq!(
        check_target("Taken", &tmp.path().display().to_string(), false),
        TargetStatus::Problem(TargetProblem::Exists(tmp.path().join("Taken.snxprj")))
    );
}

#[test]
fn a_folder_that_is_not_empty_disables_create_and_an_empty_one_does_not() {
    let tmp = tempfile::tempdir().unwrap();
    let loc = tmp.path().display().to_string();
    let folder = tmp.path().join("Busy");
    std::fs::create_dir(&folder).unwrap();
    assert_eq!(
        check_target("Busy", &loc, true),
        TargetStatus::Clear,
        "empty folder"
    );
    std::fs::write(folder.join("notes.txt"), b"x").unwrap();
    assert_eq!(
        check_target("Busy", &loc, true),
        TargetStatus::Problem(TargetProblem::FolderNotEmpty(folder))
    );
    assert_eq!(
        check_target("Busy", &loc, false),
        TargetStatus::Clear,
        "without the folder the user's own folder may hold other files"
    );
}

#[test]
fn a_missing_location_is_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let gone = tmp.path().join("nope").display().to_string();
    assert_eq!(
        check_target("P", &gone, true),
        TargetStatus::Problem(TargetProblem::LocationMissing)
    );
    assert_eq!(
        check_target("P", "  ", true),
        TargetStatus::Problem(TargetProblem::LocationEmpty)
    );
}

#[test]
fn the_menu_opens_the_dialog_and_nothing_is_written_until_create() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, _t) = Signex::new();
    settle(&mut app, Message::Menu(MenuMessage::NewProject));
    assert!(app.ui_state.new_project.is_some());
    dialog(
        &mut app,
        NewProjectMsg::LocationEdited(tmp.path().display().to_string()),
    );
    dialog(&mut app, NewProjectMsg::NameEdited("Opened".into()));
    assert!(app.ui_state.new_project.as_ref().unwrap().can_create());
    assert_eq!(
        std::fs::read_dir(tmp.path()).unwrap().count(),
        0,
        "nothing before Create"
    );

    dialog(&mut app, NewProjectMsg::CancelRequested);
    assert!(app.ui_state.new_project.is_none());
    assert_eq!(
        std::fs::read_dir(tmp.path()).unwrap().count(),
        0,
        "nor after Cancel"
    );
}

#[test]
fn create_writes_the_project_with_the_chosen_defaults() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, _t) = Signex::new();
    settle(&mut app, Message::Menu(MenuMessage::NewProject));
    dialog(
        &mut app,
        NewProjectMsg::LocationEdited(tmp.path().display().to_string()),
    );
    dialog(
        &mut app,
        NewProjectMsg::NameEdited("Motor Driver Rev A".into()),
    );
    dialog(&mut app, NewProjectMsg::ProfilePicked(SetupProfile::Hdi));
    dialog(&mut app, NewProjectMsg::SheetSizePicked("A4".into()));
    dialog(&mut app, NewProjectMsg::SubmitRequested);
    assert!(
        app.ui_state.new_project.is_none(),
        "Create closes the dialog"
    );

    let project = tmp
        .path()
        .join("Motor Driver Rev A")
        .join("Motor Driver Rev A.snxprj");
    let data = signex_types::project::parse_project(&project).expect("project written");
    let pcb = data.project_defaults.pcb.clone().expect("pcb defaults");
    assert_eq!(
        (pcb.copper_layers, pcb.clearance_mm, pcb.thickness_mm),
        (6, 0.1, 1.2)
    );
    assert_eq!(
        data.project_defaults.sheet.as_ref().unwrap().paper_size,
        "A4"
    );
    assert!(data.designator_standard().is_some());
    // The first sheet starts from the sheet defaults.
    let sheet_text = std::fs::read_to_string(project.with_extension("snxsch")).unwrap();
    assert!(sheet_text.contains("A4"), "{sheet_text}");
}

#[test]
fn with_the_folder_off_the_project_goes_straight_into_the_location() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, _t) = Signex::new();
    settle(&mut app, Message::Menu(MenuMessage::NewProject));
    dialog(
        &mut app,
        NewProjectMsg::LocationEdited(tmp.path().display().to_string()),
    );
    dialog(&mut app, NewProjectMsg::NameEdited("Flat".into()));
    dialog(&mut app, NewProjectMsg::MakeFolderToggled(false));
    dialog(&mut app, NewProjectMsg::SubmitRequested);
    assert!(tmp.path().join("Flat.snxprj").is_file());
    assert!(tmp.path().join("Flat.snxsch").is_file());
    assert!(!tmp.path().join("Flat").exists());
}

#[test]
fn create_does_nothing_while_the_target_is_taken() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("Taken.snxprj"), b"mine").unwrap();
    let (mut app, _t) = Signex::new();
    settle(&mut app, Message::Menu(MenuMessage::NewProject));
    dialog(
        &mut app,
        NewProjectMsg::LocationEdited(tmp.path().display().to_string()),
    );
    dialog(&mut app, NewProjectMsg::MakeFolderToggled(false));
    dialog(&mut app, NewProjectMsg::NameEdited("Taken".into()));
    assert!(!app.ui_state.new_project.as_ref().unwrap().can_create());
    dialog(&mut app, NewProjectMsg::SubmitRequested);
    assert!(app.ui_state.new_project.is_some(), "the dialog stays open");
    assert_eq!(
        std::fs::read(tmp.path().join("Taken.snxprj")).unwrap(),
        b"mine"
    );
}

#[test]
fn an_old_check_result_is_dropped() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, _t) = Signex::new();
    settle(&mut app, Message::Menu(MenuMessage::NewProject));
    dialog(
        &mut app,
        NewProjectMsg::LocationEdited(tmp.path().display().to_string()),
    );
    let stale = app.ui_state.new_project.as_ref().unwrap().generation - 1;
    dialog(
        &mut app,
        NewProjectMsg::TargetChecked {
            generation: stale,
            status: TargetStatus::Problem(TargetProblem::LocationEmpty),
        },
    );
    assert_eq!(
        app.ui_state.new_project.as_ref().unwrap().target,
        TargetStatus::Clear
    );
}

#[test]
fn an_imported_rule_file_fills_the_fields_and_a_bad_one_reports_the_key() {
    use signex_types::project::RuleTemplate;
    let (mut app, _t) = Signex::new();
    settle(&mut app, Message::Menu(MenuMessage::NewProject));
    let template = RuleTemplate::parse(
        "[pcb]\nclearance_mm=0.127\ntrack_width_mm=0.127\nvia_diameter_mm=0.45\nvia_drill_mm=0.2\ncopper_layers=6\n",
    )
    .unwrap();
    dialog(&mut app, NewProjectMsg::RulesFileRead(Some(Ok(template))));
    let s = app.ui_state.new_project.as_ref().unwrap();
    assert_eq!(
        (s.drafts.clearance.as_str(), s.setup.pcb.copper_layers),
        ("0.127", 6)
    );
    assert!(s.setup.imported);
    let bad =
        super::RuleReadError::File(RuleTemplate::parse("[pcb]\nclearance_mm=0.1").unwrap_err());
    dialog(
        &mut app,
        NewProjectMsg::RulesFileRead(Some(Err(bad.clone()))),
    );
    assert_eq!(
        app.ui_state.new_project.as_ref().unwrap().import_error,
        Some(bad)
    );
}

#[test]
fn a_dialog_message_with_no_dialog_open_does_nothing() {
    let (mut app, _t) = Signex::new();
    dialog(&mut app, NewProjectMsg::SubmitRequested);
    dialog(&mut app, NewProjectMsg::NameEdited("x".into()));
    assert!(app.ui_state.new_project.is_none());
}

#[test]
fn a_failed_create_keeps_the_dialog_and_its_setup_and_says_why() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, _t) = Signex::new();
    settle(&mut app, Message::Menu(MenuMessage::NewProject));
    dialog(
        &mut app,
        NewProjectMsg::LocationEdited(tmp.path().display().to_string()),
    );
    dialog(&mut app, NewProjectMsg::NameEdited("Raced".into()));
    dialog(&mut app, NewProjectMsg::ProfilePicked(SetupProfile::Hdi));
    // The target is taken after the check said it was clear.
    let task = app.update(Message::from(NewProjectMsg::SubmitRequested));
    let state = app
        .ui_state
        .new_project
        .as_ref()
        .expect("still open while writing");
    assert!(
        state.creating.is_some() && !state.can_create(),
        "Create is off meanwhile"
    );
    std::fs::create_dir(tmp.path().join("Raced")).unwrap();
    std::fs::write(tmp.path().join("Raced").join("theirs.txt"), b"x").unwrap();
    TaskDriver::new().settle(&mut app, task).expect("settled");
    let state = app
        .ui_state
        .new_project
        .as_ref()
        .expect("the dialog stays open");
    assert!(
        state.create_error.is_some(),
        "the reason is shown in the dialog"
    );
    assert!(state.creating.is_none());
    assert_eq!(state.setup.profile, SetupProfile::Hdi, "the setup is kept");
    assert!(
        app.document_state.error_notice.is_none(),
        "no error card over the dialog"
    );
}

#[test]
fn a_location_that_is_itself_a_project_is_refused_without_the_folder() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("Old.snxprj"), b"").unwrap();
    let status = check_target("New", &tmp.path().display().to_string(), false);
    assert!(
        matches!(&status, TargetStatus::Problem(TargetProblem::Other(why)) if why.contains("a project already exists")),
        "{status:?}"
    );
}

#[test]
fn enter_submits_again_after_a_dropdown_pick() {
    let (mut app, _t) = Signex::new();
    settle(&mut app, Message::Menu(MenuMessage::NewProject));
    for pick in [
        NewProjectMsg::SheetSizePicked("A4".into()),
        NewProjectMsg::TitleBlockPicked(false),
        NewProjectMsg::DesignatorsPicked(signex_types::designator::DesignatorStandard::Din40719_2),
    ] {
        dialog(&mut app, NewProjectMsg::DropdownToggled(true));
        assert!(app.ui_state.new_project.as_ref().unwrap().dropdown_open);
        dialog(&mut app, pick);
        assert!(
            !app.ui_state.new_project.as_ref().unwrap().dropdown_open,
            "the pick closed the list"
        );
    }
}

#[test]
fn cancel_and_esc_wait_while_create_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, _t) = Signex::new();
    settle(&mut app, Message::Menu(MenuMessage::NewProject));
    dialog(
        &mut app,
        NewProjectMsg::LocationEdited(tmp.path().display().to_string()),
    );
    dialog(&mut app, NewProjectMsg::NameEdited("Busy".into()));
    let task = app.update(Message::from(NewProjectMsg::SubmitRequested));
    let _ = app.update(Message::from(NewProjectMsg::CancelRequested));
    assert!(
        app.ui_state.new_project.is_some(),
        "Cancel does not close the dialog while Create writes"
    );
    TaskDriver::new().settle(&mut app, task).expect("settled");
    assert!(
        app.ui_state.new_project.is_none(),
        "it closes when Create lands"
    );
    assert!(tmp.path().join("Busy").join("Busy.snxprj").is_file());
}

#[test]
fn a_create_that_cannot_start_says_why_and_does_not_hang() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, _t) = Signex::new();
    settle(&mut app, Message::Menu(MenuMessage::NewProject));
    dialog(
        &mut app,
        NewProjectMsg::LocationEdited(tmp.path().display().to_string()),
    );
    dialog(&mut app, NewProjectMsg::NameEdited("Twice".into()));
    let chosen = tmp.path().join("Twice.snxprj");
    assert!(app.document_state.pending_opens.insert(chosen));
    dialog(&mut app, NewProjectMsg::SubmitRequested);
    let state = app.ui_state.new_project.as_ref().unwrap();
    assert!(state.creating.is_none(), "not left waiting");
    assert!(state.create_error.is_some(), "the reason is shown");
}

#[test]
fn changing_the_name_location_or_folder_clears_a_stale_create_error() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut app, _t) = Signex::new();
    settle(&mut app, Message::Menu(MenuMessage::NewProject));
    for change in [
        NewProjectMsg::NameEdited("Other".into()),
        NewProjectMsg::LocationEdited(tmp.path().display().to_string()),
        NewProjectMsg::MakeFolderToggled(false),
    ] {
        app.ui_state.new_project.as_mut().unwrap().create_error = Some("old reason".into());
        dialog(&mut app, change.clone());
        assert!(
            app.ui_state
                .new_project
                .as_ref()
                .unwrap()
                .create_error
                .is_none(),
            "{change:?} left the old reason"
        );
    }
}
