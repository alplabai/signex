//! #2368 — a designator moved on its own: by a drag on the board and by
//! `set_footprint_text`, each one board undo step that leaves the footprint
//! where it is, answered with the text as JSON.

use std::path::{Path, PathBuf};

use pcb_geom::BoardTextField;
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::keymap::AppCommandId;

const R1: Uuid = Uuid::from_u128(7);

/// R1 at (10, 10) with its designator at the default spot, 2 mm above, and
/// U2 locked at (30, 10), in a project that names the board, open in its tab.
fn app_with_board(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
    let path = dir.join("board.snxpcb");
    let part = |id: Uuid, name: &str, x: f64, locked: bool| {
        serde_json::json!({
            "uuid": id, "reference": name, "position": {"x": x, "y": 10.0},
            "layer": "TopCopper", "locked": locked,
            "properties": [{ "key": "Reference", "value": name, "layer": "TopSilkscreen" }]
        })
    };
    let board: PcbBoard = serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "footprints": [part(R1, "R1", 10.0, false), part(Uuid::from_u128(8), "U2", 30.0, true)]
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

fn designator_at(app: &Signex, path: &Path, name: &str) -> Point {
    let board = app.document_state.pcb_board(path).expect("board");
    let fp = board
        .footprints
        .iter()
        .find(|f| f.reference == name)
        .unwrap();
    pcb_geom::footprint_text(fp, BoardTextField::Designator)
        .unwrap()
        .unwrap()
        .placement
        .anchor
        .to_mm()
}

fn footprint_at(app: &Signex, path: &Path) -> Point {
    let board = app.document_state.pcb_board(path).expect("board");
    board.footprints[0].position
}

/// Within a few nm: a board point goes through the integer-nm frame twice.
fn close(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-5 && (a.y - b.y).abs() < 1e-5
}

#[test]
fn a_drag_on_the_designator_moves_it_alone_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = designator_at(&app, &path, "R1");
    assert!(close(before, Point::new(10.0, 8.0)));
    let to = Point::new(before.x + 3.0, before.y - 1.0);
    app.pcb_footprints_dragged(before, to, 0.1);
    app.pcb_footprints_dropped(before, to);
    assert!(close(designator_at(&app, &path, "R1"), to));
    assert_eq!(footprint_at(&app, &path), Point::new(10.0, 10.0));
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(close(designator_at(&app, &path, "R1"), before), "one step");
}

#[test]
fn a_locked_footprints_designator_stays() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = designator_at(&app, &path, "U2");
    let to = Point::new(before.x + 3.0, before.y);
    app.pcb_footprints_dragged(before, to, 0.1);
    app.pcb_footprints_dropped(before, to);
    assert!(close(designator_at(&app, &path, "U2"), before));
}

#[test]
fn an_agent_places_the_designator_as_a_drag_does() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let document = format!("document={}", path.display());
    let origin = app.document_state.pcb_board(&path).unwrap().options.origin;
    let user = origin.to_user(Point::new(13.0, 7.0));
    let set = |app: &mut Signex, field: &str, value: String| {
        let field = format!("field={field}");
        let value = format!("value={value}");
        headless(
            app,
            "set_footprint_text",
            &[
                &document,
                "designator=R1",
                "text=designator",
                &field,
                &value,
            ],
        )
    };
    let CommandOutcome::Answered(answer) = set(&mut app, "x", user.x.to_string()) else {
        panic!("not answered");
    };
    assert_eq!(answer["changed"], true);
    assert_eq!(answer["text"], "designator");
    let CommandOutcome::Answered(answer) = set(&mut app, "y", user.y.to_string()) else {
        panic!("not answered");
    };
    assert_eq!(answer["layer"], "TopSilkscreen");
    assert!(close(
        designator_at(&app, &path, "R1"),
        Point::new(13.0, 7.0)
    ));
    assert_eq!(footprint_at(&app, &path), Point::new(10.0, 10.0));
    let CommandOutcome::Answered(answer) = set(&mut app, "justify", "left".into()) else {
        panic!("not answered");
    };
    assert_eq!(answer["justify"], "left");
    // The same value again changes nothing.
    let CommandOutcome::Answered(answer) = set(&mut app, "justify", "left".into()) else {
        panic!("not answered");
    };
    assert_eq!(answer["changed"], false);
    // One step each: undoing the justification and Y leaves the X move.
    let _ = app.update(Message::Edit(EditMsg::Undo));
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(close(
        designator_at(&app, &path, "R1"),
        Point::new(13.0, 8.0)
    ));
}

#[test]
fn a_bad_field_or_text_is_refused_with_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let document = format!("document={}", path.display());
    for (operands, says) in [
        (
            ["text=designator", "field=colour", "value=1"],
            "not a text field",
        ),
        (
            ["text=label", "field=x", "value=1"],
            "`designator` or `comment`",
        ),
        (["text=designator", "field=justify", "value=up"], "`left`"),
        (["text=designator", "field=height", "value=0"], "height"),
    ] {
        let mut all = vec![document.as_str(), "designator=R1"];
        all.extend(operands);
        match headless(&mut app, "set_footprint_text", &all) {
            CommandOutcome::Disabled { reason } => {
                assert!(reason.contains(says), "{reason} lacks {says}");
            }
            other => panic!("not refused: {other:?}"),
        }
    }
    let locked = headless(
        &mut app,
        "set_footprint_text",
        &[
            &document,
            "designator=U2",
            "text=designator",
            "field=x",
            "value=1",
        ],
    );
    assert!(matches!(locked, CommandOutcome::Disabled { .. }));
}
