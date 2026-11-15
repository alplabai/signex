//! #2389 — Interactive Routing through the app: a route between two pads
//! goes around an other-net pad at the rule clearance, push mode moves an
//! other-net track and keeps it legal, nothing lands until the route ends,
//! and one undo removes the whole route. A board that changed under the
//! route (an undo) is checked again: the route lands when it still fits,
//! and is kept, then discarded, when it does not.

use std::path::{Path, PathBuf};

use serde_json::json;
use signex_engine::pcb::draw::place_via;
use signex_engine::pcb::route::{RouteDraft, RouteMode};
use signex_engine::pcb::{BoardCommand, BoardItem};
use signex_types::pcb::{PcbBoard, Point, Segment};
use uuid::Uuid;

use super::tests::{board_of, click, escape, has_ghost, hover, live, run, undo};
use crate::app::Signex;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::pcb_tools::BoardPlaceTool;

/// A footprint holding one round 1 mm top pad at (`x`, `y`) on `net`.
fn pad(n: u128, x: f64, y: f64, net: u32) -> serde_json::Value {
    let name = if net == 1 { "A" } else { "B" };
    json!({
        "uuid": Uuid::from_u128(0x100 + n), "reference": format!("P{n}"), "value": "",
        "footprint_id": "Pad", "layer": "TopCopper", "rotation": 0.0,
        "position": {"x": x, "y": y},
        "pads": [{
            "uuid": Uuid::from_u128(0x200 + n), "number": "1", "pad_type": "smd",
            "shape": "circle", "position": {"x": 0.0, "y": 0.0}, "size": {"x": 1.0, "y": 1.0},
            "drill": null, "layers": ["TopCopper"], "net": {"number": net, "name": name}
        }]
    })
}

fn track(n: u128, a: (f64, f64), b: (f64, f64), net: u32) -> serde_json::Value {
    json!({
        "uuid": Uuid::from_u128(n), "start": {"x": a.0, "y": a.1}, "end": {"x": b.0, "y": b.1},
        "width": 0.25, "layer": "TopCopper", "net": net
    })
}

/// Net A's pads at (0, 0) and (10, 0), a 0.5 mm grid, 0.25 mm tracks and a
/// 0.2 mm clearance, with `footprints` and `segments` of net B added.
fn app_with(
    dir: &Path,
    footprints: Vec<serde_json::Value>,
    segments: Vec<serde_json::Value>,
) -> (Signex, PathBuf) {
    let mut all = vec![pad(1, 0.0, 0.0, 1), pad(2, 10.0, 0.0, 1)];
    all.extend(footprints);
    let board: PcbBoard = serde_json::from_value(json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 0.5, "trace_width": 0.25, "clearance": 0.2},
        "nets": [{"number": 1, "name": "A"}, {"number": 2, "name": "B"}],
        "footprints": all,
        "segments": segments
    }))
    .unwrap();
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board);
    (app, path)
}

/// Route from (0, 0) to (10, 0) and end the route with Esc; the board does
/// not change until the route ends.
fn route_across(app: &mut Signex, path: &Path) {
    let before = live(app, path);
    run(app, "place_board_route");
    click(app, 0.0, 0.0);
    hover(app, 10.0, 0.0);
    assert!(has_ghost(app), "the next leg follows the pointer");
    click(app, 10.0, 0.0);
    assert_eq!(live(app, path), before, "nothing lands mid-route");
    escape(app);
}

/// The board passes the design rule check with the rules the router used.
fn assert_legal(app: &Signex, path: &Path) {
    let board = board_of(app, path);
    let (rules, _) = app.board_design_rules(path, board).expect("usable rules");
    let report = signex_drc::run(board, &rules);
    assert!(report.is_clean(), "{:?}", report.violations);
}

#[test]
fn a_route_goes_around_an_other_net_pad_and_one_undo_removes_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), vec![pad(3, 5.0, 0.0, 2)], Vec::new());
    let before = live(&app, &path);
    route_across(&mut app, &path);

    let tracks = &board_of(&app, &path).segments;
    assert!(tracks.len() >= 3, "around the pad, not through: {tracks:?}");
    assert!(tracks.iter().all(|t| t.net == 1 && t.width == 0.25));
    assert_eq!(tracks[0].start, Point::new(0.0, 0.0));
    assert_eq!(tracks.last().map(|t| t.end), Some(Point::new(10.0, 0.0)));
    assert_legal(&app, &path);
    assert!(
        app.interaction_state.pcb_tool.is_some(),
        "Esc ends the route; the tool stays armed"
    );

    undo(&mut app);
    assert_eq!(live(&app, &path), before, "one undo removes the route");
}

#[test]
fn push_mode_moves_an_other_net_track_and_keeps_it_legal() {
    let dir = tempfile::tempdir().unwrap();
    let net_b = vec![pad(3, 2.0, 5.0, 2), pad(4, 8.0, 5.0, 2)];
    let run_b = vec![
        track(11, (2.0, 0.3), (8.0, 0.3), 2),
        track(12, (2.0, 0.3), (2.0, 5.0), 2),
        track(13, (8.0, 5.0), (8.0, 0.3), 2),
    ];
    let (mut app, path) = app_with(dir.path(), net_b, run_b);
    let before = live(&app, &path);
    run(&mut app, "cycle_route_mode");
    assert_eq!(app.interaction_state.route_mode, RouteMode::Shove);
    route_across(&mut app, &path);

    let board = board_of(&app, &path);
    let pushed = board
        .segments
        .iter()
        .find(|s| s.uuid == Uuid::from_u128(11))
        .expect("the run is still there");
    assert!(pushed.start.y > 0.45, "pushed clear: {pushed:?}");
    let route: Vec<_> = board.segments.iter().filter(|s| s.net == 1).collect();
    assert_eq!(route.len(), 1, "straight through, the run moved aside");
    assert_legal(&app, &path);

    undo(&mut app);
    assert_eq!(live(&app, &path), before, "route and push are one step");
    run(&mut app, "cycle_route_mode");
    assert_eq!(
        app.interaction_state.route_mode,
        RouteMode::Walkaround,
        "the mode cycles back"
    );
}

#[test]
fn interactive_routing_is_a_live_tool_and_space_bends_its_route() {
    assert_eq!(
        BoardPlaceTool::InteractiveRouting.command(),
        Some("place_board_route")
    );
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with(dir.path(), Vec::new(), Vec::new());
    run(&mut app, "place_board_route");
    assert!(!app.is_routing_board_track(), "no route yet");
    click(&mut app, 0.0, 0.0);
    assert!(
        app.is_routing_board_track(),
        "Space and Shift+Space bend it"
    );
    let rule = app.interaction_state.corner_rule;
    run(&mut app, "rotate_counterclockwise");
    assert_eq!(app.interaction_state.corner_rule, rule.flipped());
}

/// The route drawn with the tool, if any.
fn drawn_draft(app: &Signex) -> Option<RouteDraft> {
    let session = app.interaction_state.pcb_tool.as_ref()?;
    session.router.as_ref().map(|router| router.draft.clone())
}

#[test]
fn a_route_whose_pushed_run_was_undone_does_not_land_and_is_discarded_on_the_next_esc() {
    let dir = tempfile::tempdir().unwrap();
    let net_b = vec![pad(3, 2.0, 5.0, 2), pad(4, 8.0, 5.0, 2)];
    let (mut app, path) = app_with(dir.path(), net_b, Vec::new());
    let before_run = live(&app, &path);
    // Net B's run lands as its own undo step.
    let run_b: Vec<BoardItem> = [
        track(11, (2.0, 0.3), (8.0, 0.3), 2),
        track(12, (2.0, 0.3), (2.0, 5.0), 2),
        track(13, (8.0, 5.0), (8.0, 0.3), 2),
    ]
    .into_iter()
    .map(|value| BoardItem::Segment(serde_json::from_value::<Segment>(value).unwrap()))
    .collect();
    let add = BoardCommand::AddItems { items: run_b };
    app.execute_board_commands(&path, vec![add], None)
        .expect("the run lands");
    let with_run = live(&app, &path);

    run(&mut app, "cycle_route_mode");
    run(&mut app, "place_board_route");
    click(&mut app, 0.0, 0.0);
    click(&mut app, 10.0, 0.0);
    let pushed = drawn_draft(&app).expect("a route is drawn");
    assert!(!pushed.shoved.is_empty(), "the leg pushed the run");
    assert_eq!(live(&app, &path), with_run, "nothing lands mid-route");

    // Undo takes the run away under the route.
    undo(&mut app);
    assert_eq!(live(&app, &path), before_run);
    escape(&mut app);
    assert_eq!(
        live(&app, &path),
        before_run,
        "a route built on the undone run does not land"
    );
    assert_eq!(
        drawn_draft(&app),
        Some(pushed),
        "the route is kept, not lost behind a log line"
    );

    escape(&mut app);
    assert_eq!(live(&app, &path), before_run);
    let session = app
        .interaction_state
        .pcb_tool
        .as_ref()
        .expect("still armed");
    assert!(
        session.router.is_none() && session.points.is_empty(),
        "the second Esc discards it"
    );
}

#[test]
fn a_route_lands_after_an_undo_that_leaves_it_fitting() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), vec![pad(3, 5.0, 0.0, 2)], Vec::new());
    let before = live(&app, &path);
    let far_via = place_via(
        board_of(&app, &path),
        Uuid::from_u128(77),
        Point::new(30.0, 30.0),
    );
    app.execute_board_commands(&path, vec![far_via], None)
        .expect("the via lands");

    run(&mut app, "place_board_route");
    click(&mut app, 0.0, 0.0);
    click(&mut app, 10.0, 0.0);
    // Undo takes the far via away; the route still fits.
    undo(&mut app);
    assert_eq!(live(&app, &path), before);
    escape(&mut app);

    let board = board_of(&app, &path);
    assert!(board.vias.is_empty(), "the undone via stays undone");
    assert!(board.segments.len() >= 3, "the route landed around the pad");
    assert_legal(&app, &path);
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "one undo removes the route");
}
