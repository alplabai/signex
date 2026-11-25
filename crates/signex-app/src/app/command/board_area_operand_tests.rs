//! #2470 — the area tools from their operands, as the CLI and the agent
//! channel run them: each adds one zone as one board undo step, answers with
//! it as JSON (a pour with its fill), and is refused with the reason for a
//! layer, outline or net the board cannot take.

use std::path::{Path, PathBuf};

use signex_i18n::RenderTranslatable;
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::keymap::AppCommandId;

/// A two-layer board with nets GND (3) and VCC (4) and a net-3 track along
/// y = 0 on the top layer, in a project that names it, open in its tab.
fn app_with_board(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
    let path = dir.join("board.snxpcb");
    let board: PcbBoard = serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"clearance": 0.2},
        "nets": [{"number": 3, "name": "GND"}, {"number": 4, "name": "VCC"}],
        "segments": [{"uuid": Uuid::from_u128(2), "start": {"x": 0.0, "y": 0.0},
                      "end": {"x": 10.0, "y": 0.0}, "width": 0.5,
                      "layer": "TopCopper", "net": 3}]
    }))
    .unwrap();
    app.open_pcb_tab(path.clone(), "board".into(), board);
    (app, path)
}

fn headless(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).unwrap();
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args).0
}

fn answered(outcome: CommandOutcome) -> serde_json::Value {
    match outcome {
        CommandOutcome::Answered(answer) => answer,
        other => panic!("not answered: {other:?}"),
    }
}

fn zones(app: &Signex, path: &Path) -> usize {
    app.document_state
        .pcb_board(path)
        .expect("board")
        .zones
        .len()
}

/// An 8 x 8 mm square across the net-3 track, from the board origin with Y
/// up, no corner on the track.
const SQUARE: &str = "corners=1,-4;9,-4;9,4;1,4";

#[test]
fn each_area_tool_adds_its_area_in_one_step_and_answers_with_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let document = format!("document={}", path.display());
    let top = "layer=TopCopper";
    let cases: [(&str, Vec<&str>, &str); 5] = [
        (
            "place_board_fill",
            vec![top, "start=1,0", "end=9,-4"],
            "fill",
        ),
        ("place_board_region", vec![top, SQUARE], "region"),
        (
            "place_board_polygon_pour",
            vec![top, SQUARE, "net=VCC"],
            "pour",
        ),
        (
            "place_board_keepout",
            vec!["layer=TopKeepout", SQUARE],
            "keepout",
        ),
        ("place_board_pour_cutout", vec![top, SQUARE], "pour_cutout"),
    ];
    for (count, (id, operands, kind)) in cases.into_iter().enumerate() {
        let mut all = vec![document.as_str()];
        all.extend(operands);
        let answer = answered(headless(&mut app, id, &all));
        assert_eq!(answer["kind"], kind, "{id}: {answer}");
        assert!(Uuid::parse_str(answer["id"].as_str().unwrap()).is_ok());
        assert!(answer["zone"].is_object(), "{id}: {answer}");
        assert_eq!(zones(&app, &path), count + 1, "{id} adds one area");
    }
    let board = app.document_state.pcb_board(&path).unwrap();
    // The fill takes the net of the track under its first corner, the
    // region none (no corner on copper), the pour the net named, a keepout
    // and a cutout none.
    let nets: Vec<u32> = board.zones.iter().map(|zone| zone.net).collect();
    assert_eq!(nets, [3, 0, 4, 0, 0]);
    assert_eq!(board.zones[3].layer, "TopKeepout");
    assert!(board.zones[2].is_pour());
    assert!(board.zones[4].is_pour_cutout());
    // From the board origin with Y up: y = 4 is y = -4 in the model.
    assert_eq!(board.zones[1].outline[0], Point::new(1.0, 4.0));
    for _ in 0..5 {
        let _ = app.update(Message::Edit(EditMsg::Undo));
    }
    assert_eq!(zones(&app, &path), 0, "one undo step per area");
}

#[test]
fn a_pour_answers_with_its_fill() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let document = format!("document={}", path.display());
    let answer = answered(headless(
        &mut app,
        "place_board_polygon_pour",
        &[&document, "layer=TopCopper", SQUARE, "net=VCC"],
    ));
    assert_eq!(
        (answer["net"].as_u64(), answer["net_name"].as_str()),
        (Some(4), Some("VCC"))
    );
    let fill = &answer["fill"];
    assert_eq!(fill["zone"], answer["id"], "{answer}");
    assert_eq!(fill["filled"], true, "{answer}");
    assert_eq!(fill["pieces"], 2, "cut in two by the GND track: {answer}");
    // An empty `net` places it with no net.
    let netless = answered(headless(
        &mut app,
        "place_board_polygon_pour",
        &[&document, "layer=BottomCopper", SQUARE, "net="],
    ));
    assert_eq!(netless["net"], 0, "{netless}");
}

#[test]
fn an_area_the_board_cannot_take_is_refused_with_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let document = format!("document={}", path.display());
    for (id, operands, why) in [
        (
            "place_board_polygon_pour",
            vec!["layer=TopSilkscreen", SQUARE],
            "copper layer",
        ),
        (
            "place_board_keepout",
            vec!["layer=TopSilkscreen", SQUARE],
            "keep-out layer",
        ),
        (
            "place_board_region",
            vec!["layer=Inner2Copper", SQUARE],
            "no such copper layer",
        ),
        (
            "place_board_region",
            vec!["layer=TopCopper", "corners=0,1;2,1;4,1"],
            "no area",
        ),
        (
            "place_board_region",
            vec!["layer=TopCopper", "corners=0,1;2"],
            "`corners` must be",
        ),
        (
            "place_board_fill",
            vec!["layer=TopCopper", "start=1,1", "end=4,4", "net=NOPE"],
            "no net named `NOPE`",
        ),
        (
            "place_board_pour_cutout",
            vec!["layer=Top", SQUARE],
            "not a board layer name",
        ),
    ] {
        let mut all = vec![document.as_str()];
        all.extend(operands);
        let outcome = headless(&mut app, id, &all);
        assert!(
            matches!(&outcome, CommandOutcome::Disabled { reason } if reason.render_en_us().contains(why)),
            "{why}: {outcome:?}"
        );
    }
    assert_eq!(zones(&app, &path), 0, "nothing added");
}

#[test]
fn a_headless_run_without_operands_is_asked_for_the_board() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    for id in [
        "place_board_fill",
        "place_board_region",
        "place_board_polygon_pour",
        "place_board_keepout",
        "place_board_pour_cutout",
    ] {
        let command = AppCommandId::new(id).unwrap();
        let outcome = app
            .run_headless_command_in_test(&command, CommandArgs::none())
            .0;
        assert!(
            matches!(&outcome, CommandOutcome::Disabled { reason } if reason.render_en_us().contains("`document`")),
            "{id}: {outcome:?}"
        );
    }
    assert_eq!(zones(&app, &path), 0);
}
