//! #2399: the DRC results panel through `Signex::update` and through the
//! commands an agent runs: a run's violations listed grouped by rule, a row
//! picked to select its objects and centre the view, the filters, a waiver
//! with its note as one undo step, and the results going stale.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use pcb_geom::ItemRef;
use serde_json::Value;
use signex_types::pcb::{NetDef, PcbBoard, Point, Segment};
use signex_types::violation::Severity;
use uuid::Uuid;

use crate::app::command::queries::check_drc_tests::drc_serial;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::dock::DockMessage;
use crate::keymap::AppCommandId;
use crate::panels::{DrcFilterChoice, DrcPanelMsg, PanelKind, PanelMsg};

const BOARD: &str = "board.snxpcb";
const GND_TRACK: Uuid = Uuid::from_u128(0x2001);
const VCC_TRACK: Uuid = Uuid::from_u128(0x2002);
const AUX_TRACK: Uuid = Uuid::from_u128(0x2004);

fn track(id: Uuid, net: u32, a: (f64, f64), b: (f64, f64)) -> Segment {
    Segment {
        uuid: id,
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        width: 0.25,
        layer: "TopCopper".into(),
        net,
        unknown_columns: Default::default(),
    }
}

/// Two violations on the Standard setup (0.15 mm clearance): GND and VCC
/// 0.1 mm apart (a clearance), AUX across GND (a short).
fn board() -> PcbBoard {
    let mut board = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    board.nets = [(1, "GND"), (2, "VCC"), (4, "AUX")]
        .into_iter()
        .map(|(number, name)| NetDef {
            number,
            name: name.into(),
        })
        .collect();
    board.segments = vec![
        track(GND_TRACK, 1, (0.0, 0.0), (10.0, 0.0)),
        track(VCC_TRACK, 2, (0.0, 0.35), (10.0, 0.35)),
        track(AUX_TRACK, 4, (5.0, -0.1), (5.0, -5.0)),
    ];
    board
}

/// The board open in its tab, in the project, on an 800 x 600 canvas.
fn board_app(dir: &tempfile::TempDir) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    // The built-in severities, whatever the shared test prefs hold.
    app.ui_state.machine_drc_rules = Default::default();
    app.document_state.projects[0].data.pcb_file = Some(BOARD.into());
    let path = dir.path().join(BOARD);
    app.open_pcb_tab(path.clone(), BOARD.into(), board());
    app.interaction_state
        .pcb_canvas
        .set_viewport(iced::Rectangle::new(
            iced::Point::ORIGIN,
            iced::Size::new(800.0, 600.0),
        ));
    (app, path)
}

fn run_drc(app: &mut Signex) {
    let task = app.handle_pcb_drc_requested();
    crate::task_driver::TaskDriver::new()
        .settle(app, task)
        .expect("the run lands");
}

fn panel(app: &mut Signex, msg: DrcPanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::Drc(msg))));
}

fn rows(app: &Signex) -> Vec<crate::panels::DrcRow> {
    app.pcb_drc_panel().expect("a board in view").rows
}

fn key_of(app: &Signex, rule: &str) -> String {
    rows(app)
        .into_iter()
        .find(|r| r.rule == rule)
        .unwrap_or_else(|| panic!("no {rule} row"))
        .key
}

fn selected(app: &Signex) -> BTreeSet<ItemRef> {
    app.interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect()
}

/// Where board point `(x, y)` is on the canvas, in pixels.
fn on_screen(app: &Signex, x: f32, y: f32) -> (f32, f32) {
    let (ox, oy, scale) = app.interaction_state.pcb_canvas.live_camera();
    (ox + x * scale, oy + y * scale)
}

/// The texts the DRC results panel shows, bidi marks dropped.
fn panel_texts(app: &Signex) -> Vec<String> {
    let ctx = &app.document_state.panel_ctx;
    let view = crate::panels::view_panel(PanelKind::Drc, ctx);
    crate::layout_probe::lay_out_texts(view, iced::Size::new(1400.0, 2000.0))
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
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

/// The issue's test: a run on a board with two violations lists both,
/// grouped by rule with their counts; a click on one selects its objects
/// and centres the view on it.
#[test]
fn a_run_lists_both_violations_and_a_click_selects_and_centres_one() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = board_app(&dir);
    run_drc(&mut app);
    app.refresh_panel_ctx();

    let listed = rows(&app);
    let rules: Vec<&str> = listed.iter().map(|r| r.rule.as_str()).collect();
    assert_eq!(
        rules,
        ["builtin::short", "builtin::clearance"],
        "{listed:?}"
    );
    assert!(listed.iter().all(|r| r.severity == Some(Severity::Error)));
    let texts = panel_texts(&app);
    for heading in ["Short (1)", "Clearance (1)"] {
        assert!(texts.iter().any(|t| t == heading), "{heading}: {texts:?}");
    }

    let clearance = key_of(&app, "builtin::clearance");
    panel(&mut app, DrcPanelMsg::RowPicked(clearance));
    assert_eq!(
        selected(&app),
        BTreeSet::from([ItemRef::Track(GND_TRACK), ItemRef::Track(VCC_TRACK)])
    );
    let at = listed
        .iter()
        .find(|r| r.rule == "builtin::clearance")
        .map(|r| {
            (
                r.x_mm.parse::<f32>().unwrap(),
                r.y_mm.parse::<f32>().unwrap(),
            )
        })
        .unwrap();
    let (sx, sy) = on_screen(&app, at.0, at.1);
    assert!(
        (sx - 400.0).abs() < 0.5 && (sy - 300.0).abs() < 0.5,
        "({sx}, {sy})"
    );
}

/// The same pick through the command an agent runs answers with what it
/// selected; an unknown id is refused with the reason.
#[test]
fn show_drc_violation_selects_through_the_command_and_answers() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir);
    run_drc(&mut app);
    let short = key_of(&app, "builtin::short");
    let id = format!("id={short}");
    let got = answer(run(
        &mut app,
        "show_drc_violation",
        &[&document(&path), &id],
    ));
    assert_eq!(got["rule"], "builtin::short");
    assert_eq!(got["items"].as_array().map(Vec::len), Some(2), "{got}");
    assert!(selected(&app).contains(&ItemRef::Track(AUX_TRACK)));

    let outcome = run(
        &mut app,
        "show_drc_violation",
        &[&document(&path), "id=nope"],
    );
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.contains("nope")),
        "{outcome:?}"
    );
}

/// Filters by rule, layer and severity narrow the list; `all` clears one.
#[test]
fn the_filters_narrow_the_list_on_the_panel_and_through_the_command() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir);
    run_drc(&mut app);
    panel(
        &mut app,
        DrcPanelMsg::FilterPicked(DrcFilterChoice::Rule(Some("builtin::short".into()))),
    );
    let shown = app.drc_results_json();
    assert_eq!(shown["rows"].as_array().map(Vec::len), Some(1), "{shown}");
    assert_eq!(shown["rows"][0]["rule"], "builtin::short");

    let got = answer(run(
        &mut app,
        "filter_drc_results",
        &[
            &document(&path),
            "rule=all",
            "layer=TopCopper",
            "severity=error",
        ],
    ));
    assert_eq!(got["rows"].as_array().map(Vec::len), Some(2), "{got}");
    let none = answer(run(
        &mut app,
        "filter_drc_results",
        &[&document(&path), "severity=warning"],
    ));
    assert_eq!(none["rows"].as_array().map(Vec::len), Some(0), "{none}");
    let refused = run(
        &mut app,
        "filter_drc_results",
        &[&document(&path), "layer=Nowhere"],
    );
    assert!(
        matches!(refused, CommandOutcome::Disabled { .. }),
        "{refused:?}"
    );
}

/// A waiver is stored on the board with its note as one undo step, hides
/// the row until Show waived, does not make the results stale, and goes
/// with Unwaive or Undo.
#[test]
fn a_waiver_is_stored_with_its_note_as_one_undo_step() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir);
    run_drc(&mut app);
    let clearance = key_of(&app, "builtin::clearance");

    panel(&mut app, DrcPanelMsg::WaiveStarted(clearance.clone()));
    panel(
        &mut app,
        DrcPanelMsg::NoteEdited("checked on the prototype".into()),
    );
    panel(&mut app, DrcPanelMsg::WaiveConfirmed);
    let waivers = &app
        .document_state
        .pcb_board(&path)
        .unwrap()
        .options
        .drc_waivers;
    assert_eq!(waivers.len(), 1);
    assert_eq!(waivers[0].key, clearance);
    assert_eq!(waivers[0].rule, "builtin::clearance");
    assert_eq!(waivers[0].note, "checked on the prototype");
    let drc = app.pcb_drc_panel().unwrap();
    assert!(!drc.stale, "a waiver changes nothing the check reads");
    assert!(drc.waive_draft.is_none());
    let shown = app.drc_results_json();
    assert_eq!(shown["rows"].as_array().map(Vec::len), Some(1), "{shown}");
    assert_eq!(shown["waived"], 1);
    panel(&mut app, DrcPanelMsg::WaivedShownToggled(true));
    let all = app.drc_results_json();
    assert_eq!(
        all["rows"][1]["waiver"], "checked on the prototype",
        "{all}"
    );

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(
        app.document_state
            .pcb_board(&path)
            .unwrap()
            .options
            .drc_waivers
            .is_empty()
    );
    let _ = app.update(Message::Edit(EditMsg::Redo));
    assert_eq!(
        app.document_state
            .pcb_board(&path)
            .unwrap()
            .options
            .drc_waivers
            .len(),
        1
    );
    panel(&mut app, DrcPanelMsg::Unwaived(clearance));
    assert!(
        app.document_state
            .pcb_board(&path)
            .unwrap()
            .options
            .drc_waivers
            .is_empty()
    );
}

/// The agent path: waive with a note, read it back, unwaive; a second
/// unwaive is refused.
#[test]
fn waive_and_unwaive_run_through_the_commands() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir);
    run_drc(&mut app);
    let short = key_of(&app, "builtin::short");
    let id = format!("id={short}");
    let got = answer(run(
        &mut app,
        "waive_drc_violation",
        &[&document(&path), &id, "text=jumper on purpose"],
    ));
    assert_eq!(got["waived"], true);
    assert_eq!(got["note"], "jumper on purpose");
    let rows = rows(&app);
    let waived = rows.iter().find(|r| r.key == short).unwrap();
    assert_eq!(waived.waiver.as_deref(), Some("jumper on purpose"));
    let got = answer(run(
        &mut app,
        "unwaive_drc_violation",
        &[&document(&path), &id],
    ));
    assert_eq!(got["waived"], false);
    let again = run(&mut app, "unwaive_drc_violation", &[&document(&path), &id]);
    assert!(
        matches!(again, CommandOutcome::Disabled { .. }),
        "{again:?}"
    );
}

/// A board edit after the run marks the results stale.
#[test]
fn a_board_edit_after_the_run_marks_the_results_stale() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir);
    run_drc(&mut app);
    assert!(!app.pcb_drc_panel().unwrap().stale);
    let command = signex_engine::pcb::BoardCommand::DeleteItems {
        items: vec![signex_engine::pcb::BoardItemId::Segment(AUX_TRACK)],
    };
    app.execute_board_commands(&path, vec![command], None)
        .expect("the track goes");
    assert!(app.pcb_drc_panel().unwrap().stale);
    assert!(panel_texts(&app).iter().any(|t| t.contains("run it again")));
}

/// Edit ▸ Undo and Redo of a waiver keep the results current, as the
/// waiver step itself does; any other board step still makes them stale.
#[test]
fn undoing_a_waiver_keeps_the_results_current() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir);
    run_drc(&mut app);
    let clearance = key_of(&app, "builtin::clearance");
    panel(&mut app, DrcPanelMsg::WaiveStarted(clearance));
    panel(&mut app, DrcPanelMsg::NoteEdited("on purpose".into()));
    panel(&mut app, DrcPanelMsg::WaiveConfirmed);
    assert!(!app.pcb_drc_panel().unwrap().stale);
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(
        app.document_state
            .pcb_board(&path)
            .unwrap()
            .options
            .drc_waivers
            .is_empty()
    );
    assert!(!app.pcb_drc_panel().unwrap().stale, "undo of a waiver");
    let _ = app.update(Message::Edit(EditMsg::Redo));
    assert!(!app.pcb_drc_panel().unwrap().stale, "redo of a waiver");

    let command = signex_engine::pcb::BoardCommand::DeleteItems {
        items: vec![signex_engine::pcb::BoardItemId::Segment(AUX_TRACK)],
    };
    app.execute_board_commands(&path, vec![command], None)
        .expect("the track goes");
    run_drc(&mut app);
    assert!(!app.pcb_drc_panel().unwrap().stale);
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(app.pcb_drc_panel().unwrap().stale, "undo of a track delete");
}

/// A refused waiver keeps the note the user typed in the panel.
#[test]
fn a_refused_waiver_keeps_its_note() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir);
    run_drc(&mut app);
    panel(
        &mut app,
        DrcPanelMsg::WaiveStarted("not-a-violation".into()),
    );
    panel(&mut app, DrcPanelMsg::NoteEdited("typed with care".into()));
    panel(&mut app, DrcPanelMsg::WaiveConfirmed);
    assert!(
        app.document_state
            .pcb_board(&path)
            .unwrap()
            .options
            .drc_waivers
            .is_empty()
    );
    let draft = app.pcb_drc_panel().unwrap().waive_draft;
    assert_eq!(
        draft.map(|d| d.note).as_deref(),
        Some("typed with care"),
        "the note stays for another try"
    );
}

/// A row that cannot be shown says so, not that a waiver was not saved.
#[test]
fn a_refused_show_is_logged_as_a_show() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = board_app(&dir);
    let _ = crate::diagnostics::init_logging();
    run_drc(&mut app);
    let key = "no-row-2399-show";
    panel(&mut app, DrcPanelMsg::RowPicked(key.into()));
    let said: Vec<String> = crate::diagnostics::recent_entries()
        .into_iter()
        .map(|e| e.message)
        .filter(|m| m.contains(key))
        .collect();
    assert!(
        said.iter().any(|m| m.contains("could not be shown")),
        "{said:?}"
    );
    assert!(
        !said.iter().any(|m| m.contains("waiver was not saved")),
        "{said:?}"
    );
}
