//! #2400: the board's net classes through `Signex::update` (the Net Classes
//! panel) and through the commands an agent runs: the project's table, each
//! edit one step of its history, and DRC checking a class's clearance.

use std::path::{Path, PathBuf};

use serde_json::Value;
use signex_types::pcb::{NetDef, PcbBoard, Point, Segment};
use uuid::Uuid;

use crate::app::command::queries::check_drc_tests::drc_serial;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, Signex};
use crate::dock::DockMessage;
use crate::keymap::AppCommandId;
use crate::panels::{BoardNetClassField, BoardNetClassMsg, NetClassMsg, PanelMsg};

const BOARD: &str = "board.snxpcb";

fn track(n: u128, net: u32, y: f64) -> Segment {
    Segment {
        uuid: Uuid::from_u128(0x3000 + n),
        start: Point::new(0.0, y),
        end: Point::new(10.0, y),
        width: 0.25,
        layer: "TopCopper".into(),
        net,
        unknown_columns: Default::default(),
    }
}

/// GND and VCC 0.25 mm apart: clean at the Standard setup's 0.15 mm.
fn board() -> PcbBoard {
    let mut board = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    board.nets = [(1, "GND"), (2, "VCC")]
        .into_iter()
        .map(|(number, name)| NetDef {
            number,
            name: name.into(),
        })
        .collect();
    board.segments = vec![track(1, 1, 0.0), track(2, 2, 0.5)];
    board
}

fn board_app(dir: &tempfile::TempDir) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    app.ui_state.machine_drc_rules = Default::default();
    app.document_state.projects[0].data.pcb_file = Some(BOARD.into());
    let path = dir.path().join(BOARD);
    app.open_pcb_tab(path.clone(), BOARD.into(), board());
    (app, path)
}

fn panel(app: &mut Signex, msg: BoardNetClassMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::BoardNetClass(
        msg,
    ))));
}

fn type_in(app: &mut Signex, class: &str, field: BoardNetClassField, text: &str) {
    panel(
        app,
        BoardNetClassMsg::CellEdited {
            class: class.into(),
            field,
            text: text.into(),
        },
    );
    panel(app, BoardNetClassMsg::CellSubmitted);
}

fn run(app: &mut Signex, command: &str, operands: &[&str]) -> CommandOutcome {
    let id = AppCommandId::new(command).expect("a catalog id");
    let args = CommandArgs::parse_operands(&id, operands).expect("operands parse");
    app.run_operands_settled(&id, &args).0
}

fn answer(outcome: CommandOutcome) -> Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn document(path: &Path) -> String {
    format!("document={}", path.display())
}

fn notice(app: &Signex) -> Option<String> {
    app.document_state
        .panel_ctx
        .net_classes
        .board
        .as_ref()
        .and_then(|view| view.notice.clone())
}

/// The issue's test, on the panel: create a class, give it a clearance and
/// a net, and DRC checks the class's clearance where the board's passes.
#[test]
fn a_class_made_in_the_panel_is_the_project_table_and_drc_uses_its_clearance() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir);
    panel(&mut app, BoardNetClassMsg::AddRequested);
    type_in(&mut app, "Class 1", BoardNetClassField::Name, "Power");
    type_in(&mut app, "Power", BoardNetClassField::Clearance, "0.3");
    panel(
        &mut app,
        BoardNetClassMsg::NetPicked {
            class: "Power".into(),
            net: "VCC".into(),
        },
    );
    let table = &app.document_state.projects[0].data.net_classes;
    assert_eq!(table.len(), 1, "{table:?}");
    assert_eq!(
        (table[0].name.as_str(), table[0].clearance, &table[0].nets),
        ("Power", 0.3, &vec!["VCC".to_string()])
    );
    let shown = app
        .document_state
        .panel_ctx
        .net_classes
        .board
        .clone()
        .expect("the board's view");
    assert!(shown.in_project && shown.classes == *table);
    assert_eq!(shown.nets, ["GND", "VCC"]);

    let task = app.handle_pcb_drc_requested();
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the run lands");
    let report = &app
        .interaction_state
        .pcb_drc
        .done_for(&path)
        .expect("a report")
        .report;
    assert_eq!(report.violations.len(), 1, "{report:?}");
    assert_eq!(
        report.violations[0].kind,
        signex_drc::ViolationKind::Clearance
    );
    assert_eq!(report.violations[0].required_nm, 300_000);
}

/// Each panel edit is one step of the project's net-class history: Undo
/// takes back the net pick, then the width, Redo puts the width back.
#[test]
fn every_panel_edit_is_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = board_app(&dir);
    panel(&mut app, BoardNetClassMsg::AddRequested);
    type_in(&mut app, "Class 1", BoardNetClassField::Width, "0.4");
    panel(
        &mut app,
        BoardNetClassMsg::NetPicked {
            class: "Class 1".into(),
            net: "GND".into(),
        },
    );
    let class = |app: &Signex| app.document_state.projects[0].data.net_classes[0].clone();
    panel(&mut app, BoardNetClassMsg::UndoRequested);
    assert!(class(&app).nets.is_empty());
    assert_eq!(class(&app).trace_width, 0.4);
    panel(&mut app, BoardNetClassMsg::UndoRequested);
    assert_eq!(class(&app).trace_width, 0.0);
    panel(&mut app, BoardNetClassMsg::RedoRequested);
    assert_eq!(class(&app).trace_width, 0.4);
    panel(
        &mut app,
        BoardNetClassMsg::NetRemoved {
            class: "Class 1".into(),
            net: "GND".into(),
        },
    );
    panel(
        &mut app,
        BoardNetClassMsg::DeleteRequested("Class 1".into()),
    );
    assert!(app.document_state.projects[0].data.net_classes.is_empty());
}

/// Text that is not a number is refused with a notice and changes nothing.
#[test]
fn a_typed_value_that_is_not_a_number_is_refused_with_a_notice() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = board_app(&dir);
    panel(&mut app, BoardNetClassMsg::AddRequested);
    type_in(&mut app, "Class 1", BoardNetClassField::Width, "wide");
    assert_eq!(
        app.document_state.projects[0].data.net_classes[0].trace_width,
        0.0
    );
    assert!(
        notice(&app).is_some_and(|n| n.contains("width")),
        "{:?}",
        notice(&app)
    );
    type_in(&mut app, "Class 1", BoardNetClassField::Color, "#11223");
    assert!(notice(&app).is_some_and(|n| n.contains("#RRGGBB")));
}

/// The agent path: add, assign, set the colour and rules as one step,
/// rename and delete, each answering with the table.
#[test]
fn the_commands_edit_the_table_and_answer_with_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir);
    let doc = document(&path);
    let got = answer(run(&mut app, "add_net_class", &[&doc, "name=Power"]));
    assert_eq!(got["classes"][0]["name"], "Power");
    answer(run(
        &mut app,
        "assign_board_net_class",
        &[&doc, "net_name=VCC", "class=Power"],
    ));
    let got = answer(run(
        &mut app,
        "set_net_class_rules",
        &[
            &doc,
            "class=Power",
            "color=#ff0000",
            "width=0.4",
            "clearance=0.3",
        ],
    ));
    let power = &got["classes"][0];
    assert_eq!(power["nets"][0], "VCC");
    assert_eq!(power["trace_width"], 0.4);
    assert_eq!(power["clearance"], 0.3);
    assert_eq!(power["style"]["color"], "#FF0000");
    // Colour and rules were one step.
    assert!(app.step_board_net_classes(&path, false));
    let back = &app.document_state.projects[0].data.net_classes[0];
    assert_eq!((back.trace_width, back.style.color), (0.0, None));
    assert_eq!(back.nets, ["VCC"]);

    answer(run(
        &mut app,
        "rename_net_class",
        &[&doc, "class=Power", "name=Rails"],
    ));
    let refused = run(
        &mut app,
        "set_net_class_rules",
        &[&doc, "class=Rails", "width=-1"],
    );
    assert!(
        matches!(refused, CommandOutcome::Disabled { .. }),
        "{refused:?}"
    );
    let got = answer(run(&mut app, "delete_net_class", &[&doc, "class=Rails"]));
    assert_eq!(got["classes"].as_array().map(Vec::len), Some(0));
}

/// A board in no open project has no table: the commands say so.
#[test]
fn a_loose_board_has_no_net_classes() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir);
    app.document_state.projects[0].data.pcb_file = None;
    let refused = run(&mut app, "add_net_class", &[&document(&path)]);
    assert!(
        matches!(&refused, CommandOutcome::Disabled { reason } if reason.contains("no open project")),
        "{refused:?}"
    );
}

fn schematic_panel(app: &mut Signex, msg: NetClassMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::NetClass(msg))));
}

/// #2399: a net-class edit marks the DRC results stale, though the board's
/// revision does not move; an Undo back to the table the run read makes
/// them current again.
#[test]
fn a_net_class_edit_marks_the_drc_results_stale() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = board_app(&dir);
    let task = app.handle_pcb_drc_requested();
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the run lands");
    assert!(!app.pcb_drc_panel().unwrap().stale);
    panel(&mut app, BoardNetClassMsg::AddRequested);
    assert!(app.pcb_drc_panel().unwrap().stale, "a class was added");
    panel(&mut app, BoardNetClassMsg::UndoRequested);
    assert!(
        !app.pcb_drc_panel().unwrap().stale,
        "back to the run's table"
    );
}

/// A board-panel edit keeps the schematic section's selection on the class
/// it named and reloads its draft, so the draft never lands on another one.
#[test]
fn a_board_edit_keeps_the_schematic_draft_on_its_class() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = board_app(&dir);
    for _ in 0..3 {
        panel(&mut app, BoardNetClassMsg::AddRequested);
    }
    schematic_panel(&mut app, NetClassMsg::Select(1));
    let section = |app: &Signex| app.document_state.panel_ctx.net_classes.clone();
    assert_eq!(section(&app).draft.name, "Class 2");

    panel(
        &mut app,
        BoardNetClassMsg::DeleteRequested("Class 1".into()),
    );
    let shown = section(&app);
    assert_eq!(shown.selected, 0);
    assert_eq!(shown.draft.name, "Class 2");

    panel(&mut app, BoardNetClassMsg::UndoRequested);
    let shown = section(&app);
    assert_eq!(shown.selected, 1);
    assert_eq!(shown.draft.name, "Class 2");
}

/// The typed cell and the refusal notice live in the Model: a panel
/// refresh with no active project keeps them.
#[test]
fn the_board_draft_and_notice_survive_a_refresh_without_an_active_project() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = board_app(&dir);
    panel(&mut app, BoardNetClassMsg::AddRequested);
    type_in(&mut app, "Class 1", BoardNetClassField::Width, "wide");
    app.document_state.active_project = None;
    panel(
        &mut app,
        BoardNetClassMsg::CellEdited {
            class: "Class 1".into(),
            field: BoardNetClassField::Clearance,
            text: "0.2".into(),
        },
    );
    app.refresh_panel_ctx();
    let view = app
        .document_state
        .panel_ctx
        .net_classes
        .board
        .clone()
        .expect("the board's view");
    assert_eq!(view.draft.map(|d| d.text).as_deref(), Some("0.2"));
    assert!(view.notice.is_some(), "the refusal is still shown");
    panel(&mut app, BoardNetClassMsg::CellSubmitted);
    assert_eq!(
        app.document_state.projects[0].data.net_classes[0].clearance,
        0.2
    );
}
