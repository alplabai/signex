//! #2389 — length tuning from the PCB Active Bar's Tune group, the palette
//! and an agent, driven through `Signex::update` and the headless path: the
//! selected tracks' nets reach the longest of them, a pair's shorter net
//! reaches the other's length, each legal by the rule check and one undo
//! step; with nothing to tune the board stays as it was.

use std::path::{Path, PathBuf};

use pcb_geom::{ItemRef, net_routed_length_mm};
use serde_json::json;
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;

/// USB_P has a 20 mm track, USB_N a 15 mm one 10 mm away, GND a 30 mm one.
fn app() -> (Signex, PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let track = |n: u128, y: f64, length: f64, net: u32| {
        json!({
            "uuid": Uuid::from_u128(n), "start": {"x": 0.0, "y": y},
            "end": {"x": length, "y": y}, "width": 0.25, "layer": "TopCopper", "net": net
        })
    };
    let board: PcbBoard = serde_json::from_value(json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 0.5, "trace_width": 0.25, "clearance": 0.2},
        "nets": [
            {"number": 1, "name": "USB_P"},
            {"number": 2, "name": "USB_N"},
            {"number": 3, "name": "GND"}
        ],
        "segments": [track(11, 0.0, 20.0, 1), track(12, 10.0, 15.0, 2), track(13, 30.0, 30.0, 3)]
    }))
    .unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let path = dir.path().join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board);
    (app, path, dir)
}

fn board(app: &Signex, path: &Path) -> PcbBoard {
    app.document_state.pcb_board(path).expect("board").clone()
}

fn select_tracks(app: &mut Signex, ids: &[u128]) {
    let items: Vec<ItemRef> = ids
        .iter()
        .map(|&n| ItemRef::Track(Uuid::from_u128(n)))
        .collect();
    app.with_pcb_selection(|state, board| state.select_items(board, &items))
        .expect("a board in view");
}

fn run(app: &mut Signex, id: &str) {
    let _ = app.update(Message::Menu(MenuMessage::command(id)));
}

fn length(app: &Signex, path: &Path, net: u32) -> f64 {
    net_routed_length_mm(&board(app, path), net).expect("a length")
}

/// The board passes the rule check: nothing closer than the clearance.
fn assert_legal(app: &Signex, path: &Path) {
    let tuned = board(app, path);
    let (rules, _) = app.board_design_rules(path, &tuned).unwrap();
    let report = signex_drc::run(&tuned, &rules);
    let broken = report.count(signex_drc::ViolationKind::Clearance)
        + report.count(signex_drc::ViolationKind::Short);
    assert_eq!(broken, 0, "{:?}", report.violations);
}

#[test]
fn length_tuning_brings_the_selected_nets_to_the_longest_in_one_step() {
    let (mut app, path, _dir) = app();
    let before = serde_json::to_string(&board(&app, &path)).unwrap();
    select_tracks(&mut app, &[11, 12]);
    run(&mut app, "tune_net_length");
    assert!(
        (length(&app, &path, 2) - 20.0).abs() < 1.0e-3,
        "USB_N tuned"
    );
    assert!((length(&app, &path, 1) - 20.0).abs() < 1.0e-9, "USB_P left");
    assert!(
        (length(&app, &path, 3) - 30.0).abs() < 1.0e-9,
        "GND not selected"
    );
    assert_legal(&app, &path);
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(serde_json::to_string(&board(&app, &path)).unwrap(), before);
}

#[test]
fn pair_length_tuning_matches_the_pair_from_either_track() {
    let (mut app, path, _dir) = app();
    select_tracks(&mut app, &[11]);
    run(&mut app, "tune_diff_pair");
    assert!((length(&app, &path, 2) - 20.0).abs() < 1.0e-3, "matched");
    assert!(
        board(&app, &path)
            .segments
            .iter()
            .filter(|s| s.net == 2)
            .count()
            > 1,
        "USB_N became an accordion"
    );
    assert_legal(&app, &path);
}

#[test]
fn tuning_with_nothing_to_tune_leaves_the_board() {
    let (mut app, path, _dir) = app();
    let before = serde_json::to_string(&board(&app, &path)).unwrap();
    run(&mut app, "tune_net_length");
    select_tracks(&mut app, &[13]);
    run(&mut app, "tune_diff_pair");
    run(&mut app, "tune_net_length");
    assert_eq!(
        serde_json::to_string(&board(&app, &path)).unwrap(),
        before,
        "nothing selected, GND in no pair, one net already the longest"
    );
}

fn headless(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).unwrap();
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args).0
}

#[test]
fn an_agent_tunes_a_named_net_and_reads_the_lengths_back() {
    let (mut app, path, _dir) = app();
    let document = format!("document={}", path.display());
    let outcome = headless(
        &mut app,
        "tune_net_length",
        &[&document, "net=USB_P", "value=25"],
    );
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    assert_eq!(answer["nets"][0]["net"], "USB_P");
    let reached = answer["nets"][0]["length_mm"].as_f64().expect("a length");
    assert!((reached - 25.0).abs() < 1.0e-3, "{answer}");
    assert!((length(&app, &path, 1) - 25.0).abs() < 1.0e-3);

    let outcome = headless(&mut app, "tune_diff_pair", &[&document, "net=USB_N"]);
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    assert_eq!(answer["nets"][0]["net"], "USB_N");
    assert!((length(&app, &path, 2) - 25.0).abs() < 1.0e-3, "{answer}");

    let refused = headless(&mut app, "tune_net_length", &[&document, "net=NOPE"]);
    assert!(
        matches!(&refused, CommandOutcome::Disabled { reason } if reason.contains("NOPE")),
        "{refused:?}"
    );
}
