//! #2936: a board footprint moved, turned and flipped by id from its
//! operands, one board undo step each, answered with the footprint as it
//! is; each lands where the GUI surface it stands for (a drag, the
//! Component view's Rotation and Layer rows) puts it.

use std::path::{Path, PathBuf};

use pcb_geom::ItemRef;
use signex_engine::pcb::component::LocationField;
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::BoardSide;
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::keymap::AppCommandId;
use crate::panels::{ComponentField, PcbComponentMsg};

const R1: Uuid = Uuid::from_u128(7);
const U2: Uuid = Uuid::from_u128(8);
const C3: Uuid = Uuid::from_u128(9);

/// R1 at (10, 10) with pad 1 at (11, 10) and a track from (20, 10) onto it,
/// C3 at (10, 20), and U2 locked at (30, 10), open in its tab.
fn app_with_board(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
    let path = dir.join("board.snxpcb");
    let part = |id: Uuid, name: &str, x: f64, y: f64, locked: bool| {
        serde_json::json!({
            "uuid": id, "reference": name, "position": {"x": x, "y": y},
            "layer": "TopCopper", "locked": locked,
            "pads": [{
                "uuid": Uuid::from_u128(id.as_u128() * 16), "number": "1", "pad_type": "smd",
                "shape": "rect", "position": {"x": 1.0, "y": 0.0}, "size": {"x": 1.0, "y": 0.5},
                "drill": null, "layers": ["TopCopper"]
            }]
        })
    };
    let board: PcbBoard = serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "footprints": [
            part(R1, "R1", 10.0, 10.0, false),
            part(U2, "U2", 30.0, 10.0, true),
            part(C3, "C3", 10.0, 20.0, false)
        ],
        "segments": [{
            "uuid": Uuid::from_u128(900), "start": {"x": 20.0, "y": 10.0},
            "end": {"x": 11.0, "y": 10.0}, "width": 0.25, "layer": "TopCopper", "net": 0
        }]
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

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => {
            signex_i18n::RenderTranslatable::render_en_us(&reason)
        }
        other => panic!("not refused: {other:?}"),
    }
}

fn board(app: &Signex, path: &Path) -> PcbBoard {
    app.document_state.pcb_board(path).expect("board").clone()
}

fn footprint(board: &PcbBoard, id: Uuid) -> &signex_types::pcb::Footprint {
    board.footprints.iter().find(|f| f.uuid == id).unwrap()
}

/// Each track as its ends, in order: a new corner leg gets a fresh uuid on
/// each run.
fn tracks(board: &PcbBoard) -> Vec<[f64; 4]> {
    let mut ends: Vec<[f64; 4]> = (board.segments.iter())
        .map(|s| [s.start.x, s.start.y, s.end.x, s.end.y])
        .collect();
    ends.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    ends
}

/// The same tracks within a nanometre: a move by id and a drag reach the
/// same point by different sums.
fn same_tracks(a: &PcbBoard, b: &PcbBoard) -> bool {
    let (a, b) = (tracks(a), tracks(b));
    a.len() == b.len()
        && a.iter()
            .zip(&b)
            .all(|(x, y)| x.iter().zip(y).all(|(p, q)| (p - q).abs() < 1e-6))
}

fn select(app: &mut Signex, id: Uuid) {
    let picked = app
        .with_pcb_selection(|sel, board| sel.select_items(board, &[ItemRef::Footprint(id)]))
        .expect("the board is in view");
    assert!(picked.changed);
    app.sync_pcb_board_summary();
}

#[test]
fn a_footprint_moves_by_id_as_a_drag_moves_it_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut dragged, path) = app_with_board(dir.path());
    let (from, to) = (Point::new(11.0, 10.0), Point::new(13.54, 12.54));
    dragged.pcb_footprints_dragged(from, to, 0.1, false);
    dragged.pcb_footprints_dropped(from, to);
    let by_drag = board(&dragged, &path);
    let at = footprint(&by_drag, R1).position;
    assert_ne!(at, Point::new(10.0, 10.0), "the drag moved R1");

    let other = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(other.path());
    let user = by_drag.options.origin.to_user(at);
    let document = format!("document={}", path.display());
    let point = format!("at={},{}", user.x, user.y);
    let id = format!("id={R1}");
    let answer = answered(headless(
        &mut app,
        "set_footprint_position",
        &[&document, &id, &point],
    ));
    assert_eq!(answer["changed"], true);
    assert_eq!(answer["designator"], "R1");
    assert_eq!(answer["side"], "top");
    assert_eq!(answer["position"]["x"], user.x);
    let moved = board(&app, &path);
    assert_eq!(footprint(&moved, R1).position, at);
    assert!(
        same_tracks(&moved, &by_drag),
        "the track follows as in a drag: {:?} vs {:?}",
        tracks(&moved),
        tracks(&by_drag)
    );
    let _ = app.update(Message::Edit(EditMsg::Undo));
    let back = board(&app, &path);
    assert_eq!(
        footprint(&back, R1).position,
        Point::new(10.0, 10.0),
        "one step"
    );
    assert_eq!(back.segments.len(), 1);
}

#[test]
fn a_list_places_every_footprint_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let origin = board(&app, &path).options.origin;
    let (a, b) = (
        origin.to_user(Point::new(12.0, 14.0)),
        origin.to_user(Point::new(16.0, 24.0)),
    );
    let document = format!("document={}", path.display());
    let ids = format!("ids={R1},{C3}");
    let points = format!("points={},{};{},{}", a.x, a.y, b.x, b.y);
    let answer = answered(headless(
        &mut app,
        "set_footprint_position",
        &[&document, &ids, &points],
    ));
    assert_eq!(answer["footprints"].as_array().map(Vec::len), Some(2));
    let placed = board(&app, &path);
    assert_eq!(footprint(&placed, R1).position, Point::new(12.0, 14.0));
    assert_eq!(footprint(&placed, C3).position, Point::new(16.0, 24.0));
    let _ = app.update(Message::Edit(EditMsg::Undo));
    let back = board(&app, &path);
    assert_eq!(footprint(&back, R1).position, Point::new(10.0, 10.0));
    assert_eq!(
        footprint(&back, C3).position,
        Point::new(10.0, 20.0),
        "one step"
    );
    // One point short of the ids is refused, nothing moved.
    let short = format!("points={},{}", a.x, a.y);
    let why = refusal(headless(
        &mut app,
        "set_footprint_position",
        &[&document, &ids, &short],
    ));
    assert!(why.contains("1 points for 2 footprints"), "{why}");
}

#[test]
fn a_rotation_by_id_is_the_rotation_row_of_the_component_view() {
    let dir = tempfile::tempdir().unwrap();
    let (mut gui, path) = app_with_board(dir.path());
    select(&mut gui, R1);
    let field = ComponentField::Location(LocationField::Rotation);
    let _ = gui.handle_pcb_component(&PcbComponentMsg::FieldTyped {
        board: path.clone(),
        footprints: vec![R1],
        field: field.clone(),
        text: "90".into(),
    });
    let _ = gui.handle_pcb_component(&PcbComponentMsg::FieldSubmitted {
        board: path.clone(),
        footprints: vec![R1],
        field,
    });
    let by_row = board(&gui, &path);
    assert_eq!(footprint(&by_row, R1).rotation, 90.0, "the row turned R1");

    let other = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(other.path());
    let document = format!("document={}", path.display());
    let answer = answered(headless(
        &mut app,
        "set_footprint_rotation",
        &[&document, "designator=R1", "rotation=90"],
    ));
    assert_eq!(answer["rotation"], 90.0);
    assert_eq!(footprint(&board(&app, &path), R1), footprint(&by_row, R1));
    // The same rotation again changes nothing and makes no step.
    let again = answered(headless(
        &mut app,
        "set_footprint_rotation",
        &[&document, "designator=R1", "rotation=90"],
    ));
    assert_eq!(again["changed"], false);
}

#[test]
fn a_side_by_id_is_the_layer_row_of_the_component_view() {
    let dir = tempfile::tempdir().unwrap();
    let (mut gui, path) = app_with_board(dir.path());
    select(&mut gui, R1);
    let _ = gui.handle_pcb_component(&PcbComponentMsg::SidePicked {
        board: path.clone(),
        footprints: vec![R1],
        side: BoardSide::Bottom,
    });
    let by_row = board(&gui, &path);

    let other = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(other.path());
    let document = format!("document={}", path.display());
    let id = format!("id={R1}");
    let answer = answered(headless(
        &mut app,
        "set_footprint_side",
        &[&document, &id, "side=bottom"],
    ));
    assert_eq!(answer["side"], "bottom");
    assert_eq!(footprint(&board(&app, &path), R1), footprint(&by_row, R1));
    let _ = app.update(Message::Edit(EditMsg::Undo));
    let back = board(&app, &path);
    assert_eq!(footprint(&back, R1).layer, "TopCopper", "one step");
}

#[test]
fn a_locked_footprint_is_refused_with_the_reason_and_nothing_changes() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = board(&app, &path);
    let revision = app.document_state.board_engine(&path).map(|e| e.revision());
    let document = format!("document={}", path.display());
    let id = format!("id={U2}");
    for (command, value) in [
        ("set_footprint_position", "at=0,0"),
        ("set_footprint_rotation", "rotation=90"),
        ("set_footprint_side", "side=bottom"),
    ] {
        let why = refusal(headless(&mut app, command, &[&document, &id, value]));
        assert!(why.contains("U2 is locked"), "{command}: {why}");
    }
    let after = board(&app, &path);
    assert_eq!(after.footprints, before.footprints);
    assert_eq!(after.segments, before.segments);
    assert_eq!(
        app.document_state.board_engine(&path).map(|e| e.revision()),
        revision,
        "no step"
    );
}

#[test]
fn a_footprint_named_twice_or_not_at_all_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let document = format!("document={}", path.display());
    let id = format!("id={R1}");
    let twice = refusal(headless(
        &mut app,
        "set_footprint_side",
        &[&document, &id, "designator=R1", "side=bottom"],
    ));
    assert!(twice.contains("name the footprints once"), "{twice}");
    let none = refusal(headless(
        &mut app,
        "set_footprint_side",
        &[&document, "side=bottom"],
    ));
    assert!(none.contains("name the footprints once"), "{none}");
    let side = refusal(headless(
        &mut app,
        "set_footprint_side",
        &[&document, &id, "side=left"],
    ));
    assert!(side.contains("`top` or `bottom`"), "{side}");
}
