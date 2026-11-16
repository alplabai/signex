//! #2424 — `add_board_graphic` and `set_board_graphic` from their operands:
//! one board undo step each, answered with the drawing as JSON, refused
//! with the reason for a layer, kind or field the board cannot take.

use std::path::{Path, PathBuf};

use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::keymap::AppCommandId;

/// An empty board in a project that names it, open in its tab.
fn app_with_board(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
    let path = dir.join("board.snxpcb");
    let board: PcbBoard =
        serde_json::from_value(serde_json::json!({ "uuid": Uuid::from_u128(1) })).unwrap();
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

fn graphics(app: &Signex, path: &Path) -> usize {
    app.document_state
        .pcb_board(path)
        .expect("board")
        .graphics
        .len()
}

#[test]
fn an_agent_adds_each_kind_of_drawing_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let document = format!("document={}", path.display());
    let layer = "layer=TopSilkscreen";
    let line = answered(headless(
        &mut app,
        "add_board_graphic",
        &[&document, "kind=line", "start=1,1", "end=5,1", layer],
    ));
    assert_eq!(line["kind"], "line");
    assert_eq!(line["layer"], "TopSilkscreen");
    assert!(Uuid::parse_str(line["id"].as_str().unwrap()).is_ok());
    for operands in [
        vec!["kind=arc", "at=10,10", "start=15,10", "end=10,20"],
        vec!["kind=circle", "at=20,20", "end=23,20"],
        vec!["kind=rectangle", "start=30,30", "end=34,33"],
        vec![
            "kind=text_frame",
            "start=40,40",
            "end=50,45",
            "text=REV A\\nB",
        ],
    ] {
        let mut all = vec![document.as_str(), layer];
        all.extend(operands);
        let answer = answered(headless(&mut app, "add_board_graphic", &all));
        assert!(answer["graphic"].is_object(), "{answer}");
    }
    let board = app.document_state.pcb_board(&path).unwrap();
    assert_eq!(board.graphics.len(), 5);
    assert_eq!(board.graphics[4].text, "REV A\nB", "\\n is a line break");
    // Y up from the board origin, as Properties shows it.
    let origin = board.options.origin;
    assert_eq!(
        board.graphics[0].start,
        Some(origin.from_user(Point::new(1.0, 1.0)))
    );
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(graphics(&app, &path), 4, "one undo step per drawing");
}

#[test]
fn an_agent_is_told_why_a_drawing_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let document = format!("document={}", path.display());
    for (operands, why) in [
        (
            vec!["kind=line", "start=1,1", "end=5,1", "layer=TopCopper"],
            "non-copper",
        ),
        (
            vec!["kind=spline", "start=1,1", "end=5,1"],
            "`kind` is one of",
        ),
        (vec!["kind=circle", "at=1,1", "layer=TopFab"], "`end`"),
        (
            vec!["kind=line", "start=1,1", "end=1,1", "layer=TopFab"],
            "make no line",
        ),
    ] {
        let mut all = vec![document.as_str()];
        all.extend(operands);
        let outcome = headless(&mut app, "add_board_graphic", &all);
        assert!(
            matches!(&outcome, CommandOutcome::Disabled { reason } if reason.contains(why)),
            "{why}: {outcome:?}"
        );
    }
    assert_eq!(graphics(&app, &path), 0);
}

#[test]
fn an_agent_sets_one_field_of_a_drawing_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let document = format!("document={}", path.display());
    let circle = answered(headless(
        &mut app,
        "add_board_graphic",
        &[
            &document,
            "kind=circle",
            "at=5,5",
            "end=8,5",
            "layer=TopFab",
        ],
    ));
    let id = format!("id={}", circle["id"].as_str().unwrap());
    let set = |app: &mut Signex, field: &str, value: &str| {
        let field = format!("field={field}");
        let value = format!("value={value}");
        headless(app, "set_board_graphic", &[&document, &id, &field, &value])
    };
    let answer = answered(set(&mut app, "radius", "4"));
    assert_eq!(answer["changed"], true);
    assert_eq!(answer["graphic"]["radius"], 4.0);
    let unchanged = answered(set(&mut app, "radius", "4mm"));
    assert_eq!(unchanged["changed"], false, "nothing to change");
    answered(set(&mut app, "layer", "BottomFab"));
    let board = app.document_state.pcb_board(&path).unwrap();
    assert_eq!(board.graphics[0].layer, "BottomFab");

    for (field, value, why) in [
        ("layer", "TopCopper", "non-copper"),
        ("radius", "0", "above zero"),
        ("colour", "red", "not a drawing field"),
    ] {
        let outcome = set(&mut app, field, value);
        assert!(
            matches!(&outcome, CommandOutcome::Disabled { reason } if reason.contains(why)),
            "{field}: {outcome:?}"
        );
    }
    let _ = app.update(Message::Edit(EditMsg::Undo));
    let board = app.document_state.pcb_board(&path).unwrap();
    assert_eq!(board.graphics[0].layer, "TopFab", "the layer was one step");
}
