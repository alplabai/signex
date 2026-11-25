//! #2942 — the layout plan through the headless path the CLI takes and
//! through `Signex::update` (the window's Undo and Redo, a palette call with
//! no operands, the board's DRC run kept current), and its file round trip.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layout_plan::{LayoutPlan, PlacementPriority};
use uuid::Uuid;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::settle;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;

/// A 50 x 40 mm board with R1 on top on net GND.
fn board() -> PcbBoard {
    serde_json::from_value(json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 1.27},
        "outline": [
            {"x": 0.0, "y": 0.0}, {"x": 50.0, "y": 0.0},
            {"x": 50.0, "y": 40.0}, {"x": 0.0, "y": 40.0}
        ],
        "layers": [{"id": 0, "name": "TopCopper", "layer_type": "signal"}],
        "nets": [{"number": 1, "name": "GND"}],
        "footprints": [{
            "uuid": Uuid::from_u128(0x10), "reference": "R1", "value": "10k",
            "footprint_id": "R_0603", "position": {"x": 10.0, "y": 10.0},
            "layer": "TopCopper",
            "pads": [{"uuid": Uuid::from_u128(0x11), "number": "1", "pad_type": "smd",
                      "shape": "rect", "position": {"x": 0.0, "y": 0.0},
                      "size": {"x": 1.0, "y": 1.0}, "drill": null,
                      "layers": ["TopCopper"], "net": {"number": 1, "name": "GND"}}]
        }]
    }))
    .expect("the fixture board")
}

/// An app with the board open in its tab, in a project that names it.
fn app_with(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board());
    (app, path)
}

fn headless(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).unwrap();
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args).0
}

fn answer(outcome: CommandOutcome) -> Value {
    match outcome {
        CommandOutcome::Answered(answer) => answer,
        other => panic!("not answered: {other:?}"),
    }
}

fn refused(outcome: CommandOutcome) -> String {
    use signex_i18n::RenderTranslatable;
    match outcome {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("not refused: {other:?}"),
    }
}

fn plan(app: &Signex, path: &Path) -> LayoutPlan {
    app.document_state
        .pcb_board(path)
        .expect("board")
        .options
        .layout_plan
        .clone()
}

fn doc(path: &Path) -> String {
    format!("document={}", path.display())
}

#[test]
fn a_stage_is_set_as_one_undo_step_and_answered_with_the_plan() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    let document = doc(&path);

    let set = answer(headless(
        &mut app,
        "layout_plan_set",
        &[
            &document,
            "stage=placement",
            "weights=connection:3",
            "keep=R1",
        ],
    ));
    assert_eq!(set["changed"], true);
    assert_eq!(set["stage"], "placement");
    assert_eq!(set["empty"], false);
    let placement = &set["stages"][0];
    assert_eq!(placement["stage"], "placement");
    assert_eq!(placement["set"], true);
    assert_eq!(
        placement["settings"],
        json!({"weights": {"connection": 3.0}, "rooms": [], "keep": ["R1"]})
    );
    assert_eq!(set["stages"][1]["set"], false);
    assert_eq!(set["warnings"], json!([]));
    assert_eq!(
        plan(&app, &path)
            .placement
            .weights
            .get(&PlacementPriority::Connection),
        Some(&3.0)
    );

    // The same values again: no change and no step.
    let again = answer(headless(
        &mut app,
        "layout_plan_set",
        &[
            &document,
            "stage=placement",
            "weights=connection:3",
            "keep=R1",
        ],
    ));
    assert_eq!(again["changed"], false);

    // The window's Undo takes the one step back, Redo brings it again.
    let set_plan = plan(&app, &path);
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(plan(&app, &path).is_empty(), "one undo step");
    let _ = app.update(Message::Edit(EditMsg::Redo));
    assert_eq!(plan(&app, &path), set_plan);

    // `remove=true` clears the stage.
    let cleared = answer(headless(
        &mut app,
        "layout_plan_set",
        &[&document, "stage=placement", "remove=true"],
    ));
    assert_eq!(cleared["empty"], true);
    assert!(plan(&app, &path).is_empty());
}

#[test]
fn a_set_that_does_not_fit_its_stage_is_refused_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    let document = doc(&path);
    let revision = app.document_state.pcb_revisions.get(&path).copied();
    for (operands, said) in [
        (vec!["stage=route", "nets=GND"], "`stage` is one of"),
        (vec!["stage=routing", "weights=edges:1"], "placement stage"),
        (vec!["stage=routing"], "give a setting of the routing stage"),
        (
            vec!["stage=routing", "nets=GND", "remove=true"],
            "`remove` clears the whole stage",
        ),
        (vec!["stage=placement", "weights=heat:1"], "not a priority"),
        (
            vec!["stage=checks", "rules=builtin::nothing"],
            "not a DRC rule",
        ),
        (vec!["stage=pours", "pours=x"], "not a pour uuid"),
    ] {
        let mut all = vec![document.as_str()];
        all.extend(operands.iter().copied());
        let reason = refused(headless(&mut app, "layout_plan_set", &all));
        assert!(reason.contains(said), "{operands:?}: {reason}");
    }
    assert!(plan(&app, &path).is_empty());
    assert_eq!(
        app.document_state.pcb_revisions.get(&path).copied(),
        revision,
        "no step was taken"
    );
}

/// A name the board does not have is kept, and said in `warnings`: a plan
/// may be made before the parts, nets or rooms it names exist.
#[test]
fn what_the_board_lacks_is_a_warning_not_a_refusal() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    let set = answer(headless(
        &mut app,
        "layout_plan_set",
        &[&doc(&path), "stage=routing", "nets=GND,VCC"],
    ));
    assert_eq!(set["changed"], true);
    assert_eq!(set["warnings"], json!(["net `VCC` is not on the board"]));
}

#[test]
fn the_status_judges_each_stage_and_names_the_next() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    let document = doc(&path);
    let status = answer(headless(&mut app, "layout_plan_status", &[&document]));
    let stages = status["stages"].as_array().expect("stages");
    let tokens: Vec<&str> = stages
        .iter()
        .map(|s| s["stage"].as_str().unwrap())
        .collect();
    assert_eq!(tokens, ["placement", "routing", "pours", "checks"]);
    // R1 sits on the board; its one-pad net has nothing to route.
    assert_eq!(stages[0]["state"], "done", "{status}");
    assert_eq!(stages[0]["placed"], 1);
    assert_eq!(stages[2]["state"], "not_started", "no pour: {status}");
    assert_eq!(stages[3]["state"], "not_started", "no DRC run: {status}");
    assert_eq!(status["drc_current"], false);
    // A stage with nothing in scope is never read as done.
    assert_eq!(stages[1]["state"], "not_started", "{status}");
    assert_eq!(stages[1]["reasons"], json!(["no net is in scope"]));
    assert_eq!(status["next"], "routing");
}

/// The plan is not checked by DRC, so setting it keeps the board's last DRC
/// run current, as a waiver does.
#[test]
fn setting_the_plan_keeps_the_drc_run_current() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    let task = app.update(Message::Menu(MenuMessage::command("run_drc")));
    settle(&mut app, task);
    let document = doc(&path);
    let before = answer(headless(&mut app, "layout_plan_status", &[&document]));
    assert_eq!(before["drc_current"], true, "{before}");
    let _ = answer(headless(
        &mut app,
        "layout_plan_set",
        &[&document, "stage=checks", "rules=builtin::clearance"],
    ));
    let after = answer(headless(&mut app, "layout_plan_status", &[&document]));
    assert_eq!(after["drc_current"], true, "{after}");
    assert_eq!(after["stages"][3]["state"], "done", "{after}");
}

/// A palette or key call has no operands: refused with the reason, and the
/// board is not touched.
#[test]
fn a_palette_call_without_operands_is_refused_and_touches_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    let revision = app.document_state.pcb_revisions.get(&path).copied();
    let task = app.update(Message::Menu(MenuMessage::command("layout_plan_set")));
    settle(&mut app, task);
    assert!(plan(&app, &path).is_empty());
    assert_eq!(
        app.document_state.pcb_revisions.get(&path).copied(),
        revision
    );
}

/// The plan set through the command is written with the board and read back
/// by a fresh load.
#[test]
fn a_set_plan_round_trips_through_the_board_file() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    let _ = answer(headless(
        &mut app,
        "layout_plan_set",
        &[
            &doc(&path),
            "stage=routing",
            "nets=class:Power,GND",
            "skip=NC_1",
        ],
    ));
    let board = app.document_state.pcb_board(&path).expect("board").clone();
    let text = signex_types::format::SnxPcb::new(board.clone())
        .write_string()
        .expect("encode");
    assert!(text.starts_with("format = \"snxpcb/1.12\""), "{text}");
    let back = signex_types::format::SnxPcb::parse(&text).expect("parse");
    assert_eq!(back.board.options.layout_plan, board.options.layout_plan);
}
