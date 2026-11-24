//! Caner 2026-11-21: a multi-corner board tool shows its outline from the
//! first placed point. After one click the preview has a line from that
//! point to the pointer; once the corners enclose an area the closing
//! preview is the area, as before. Driven through `Signex::update`.

use signex_types::pcb::{PcbBoard, Point};

use super::tests::{app_with_board, click, hover, run};
use crate::app::Signex;

fn preview(app: &Signex) -> PcbBoard {
    app.board_tool_preview().expect("a preview on the pointer")
}

/// Whether `board` has a line from `a` to `b`.
fn has_line(board: &PcbBoard, a: Point, b: Point) -> bool {
    board.segments.iter().any(|s| s.start == a && s.end == b)
}

#[test]
fn each_area_tool_draws_a_line_from_its_first_corner_to_the_pointer() {
    let first = Point::new(1.0, 5.0);
    let pointer = Point::new(4.0, 7.0);
    let third = Point::new(7.0, 3.0);
    for command in [
        "place_board_polygon_pour",
        "place_board_region",
        "place_board_keepout",
        "place_board_pour_cutout",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _path) = app_with_board(dir.path());
        run(&mut app, command);
        click(&mut app, first.x, first.y);
        hover(&mut app, pointer.x, pointer.y);
        let one = preview(&app);
        assert!(has_line(&one, first, pointer), "{command}: {one:?}");
        assert!(one.zones.is_empty(), "{command}: no area yet");

        click(&mut app, pointer.x, pointer.y);
        hover(&mut app, third.x, third.y);
        let two = preview(&app);
        assert_eq!(two.zones.len(), 1, "{command}: the closing preview");
        assert!(
            two.segments.is_empty(),
            "{command}: the area replaces the lines"
        );
    }
}

#[test]
fn an_arc_by_centre_draws_a_line_from_its_centre_before_its_start() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with_board(dir.path());
    run(&mut app, "place_board_arc");
    click(&mut app, 2.0, 5.0);
    hover(&mut app, 5.0, 5.0);
    let one = preview(&app);
    assert!(has_line(&one, Point::new(2.0, 5.0), Point::new(5.0, 5.0)));
    assert!(one.free.arcs.is_empty());
}
