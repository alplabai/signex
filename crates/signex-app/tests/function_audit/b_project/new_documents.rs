//! Rows m21 (New Project) and m22 (New PCB / New Schematic).

use signex_app::app::{
    DocumentSetupMsg, Message, NewProjectMsg, ProjectMsg, SetupKind, SheetChoice, Signex,
    TabDocument,
};
use signex_app::menu_bar::MenuMessage;
use signex_types::format::SnxSchematic;
use signex_types::pcb::PcbBoard;
use signex_types::project::{PcbField, ProjectSetup, SetupProfile, SheetDefaults};
use tempfile::TempDir;

use super::support::{Bench, bench, fresh_app};
use crate::settle_support::settle;

const ONE_SHEET: &[(&str, &str)] = &[("root.snxsch", "")];

/// The New Project dialog open on a temp location, named `name`.
fn dialog(name: &str) -> (Signex, TempDir) {
    let tmp = TempDir::new().expect("tempdir");
    let mut app = fresh_app();
    settle(&mut app, Message::Menu(MenuMessage::NewProject));
    edit(
        &mut app,
        NewProjectMsg::LocationEdited(tmp.path().display().to_string()),
    );
    edit(&mut app, NewProjectMsg::NameEdited(name.to_string()));
    (app, tmp)
}

fn edit(app: &mut Signex, msg: NewProjectMsg) {
    settle(app, Message::from(msg));
}

#[test]
fn m21_create_writes_the_project_in_its_named_folder_and_opens_it() {
    let (mut app, tmp) = dialog("dcdc-buck");
    edit(&mut app, NewProjectMsg::SubmitRequested);
    assert!(tmp.path().join("dcdc-buck/dcdc-buck.snxprj").is_file());
    assert!(app.ui_state.new_project.is_none(), "the dialog closes");
    assert_eq!(app.document_state.projects.len(), 1);
    assert_eq!(app.document_state.projects[0].data.name, "dcdc-buck");
}

#[test]
fn m21_the_folder_box_off_writes_the_project_in_the_location_itself() {
    let (mut app, tmp) = dialog("flat");
    edit(&mut app, NewProjectMsg::MakeFolderToggled(false));
    edit(&mut app, NewProjectMsg::SubmitRequested);
    assert!(tmp.path().join("flat.snxprj").is_file());
    assert!(!tmp.path().join("flat").exists());
}

#[test]
fn m21_the_picked_card_sets_the_rules_the_project_stores() {
    let (mut app, _tmp) = dialog("hdi-board");
    edit(&mut app, NewProjectMsg::ProfilePicked(SetupProfile::Hdi));
    edit(&mut app, NewProjectMsg::SubmitRequested);
    let data = &app.document_state.projects[0].data;
    let pcb = data.project_defaults.pcb.as_ref().expect("stored rules");
    let hdi = SetupProfile::Hdi.pcb(pcb.copper_layers);
    assert_eq!(pcb.clearance_mm, hdi.clearance_mm);
    assert_eq!(pcb.track_width_mm, hdi.track_width_mm);
}

#[test]
fn m21_a_typed_rule_value_is_stored_in_the_project() {
    let (mut app, _tmp) = dialog("typed");
    edit(
        &mut app,
        NewProjectMsg::RuleEdited(PcbField::Clearance, "0.3".into()),
    );
    edit(&mut app, NewProjectMsg::SubmitRequested);
    let data = &app.document_state.projects[0].data;
    let pcb = data.project_defaults.pcb.as_ref().expect("stored rules");
    assert_eq!(pcb.clearance_mm, 0.3);
}

#[test]
fn m21_the_layer_stepper_changes_the_stored_copper_count() {
    let (mut app, _tmp) = dialog("stepped");
    edit(
        &mut app,
        NewProjectMsg::CardLayersStepped(SetupProfile::Standard, 2),
    );
    edit(&mut app, NewProjectMsg::SubmitRequested);
    let data = &app.document_state.projects[0].data;
    let pcb = data.project_defaults.pcb.as_ref().expect("stored rules");
    assert_eq!(pcb.copper_layers, 6, "Standard starts at 4 and steps by 2");
}

#[test]
fn m21_cancel_writes_nothing_and_opens_nothing() {
    let (mut app, tmp) = dialog("cancelled");
    edit(&mut app, NewProjectMsg::CancelRequested);
    assert!(app.ui_state.new_project.is_none());
    assert!(app.document_state.projects.is_empty());
    assert!(!tmp.path().join("cancelled").exists());
}

#[test]
fn m21_a_name_with_a_path_separator_is_refused() {
    let (mut app, tmp) = dialog("a/b");
    edit(&mut app, NewProjectMsg::SubmitRequested);
    assert!(app.ui_state.new_project.is_some(), "the dialog stays");
    assert!(app.document_state.projects.is_empty());
    assert!(!tmp.path().join("a").exists());
}

#[test]
fn m21_a_drill_not_smaller_than_the_via_keeps_create_off() {
    let (mut app, tmp) = dialog("bad-drill");
    edit(
        &mut app,
        NewProjectMsg::RuleEdited(PcbField::ViaDrill, "0.9".into()),
    );
    edit(&mut app, NewProjectMsg::SubmitRequested);
    assert!(app.ui_state.new_project.is_some(), "the dialog stays");
    assert!(!tmp.path().join("bad-drill").exists());
}

#[test]
fn m21_create_never_overwrites_a_project_already_there() {
    let (mut app, tmp) = dialog("taken");
    let folder = tmp.path().join("taken");
    std::fs::create_dir(&folder).expect("folder");
    let existing = folder.join("taken.snxprj");
    std::fs::write(&existing, b"existing project").expect("file");
    // The name is typed again, so the disk check sees the file.
    edit(&mut app, NewProjectMsg::NameEdited("taken".into()));
    edit(&mut app, NewProjectMsg::SubmitRequested);
    assert_eq!(std::fs::read(&existing).expect("file"), b"existing project");
    assert!(app.ui_state.new_project.is_some(), "the dialog stays");
}

fn open_setup(b: &mut Bench, kind: SetupKind) {
    b.send(Message::from(DocumentSetupMsg::OpenRequested {
        kind,
        project: None,
    }));
}

fn setup(b: &mut Bench, msg: NewProjectMsg) {
    b.send(Message::from(DocumentSetupMsg::Setup(msg)));
}

fn board_at<'a>(b: &'a Bench, name: &str) -> Option<&'a PcbBoard> {
    let path = b.file(name);
    b.app
        .document_state
        .tabs
        .iter()
        .find(|tab| tab.path == path)
        .and_then(|tab| tab.cached_document.as_ref())
        .and_then(TabDocument::as_pcb)
}

#[test]
fn m22_new_pcb_makes_an_unsaved_board_with_the_shown_rules() {
    let mut b = bench(ONE_SHEET);
    open_setup(&mut b, SetupKind::Pcb);
    setup(&mut b, NewProjectMsg::ProfilePicked(SetupProfile::Hdi));
    setup(
        &mut b,
        NewProjectMsg::RuleEdited(PcbField::Thickness, "1.0".into()),
    );
    b.send(Message::from(DocumentSetupMsg::SubmitRequested));
    assert!(b.app.ui_state.document_setup.is_none());
    let board = board_at(&b, "Proj.snxpcb").expect("the new board is open");
    assert_eq!(board.thickness, 1.0);
    let rules = board.setup.as_ref().expect("board setup");
    assert_eq!(
        rules.clearance,
        SetupProfile::Hdi.pcb(board_copper(board)).clearance_mm
    );
    assert!(
        !b.file("Proj.snxpcb").exists(),
        "the board opens unsaved: nothing is written until Save"
    );
}

fn board_copper(board: &PcbBoard) -> u8 {
    let count = board
        .layers
        .iter()
        .filter(|layer| layer.layer_type == "copper")
        .count();
    u8::try_from(count).expect("a layer count")
}

#[test]
fn m22_cancel_makes_no_board_and_leaves_the_project_clean() {
    let mut b = bench(ONE_SHEET);
    let tabs = b.app.document_state.tabs.len();
    open_setup(&mut b, SetupKind::Pcb);
    setup(&mut b, NewProjectMsg::ProfilePicked(SetupProfile::Hdi));
    b.send(Message::from(DocumentSetupMsg::CancelRequested));
    assert!(b.app.ui_state.document_setup.is_none());
    assert_eq!(b.app.document_state.tabs.len(), tabs);
    assert!(b.data().pcb_file.is_none());
    assert!(!b.dirty());
}

#[test]
fn m22_a_drill_not_smaller_than_the_via_disables_create() {
    let mut b = bench(ONE_SHEET);
    open_setup(&mut b, SetupKind::Pcb);
    setup(
        &mut b,
        NewProjectMsg::RuleEdited(PcbField::ViaDrill, "0.9".into()),
    );
    let state = b.app.ui_state.document_setup.as_ref().expect("the dialog");
    assert!(!state.create_enabled());
    b.send(Message::from(DocumentSetupMsg::SubmitRequested));
    assert!(b.app.ui_state.document_setup.is_some(), "the dialog stays");
    assert!(board_at(&b, "Proj.snxpcb").is_none());
}

#[test]
fn m22_a_project_without_a_setup_shows_the_cards() {
    let mut b = bench(ONE_SHEET);
    open_setup(&mut b, SetupKind::Pcb);
    let state = b.app.ui_state.document_setup.as_ref().expect("the dialog");
    assert!(state.shows_cards());
    assert!(!state.shows_project_summary());
    assert!(state.create_enabled(), "it opens valid");
}

#[test]
fn m22_a_project_with_a_setup_folds_the_cards_behind_its_summary_row() {
    let mut b = bench(ONE_SHEET);
    b.app.document_state.projects[0].data.project_defaults =
        ProjectSetup::from_profile(SetupProfile::Advanced).defaults();
    open_setup(&mut b, SetupKind::Pcb);
    let state = b.app.ui_state.document_setup.as_ref().expect("the dialog");
    assert!(state.shows_project_summary());
    assert!(!state.shows_cards());
    b.send(Message::from(DocumentSetupMsg::OtherProfileToggled));
    let state = b.app.ui_state.document_setup.as_ref().expect("the dialog");
    assert!(state.shows_cards(), "Use another profile unfolds the cards");
}

#[test]
fn m22_keep_project_setup_takes_the_projects_rules_back() {
    let mut b = bench(ONE_SHEET);
    let advanced = ProjectSetup::from_profile(SetupProfile::Advanced).defaults();
    let wanted = advanced.pcb.clone().expect("the tier's rules");
    b.app.document_state.projects[0].data.project_defaults = advanced;
    open_setup(&mut b, SetupKind::Pcb);
    b.send(Message::from(DocumentSetupMsg::OtherProfileToggled));
    setup(&mut b, NewProjectMsg::ProfilePicked(SetupProfile::Hdi));
    b.send(Message::from(DocumentSetupMsg::OtherProfileToggled));
    b.send(Message::from(DocumentSetupMsg::SubmitRequested));
    let board = board_at(&b, "Proj.snxpcb").expect("the new board");
    let rules = board.setup.as_ref().expect("board setup");
    assert_eq!(rules.clearance, wanted.clearance_mm);
    assert_eq!(rules.trace_width, wanted.track_width_mm);
}

#[test]
fn m22_new_schematic_opens_the_sheet_dialog_and_cancel_adds_nothing() {
    let mut b = bench(ONE_SHEET);
    open_setup(&mut b, SetupKind::Sheet);
    let state = b.app.ui_state.document_setup.as_ref().expect("the dialog");
    assert_eq!(state.kind, SetupKind::Sheet);
    b.send(Message::from(DocumentSetupMsg::CancelRequested));
    assert!(b.app.ui_state.document_setup.is_none());
    assert_eq!(b.listed(), ["root.snxsch"]);
}

#[test]
fn m22_the_new_sheet_is_written_with_the_chosen_size_and_listed() {
    let mut b = bench(ONE_SHEET);
    let path = b.file("power.snxsch");
    let project_id = b.id;
    b.send(Message::Project(ProjectMsg::AddNewSchematicPicked {
        project_id,
        path: Some(path.clone()),
        choice: Some(SheetChoice {
            sheet: SheetDefaults {
                paper_size: "A2".to_string(),
                show_title_block: false,
            },
            designators: None,
        }),
    }));
    assert!(b.listed().contains(&"power.snxsch".to_string()));
    let text = std::fs::read_to_string(&path).expect("the sheet is written");
    let sheet = SnxSchematic::parse(&text).expect("it parses").sheet;
    assert_eq!(sheet.paper_size, "A2");
    assert!(!sheet.frame.show_title_block);
}
