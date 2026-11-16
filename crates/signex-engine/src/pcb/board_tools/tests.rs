//! #2408, #2409: each board tool builds one undo step that undoes exactly.

use std::collections::BTreeSet;

use serde_json::json;
use signex_types::pcb::{PcbBoard, Point};
use signex_types::zone_keepout::KEEPOUT_FILL;
use uuid::Uuid;

use super::*;
use crate::pcb::BoardEngine;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn gap(_: u32, _: u32) -> i64 {
    200_000
}

/// Fresh uuids counting up from `from`.
fn ids(from: u128) -> impl FnMut() -> Uuid {
    let mut next = from;
    move || {
        next += 1;
        id(next)
    }
}

fn track(n: u128, a: (f64, f64), b: (f64, f64), net: u32) -> serde_json::Value {
    json!({"uuid": id(n), "start": {"x": a.0, "y": a.1}, "end": {"x": b.0, "y": b.1},
           "width": 0.25, "layer": "TopCopper", "net": net})
}

fn part(n: u128, x: f64, y: f64, net: u32) -> serde_json::Value {
    json!({"uuid": id(n), "reference": format!("U{n}"), "position": {"x": x, "y": y},
           "layer": "TopCopper",
           "pads": [{"uuid": id(n + 1), "number": "1", "pad_type": "smd", "shape": "circle",
                     "position": {"x": 0.0, "y": 0.0}, "size": {"x": 1.2, "y": 1.2},
                     "drill": null, "layers": ["TopCopper"],
                     "net": {"number": net, "name": format!("N{net}")}}]})
}

/// Net 1 (GND) region over (-5,-5)..(5,5), a net-1 via at (20, 0) with a
/// track out of it, a bent net-2 route, and two parts.
fn board() -> PcbBoard {
    let rect = |x0: f64, y0: f64, x1: f64, y1: f64| json!([{"x": x0, "y": y0}, {"x": x1, "y": y0}, {"x": x1, "y": y1}, {"x": x0, "y": y1}]);
    serde_json::from_value(json!({
        "uuid": id(1000),
        "setup": {"grid_size": 0.5},
        "nets": [{"number": 1, "name": "GND"}, {"number": 2, "name": "SIG"}],
        "zones": [{"uuid": id(10), "net": 1, "net_name": "GND", "layer": "TopCopper",
                   "outline": rect(-5.0, -5.0, 5.0, 5.0), "fill_type": "solid"},
                  {"uuid": id(11), "net": 0, "layer": "TopCopper",
                   "outline": rect(30.0, 0.0, 32.0, 2.0), "fill_type": "solid"}],
        "vias": [{"uuid": id(20), "position": {"x": 20.0, "y": 0.0}, "diameter": 1.0,
                  "drill": 0.5, "layers": ["TopCopper", "BottomCopper"], "net": 1}],
        "segments": [track(21, (20.0, 0.0), (25.0, 0.0), 1),
                     track(30, (40.0, 0.0), (45.0, 0.0), 2),
                     track(31, (45.0, 0.0), (45.0, 5.0), 2)],
        "footprints": [part(50, 60.0, 0.0, 2), part(60, 70.0, 0.0, 2)]
    }))
    .expect("a board")
}

/// The board as JSON: `PcbBoard` has no `PartialEq`.
fn json(board: &PcbBoard) -> serde_json::Value {
    serde_json::to_value(board).expect("a board serialises")
}

/// Run `edit` as one step, check it changed the board, then undo it and
/// check the board is back. The board after the edit is returned.
fn one_step(edit: ToolEdit) -> PcbBoard {
    let mut engine = BoardEngine::new(board(), 1);
    let before = engine.board().clone();
    engine.execute_batch(edit.commands).expect("applies");
    let after = engine.board().clone();
    assert_ne!(json(&after), json(&before));
    assert_eq!(engine.undo_len(), 1, "one undo step");
    engine.undo().expect("undoes");
    assert_eq!(
        json(engine.board()),
        json(&before),
        "undo restores the board"
    );
    after
}

#[test]
fn stitching_a_zone_adds_its_nets_vias_at_the_board_via_size() {
    let edit = stitch_vias(&board(), StitchArea::Zone(id(10)), None, 2.0, gap, ids(500))
        .expect("stitched");
    let added = edit.report.added.len();
    assert!(added > 0);
    let after = one_step(edit);
    let new: Vec<_> = after.vias.iter().filter(|v| v.uuid != id(20)).collect();
    assert_eq!(new.len(), added);
    assert!(new.iter().all(|v| v.net == 1 && v.diameter == 0.6));
}

#[test]
fn stitching_refuses_an_area_with_no_net_and_a_grid_under_the_via() {
    let board = board();
    let no_net = stitch_vias(&board, StitchArea::Zone(id(11)), None, 2.0, gap, ids(500));
    assert_eq!(no_net, Err(ToolRefusal::NoNet));
    let tight = stitch_vias(&board, StitchArea::Zone(id(10)), None, 0.3, gap, ids(500));
    assert!(matches!(tight, Err(ToolRefusal::PitchTooSmall { .. })));
    let rect = StitchArea::Rect(Point::new(-4.0, -4.0), Point::new(4.0, 4.0));
    let named = stitch_vias(&board, rect, Some(1), 2.0, gap, ids(500)).expect("a rectangle");
    assert!(!named.report.added.is_empty());
}

#[test]
fn teardrops_are_added_once_and_removed_in_one_step() {
    let edit = add_teardrops(&board(), None, gap, ids(500)).expect("added");
    assert_eq!(edit.report.added.len(), 1, "the track out of the via");
    let after = one_step(edit);
    assert_eq!(after.zones.iter().filter(|z| z.is_teardrop()).count(), 1);

    // Adding again replaces the old one: still one.
    let again = add_teardrops(&after, None, gap, ids(600)).expect("again");
    assert_eq!(again.report.removed.len(), 1);
    assert_eq!(again.report.added.len(), 1);

    let mut engine = BoardEngine::new(after, 1);
    let removal = remove_teardrops(engine.board(), None).expect("removed");
    engine.execute_batch(removal.commands).expect("applies");
    assert!(!engine.board().zones.iter().any(|z| z.is_teardrop()));
    assert_eq!(
        remove_teardrops(engine.board(), None),
        Err(ToolRefusal::NoTeardrops)
    );
}

#[test]
fn glossing_a_bent_route_is_one_step() {
    let selected: BTreeSet<Uuid> = [id(30), id(31)].into();
    let edit = gloss_tracks(&board(), &selected, gap).expect("glossed");
    assert_eq!(edit.report.removed, [id(31)]);
    let after = one_step(edit);
    let kept = after.segments.iter().find(|s| s.uuid == id(30)).unwrap();
    assert_eq!(kept.end, Point::new(45.0, 5.0));
    let none: BTreeSet<Uuid> = [id(21)].into();
    assert_eq!(
        gloss_tracks(&board(), &none, gap),
        Err(ToolRefusal::NothingToGloss)
    );
}

/// Four tracks closing a 10 x 8 mm loop on the top copper.
fn loop_board() -> (PcbBoard, BTreeSet<Uuid>) {
    let mut board = board();
    let extra: Vec<signex_types::pcb::Segment> = [
        track(80, (100.0, 0.0), (110.0, 0.0), 0),
        track(81, (110.0, 0.0), (110.0, 8.0), 0),
        track(82, (110.0, 8.0), (100.0, 8.0), 0),
        track(83, (100.0, 8.0), (100.0, 0.0), 0),
    ]
    .into_iter()
    .map(|v| serde_json::from_value(v).unwrap())
    .collect();
    board.segments.extend(extra);
    (board, [id(80), id(81), id(82), id(83)].into())
}

#[test]
fn closed_lines_become_the_board_outline_and_open_ones_are_refused() {
    let (board, lines) = loop_board();
    let edit = outline_from_tracks(&board, &lines).expect("closed");
    let mut engine = BoardEngine::new(board.clone(), 1);
    engine.execute_batch(edit.commands).unwrap();
    assert_eq!(engine.board().outline.len(), 4);
    assert_eq!(engine.board().outline[0], Point::new(100.0, 0.0));
    assert_eq!(
        engine.board().segments.len(),
        board.segments.len(),
        "lines stay"
    );

    let open: BTreeSet<Uuid> = [id(80), id(81), id(82)].into();
    assert!(matches!(
        outline_from_tracks(&board, &open),
        Err(ToolRefusal::Path(_))
    ));
}

#[test]
fn a_selected_area_or_closed_lines_become_a_keepout() {
    let zones: BTreeSet<Uuid> = [id(10)].into();
    let edit = keepout_from_selection(&board(), &zones, &BTreeSet::new(), id(900)).unwrap();
    let after = one_step(edit);
    let zone = after.zones.iter().find(|z| z.uuid == id(10)).unwrap();
    assert_eq!((zone.fill_type.as_str(), zone.net), (KEEPOUT_FILL, 0));
    let again = keepout_from_selection(&after, &zones, &BTreeSet::new(), id(900));
    assert!(matches!(again, Err(ToolRefusal::AlreadyKeepout(_))));

    let (board, lines) = loop_board();
    let edit = keepout_from_selection(&board, &BTreeSet::new(), &lines, id(900)).unwrap();
    let mut engine = BoardEngine::new(board, 1);
    engine.execute_batch(edit.commands).unwrap();
    let keepout = engine
        .board()
        .zones
        .iter()
        .find(|z| z.uuid == id(900))
        .unwrap();
    assert_eq!(keepout.fill_type, KEEPOUT_FILL);
    assert!(
        lines
            .iter()
            .all(|l| !engine.board().segments.iter().any(|s| s.uuid == *l))
    );
}

#[test]
fn arranged_parts_land_inside_the_box_without_overlap() {
    let parts: BTreeSet<Uuid> = [id(50), id(60)].into();
    let (a, b) = (Point::new(0.0, 20.0), Point::new(10.0, 30.0));
    let edit = arrange_in_box(&board(), &parts, a, b).expect("fits");
    let after = one_step(edit);
    let placed: Vec<Point> = after
        .footprints
        .iter()
        .filter(|f| parts.contains(&f.uuid))
        .map(|f| f.position)
        .collect();
    for p in &placed {
        // A 1.2 mm pad on the origin: inside the box by its half size.
        assert!(
            p.x >= 0.6 && p.x <= 9.4 && p.y >= 20.6 && p.y <= 29.4,
            "{p:?}"
        );
    }
    let apart = (placed[0].x - placed[1].x)
        .abs()
        .max((placed[0].y - placed[1].y).abs());
    assert!(apart >= 1.2, "no overlap: {placed:?}");

    let small = arrange_in_box(&board(), &parts, a, Point::new(1.0, 21.0));
    assert_eq!(small, Err(ToolRefusal::DoesNotFit));
}

#[test]
fn lines_that_carry_a_net_are_not_turned_into_a_keepout() {
    let (mut board, lines) = loop_board();
    board
        .segments
        .iter_mut()
        .filter(|s| s.uuid == id(81))
        .for_each(|s| s.net = 2);
    let refused = keepout_from_selection(&board, &BTreeSet::new(), &lines, id(900));
    assert_eq!(refused, Err(ToolRefusal::LinesOnNet(1)));
}

#[test]
fn arranged_origins_follow_an_offset_components_grid() {
    use signex_types::pcb_options::{BoardGrid, MARK_COLORS};
    let mut board = board();
    board.options.grids.push(BoardGrid {
        uuid: id(990),
        name: "parts".into(),
        step_mm: 1.0,
        origin: Point::new(0.25, 0.25),
        color: MARK_COLORS[0],
        components: true,
        non_components: false,
    });
    let parts: BTreeSet<Uuid> = [id(50), id(60)].into();
    let (a, b) = (Point::new(0.0, 20.0), Point::new(10.0, 30.0));
    let edit = arrange_in_box(&board, &parts, a, b).expect("fits");
    let mut engine = BoardEngine::new(board, 1);
    engine.execute_batch(edit.commands).expect("applies");
    for fp in engine.board().footprints.iter() {
        let on_grid = |v: f64| ((v - 0.25) - (v - 0.25).round()).abs() < 1e-9;
        assert!(
            on_grid(fp.position.x) && on_grid(fp.position.y),
            "{:?} off the grid counted from (0.25, 0.25)",
            fp.position
        );
        assert!(fp.position.x > 0.0 && fp.position.y > 20.0);
    }
}
