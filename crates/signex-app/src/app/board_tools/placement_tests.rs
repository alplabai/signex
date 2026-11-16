//! #2409 — Board Shape from Selection, Keepout from Selection and Arrange
//! in Box through the app: from the menu row through `Signex::update`, and
//! from operands as an agent or the CLI runs them. Each is one undo step.

use pcb_geom::{ClickMode, ItemRef, PickKind};
use signex_types::pcb::Point;
use signex_types::zone_keepout::KEEPOUT_FILL;

use super::copper_tests::{
    app_with_board, board_of, drc_findings, id, live, run, run_operands, undo,
};
use crate::app::command::CommandOutcome;
use crate::app::{Message, Signex};
use crate::pcb_canvas::PcbCanvasMsg;

fn click(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::PcbCanvas(PcbCanvasMsg::Clicked {
        at: Point::new(x, y),
        tolerance_mm: 0.05,
        mode: ClickMode::Replace,
    }));
}

fn loop_lines() -> Vec<ItemRef> {
    [80, 81, 82, 83].map(|n| ItemRef::Track(id(n))).to_vec()
}

#[test]
fn closed_lines_become_the_board_outline_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    app.select_pcb_items(&loop_lines());
    run(&mut app, "board_shape_from_selection");
    let board = board_of(&app, &path);
    assert_eq!(
        board.outline,
        [
            Point::new(100.0, 0.0),
            Point::new(110.0, 0.0),
            Point::new(110.0, 8.0),
            Point::new(100.0, 8.0),
        ]
    );
    assert_eq!(board.segments.len(), 8, "the lines stay");
    undo(&mut app);
    assert_eq!(live(&app, &path), before);

    // Three of the four lines leave it open: refused, nothing changes.
    app.select_pcb_items(&loop_lines()[..3]);
    run(&mut app, "board_shape_from_selection");
    assert_eq!(live(&app, &path), before);

    let ids = format!("ids={},{},{},{}", id(80), id(81), id(82), id(83));
    let outcome = run_operands(&mut app, &path, "board_shape_from_selection", &[&ids]);
    assert!(
        matches!(&outcome, CommandOutcome::Answered(v) if v["outline_points"] == 4),
        "{outcome:?}"
    );
}

#[test]
fn a_selected_area_becomes_a_keepout_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    let kind = PickKind::of_zone(&board_of(&app, &path).zones[0]).expect("pickable");
    app.select_pcb_items(&[ItemRef::Zone { zone: id(10), kind }]);
    run(&mut app, "keepout_from_selection");
    let zone = &board_of(&app, &path).zones[0];
    assert_eq!(zone.uuid, id(10), "it keeps its uuid");
    assert_eq!((zone.fill_type.as_str(), zone.net), (KEEPOUT_FILL, 0));
    undo(&mut app);
    assert_eq!(live(&app, &path), before);

    let ids = format!("ids={},{},{},{}", id(80), id(81), id(82), id(83));
    let outcome = run_operands(&mut app, &path, "keepout_from_selection", &[&ids]);
    assert!(
        matches!(&outcome, CommandOutcome::Answered(v) if v["removed"].as_array().map(Vec::len) == Some(4)),
        "{outcome:?}"
    );
    let board = board_of(&app, &path);
    assert_eq!(board.segments.len(), 4, "the lines became the keepout");
    assert!(board.zones.iter().any(|z| z.fill_type == KEEPOUT_FILL));
}

#[test]
fn arrange_in_box_packs_the_selected_parts_between_two_clicks() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    app.select_pcb_items(&[ItemRef::Footprint(id(50)), ItemRef::Footprint(id(60))]);
    run(&mut app, "arrange_in_box");
    assert!(app.interaction_state.pcb_arrange_box.is_some(), "armed");
    click(&mut app, 0.0, 20.0);
    assert_eq!(live(&app, &path), before, "nothing until the second corner");
    click(&mut app, 10.0, 30.0);
    assert!(app.interaction_state.pcb_arrange_box.is_none());
    let placed: Vec<Point> = board_of(&app, &path)
        .footprints
        .iter()
        .map(|f| f.position)
        .collect();
    for p in &placed {
        // A 1.2 mm pad on each origin: inside the box by its half size.
        assert!(
            p.x >= 0.6 && p.x <= 9.4 && p.y >= 20.6 && p.y <= 29.4,
            "{p:?}"
        );
    }
    let apart = (placed[0].x - placed[1].x)
        .abs()
        .max((placed[0].y - placed[1].y).abs());
    assert!(apart >= 1.2, "no overlap: {placed:?}");
    assert_eq!(drc_findings(&app, &path), 0);
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "one undo step");

    // Esc drops an armed box.
    run(&mut app, "arrange_in_box");
    click(&mut app, 0.0, 20.0);
    app.clear_pcb_selection();
    assert!(app.interaction_state.pcb_arrange_box.is_none());

    let ids = format!("ids={},{}", id(50), id(60));
    let outcome = run_operands(
        &mut app,
        &path,
        "arrange_in_box",
        &[&ids, "start=0,20", "end=10,30"],
    );
    assert!(
        matches!(&outcome, CommandOutcome::Answered(v) if v["changed"].as_array().map(Vec::len) == Some(2)),
        "{outcome:?}"
    );
    let small = run_operands(
        &mut app,
        &path,
        "arrange_in_box",
        &[&ids, "start=0,40", "end=1,41"],
    );
    assert!(
        matches!(&small, CommandOutcome::Disabled { reason } if reason.contains("do not fit")),
        "{small:?}"
    );
}
