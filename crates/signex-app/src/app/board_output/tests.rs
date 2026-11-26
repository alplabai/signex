//! #2404 — board reports through `Signex::update` (menu row, palette
//! command, Board panel button, tabs, CSV) and through the headless path an
//! agent and the CLI take.

use std::path::{Path, PathBuf};

use signex_output::{BoardReportKind, ReportCell};
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::{BoardOutputDialog, BoardOutputMsg};
use signex_i18n::RenderTranslatable;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::settle;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;

/// R1 and R2 (10k, one footprint), C1 on the bottom and a mechanical
/// standoff MH1. VCC has two tracks (3 mm and 4 mm) and a via.
pub(super) fn board() -> PcbBoard {
    let pad = |number: &str, net: u32, name: &str| {
        serde_json::json!({
            "uuid": Uuid::now_v7(), "number": number, "pad_type": "smd",
            "shape": "rect", "position": {"x": 0.0, "y": 0.0},
            "size": {"x": 1.0, "y": 1.0}, "drill": null,
            "net": {"number": net, "name": name}
        })
    };
    let part = |reference: &str, value: &str, x: f64, layer: &str| {
        serde_json::json!({
            "uuid": Uuid::now_v7(), "reference": reference, "value": value,
            "footprint_id": "R_0603", "position": {"x": x, "y": 10.0},
            "layer": layer,
            "pads": [pad("1", 1, "VCC"), pad("2", 2, "GND")]
        })
    };
    let mut standoff = part("MH1", "M3", 40.0, "TopCopper");
    standoff["component"] = serde_json::json!({ "type": "mechanical" });
    let track = |x0: f64, y0: f64, x1: f64, y1: f64| {
        serde_json::json!({
            "uuid": Uuid::now_v7(), "start": {"x": x0, "y": y0},
            "end": {"x": x1, "y": y1}, "width": 0.25,
            "layer": "TopCopper", "net": 1
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "outline": [
            {"x": 0.0, "y": 0.0}, {"x": 50.0, "y": 0.0},
            {"x": 50.0, "y": 30.0}, {"x": 0.0, "y": 30.0}
        ],
        "nets": [{"number": 1, "name": "VCC"}, {"number": 2, "name": "GND"}],
        "footprints": [
            part("R2", "10k", 20.0, "TopCopper"),
            part("R1", "10k", 10.0, "TopCopper"),
            part("C1", "100n", 30.0, "BottomCopper"),
            standoff
        ],
        "segments": [track(0.0, 0.0, 3.0, 0.0), track(3.0, 0.0, 3.0, 4.0)],
        "vias": [{"uuid": Uuid::now_v7(), "position": {"x": 3.0, "y": 4.0},
                  "diameter": 0.6, "drill": 0.3, "net": 1}]
    }))
    .expect("the fixture board")
}

/// An app with the fixture board open in its tab, in a project that names it.
pub(super) fn app_with_board(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board());
    (app, path)
}

pub(super) fn run(app: &mut Signex, id: &str) {
    let _ = app.update(Message::Menu(MenuMessage::command(id)));
}

pub(super) fn output(app: &mut Signex, msg: BoardOutputMsg) -> iced::Task<Message> {
    app.update(Message::from(msg))
}

fn shown_kind(app: &Signex) -> Option<BoardReportKind> {
    match app.ui_state.board_output.dialog.as_deref()? {
        BoardOutputDialog::Reports(state) => Some(state.report.kind),
        BoardOutputDialog::Print(_) => None,
    }
}

fn shown_rows(app: &Signex) -> Vec<Vec<ReportCell>> {
    match app.ui_state.board_output.dialog.as_deref() {
        Some(BoardOutputDialog::Reports(state)) => state.report.rows.clone(),
        _ => Vec::new(),
    }
}

fn text(cell: &ReportCell) -> String {
    cell.csv_text()
}

#[test]
fn the_bom_row_opens_the_board_bom_without_the_mechanical_part() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(dir.path());

    run(&mut app, "board_report_bom");

    assert_eq!(shown_kind(&app), Some(BoardReportKind::Bom));
    let rows = shown_rows(&app);
    let designators: Vec<String> = rows.iter().map(|r| text(&r[0])).collect();
    assert_eq!(designators, ["C1", "R1, R2"], "MH1 is mechanical");
    assert_eq!(rows[1][1], ReportCell::Count(2));
}

#[test]
fn each_report_command_opens_its_table_and_a_tab_switches_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(dir.path());
    for (id, kind) in [
        ("board_report_netlist", BoardReportKind::Netlist),
        ("board_report_net_length", BoardReportKind::NetLength),
        ("board_report_components", BoardReportKind::Components),
    ] {
        run(&mut app, id);
        assert_eq!(shown_kind(&app), Some(kind), "{id}");
    }
    let _ = output(
        &mut app,
        BoardOutputMsg::ReportPicked(BoardReportKind::NetLength),
    );
    let rows = shown_rows(&app);
    let vcc = rows.iter().find(|r| text(&r[0]) == "VCC").expect("VCC");
    assert_eq!(vcc[1], ReportCell::Millimetres(7.0), "3 mm + 4 mm");
    assert_eq!(vcc[2], ReportCell::Count(2));
    assert_eq!(vcc[3], ReportCell::Count(1));

    let _ = output(
        &mut app,
        BoardOutputMsg::ReportPicked(BoardReportKind::Netlist),
    );
    let rows = shown_rows(&app);
    let gnd = rows.iter().find(|r| text(&r[0]) == "GND").expect("GND");
    assert_eq!(text(&gnd[2]), "C1.2 MH1.2 R1.2 R2.2");

    let _ = output(
        &mut app,
        BoardOutputMsg::ReportPicked(BoardReportKind::Components),
    );
    let rows = shown_rows(&app);
    let c1 = rows.iter().find(|r| text(&r[0]) == "C1").expect("C1");
    assert_eq!(text(&c1[6]), "bottom");

    let _ = output(&mut app, BoardOutputMsg::Closed);
    assert!(app.ui_state.board_output.dialog.is_none());
}

#[test]
fn a_tab_switch_after_the_board_is_dropped_says_why() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());
    run(&mut app, "board_report_bom");
    assert_eq!(shown_kind(&app), Some(BoardReportKind::Bom));
    // A failed History restore leaves the tab without its board.
    for idx in 0..app.document_state.tabs.len() {
        if app.document_state.tabs[idx].path == board {
            app.document_state.take_tab_board_engine(idx);
        }
    }
    app.document_state.error_notice = None;
    let _ = output(
        &mut app,
        BoardOutputMsg::ReportPicked(BoardReportKind::Netlist),
    );
    assert!(
        app.document_state.error_notice.is_some(),
        "the click is answered with a notice"
    );
    assert_eq!(shown_kind(&app), Some(BoardReportKind::Bom), "table kept");
}

#[test]
fn the_board_panel_reports_button_opens_board_reports() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(dir.path());
    let msg = crate::panels::PanelMsg::PcbBoard(crate::panels::PcbBoardMsg::ReportsOpened);
    let task = app.update(Message::Dock(crate::dock::DockMessage::Panel(msg)));
    settle(&mut app, task);
    assert_eq!(shown_kind(&app), Some(BoardReportKind::Bom));
}

#[test]
fn the_report_rows_are_greyed_and_refused_without_a_board() {
    let (mut app, _task) = Signex::new();
    let command = AppCommandId::new("board_report_bom").unwrap();
    let (outcome, _) = app.dispatch_command_reporting(&command, CommandArgs::none(), None);
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason }
            if reason.render_en_us() == "No PCB is open"),
        "{outcome:?}"
    );
    assert!(app.ui_state.board_output.dialog.is_none());
}

#[test]
fn export_csv_writes_the_table_in_view() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());
    run(&mut app, "board_report_components");
    let file = dir.path().join("parts.csv");
    let task = output(
        &mut app,
        BoardOutputMsg::CsvPathChosen {
            board,
            kind: BoardReportKind::Components,
            path: file.clone(),
        },
    );
    settle(&mut app, task);
    let csv = std::fs::read_to_string(&file).expect("the CSV was written");
    let csv = csv.trim_start_matches('\u{feff}');
    let mut lines = csv.lines();
    assert_eq!(
        lines.next(),
        Some("designator,footprint,value,x_mm,y_mm,rotation_deg,side")
    );
    assert_eq!(lines.count(), 4, "one line a component");
}

fn headless(
    app: &mut Signex,
    id: &str,
    operands: &[&str],
) -> (CommandOutcome, iced::Task<Message>) {
    let command = AppCommandId::new(id).unwrap();
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args)
}

#[test]
fn an_agent_reads_each_report_as_json() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());
    let document = format!("document={}", board.display());
    let (outcome, _) = headless(&mut app, "board_report_net_length", &[&document]);
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    assert_eq!(answer["kind"], "net_length");
    assert_eq!(answer["columns"][1], "length_mm");
    let rows = answer["rows"].as_array().expect("rows");
    let vcc = rows.iter().find(|r| r[0] == "VCC").expect("VCC");
    assert_eq!(vcc[1], 7.0);
    assert!(
        app.ui_state.board_output.dialog.is_none(),
        "no dialog opens"
    );

    for id in [
        "board_report_bom",
        "board_report_netlist",
        "board_report_components",
    ] {
        let (outcome, _) = headless(&mut app, id, &[&document]);
        assert!(
            matches!(outcome, CommandOutcome::Answered(_)),
            "{id}: {outcome:?}"
        );
    }
}

#[test]
fn a_report_without_its_board_is_refused_headless() {
    let (mut app, _task) = Signex::new();
    let command = AppCommandId::new("board_report_bom").unwrap();
    // The CLI's parse refuses the missing operand first; the handler
    // refuses too when it is reached without one.
    assert!(CommandArgs::parse_operands(&command, &[]).is_err());
    let (outcome, _) = app.run_headless_command_in_test(&command, CommandArgs::none());
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.render_en_us().contains("document")),
        "{outcome:?}"
    );
}

#[test]
fn the_cli_writes_a_report_as_csv() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());
    let file = dir.path().join("bom.csv");
    let document = format!("document={}", board.display());
    let path = format!("path={}", file.display());
    let (outcome, task) = headless(
        &mut app,
        "export_board_report",
        &[&document, "kind=bom", &path],
    );
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    assert_eq!(answer["rows"], 2);
    settle(&mut app, task);
    let csv = std::fs::read_to_string(&file).expect("written");
    assert!(csv.contains("\"R1, R2\",2,10k,R_0603,,standard"), "{csv}");

    let (outcome, _) = headless(
        &mut app,
        "export_board_report",
        &[&document, "kind=gerber", &path],
    );
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.render_en_us().contains("`kind`")),
        "{outcome:?}"
    );
}
