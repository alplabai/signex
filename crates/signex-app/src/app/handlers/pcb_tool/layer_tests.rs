//! Board tools draw on the active layer (maintainer 2026-11-18): a route or
//! track run started on a top pad with an inner layer active starts with a
//! via on the pad's net and runs on the inner layer, one undo step; a pour
//! lands on the inner layer; a refusal is said in the status bar. A leg
//! aimed at a pad of another net or of none is refused with the pad named,
//! and the leg on the pointer is drawn and explained instead of vanishing.

use std::path::{Path, PathBuf};

use serde_json::json;
use signex_types::pcb::{PcbBoard, Point, Segment};
use signex_types::pcb_layer::{CopperLayer, InnerCopper, PcbLayer};
use uuid::Uuid;

use super::tests::{board_of, click, escape, has_ghost, hover, live, run, undo};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, PcbMsg, PcbViewMsg, Signex};

const INNER: &str = "Inner1Copper";

/// A footprint `P{n}` holding one round 1 mm top pad at (`x`, `y`) on net
/// `net` (`None`: no net).
fn pad(n: u128, x: f64, y: f64, net: Option<(u32, &str)>) -> serde_json::Value {
    let net = net.map(|(number, name)| json!({"number": number, "name": name}));
    json!({
        "uuid": Uuid::from_u128(0x100 + n), "reference": format!("P{n}"), "value": "",
        "footprint_id": "Pad", "layer": "TopCopper", "rotation": 0.0,
        "position": {"x": x, "y": y},
        "pads": [{
            "uuid": Uuid::from_u128(0x200 + n), "number": "1", "pad_type": "smd",
            "shape": "circle", "position": {"x": 0.0, "y": 0.0}, "size": {"x": 1.0, "y": 1.0},
            "drill": null, "layers": ["TopCopper"], "net": net
        }]
    })
}

/// A 0.5 mm grid, 0.25 mm tracks, a 0.2 mm clearance and nets A to D, with
/// `pads` on it.
fn app_with(dir: &Path, pads: Vec<serde_json::Value>) -> (Signex, PathBuf) {
    let board: PcbBoard = serde_json::from_value(json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 0.5, "trace_width": 0.25, "clearance": 0.2},
        "nets": [
            {"number": 1, "name": "A"}, {"number": 2, "name": "B"},
            {"number": 3, "name": "C"}, {"number": 4, "name": "D"}
        ],
        "footprints": pads
    }))
    .unwrap();
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board);
    (app, path)
}

fn make_active(app: &mut Signex, layer: PcbLayer) {
    let _ = app.update(Message::Pcb(PcbMsg::View(PcbViewMsg::ActiveLayerPicked(
        layer,
    ))));
}

fn inner() -> PcbLayer {
    PcbLayer::Copper(CopperLayer::Inner(InnerCopper::new(1).expect("inner 1")))
}

/// Whether every track of `board` is on the inner layer and net `net`.
fn all_on_inner(board: &PcbBoard, net: u32) -> bool {
    let on = |t: &Segment| t.layer == INNER && t.net == net;
    board.segments.iter().all(on)
}

const NEEDS_COPPER: &str = "make a copper layer active";
const NEEDS_TEXT_LAYER: &str = "make an overlay, mechanical or documentation layer active";
const OTHER_NET: &str = "P5.1 is on net B, the route is on net A";
const JOINED: &str = "P5.1 joined net A";
const POUR_NO_NET: &str = "Polygon Pour has no net; set it in Properties";

/// The status bar's line for the armed tool, isolation marks removed.
fn status(app: &Signex) -> String {
    app.board_tool_status()
        .unwrap_or_default()
        .replace(['\u{2068}', '\u{2069}'], "")
}

#[test]
fn a_route_from_a_top_pad_on_an_inner_layer_starts_with_a_via_on_the_pads_net() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), vec![pad(1, 0.0, 0.0, Some((1, "A")))]);
    let before = live(&app, &path);
    make_active(&mut app, inner());
    run(&mut app, "place_board_route");
    assert!(
        status(&app).starts_with("Interactive Routing:"),
        "armed, the status bar says how to start: {}",
        status(&app)
    );
    click(&mut app, 0.0, 0.0);
    hover(&mut app, 5.0, 3.0);
    assert!(has_ghost(&app), "the leg follows the pointer");
    assert_eq!(
        app.interaction_state.pcb_canvas.ghost_alpha(),
        Some(1.0),
        "the route on the cursor is drawn whole, not dimmed"
    );
    click(&mut app, 5.0, 3.0);
    assert_eq!(live(&app, &path), before, "nothing lands mid-route");
    escape(&mut app);

    let board = board_of(&app, &path);
    assert_eq!(board.vias.len(), 1, "a via down from the pad");
    assert_eq!(board.vias[0].position, Point::new(0.0, 0.0));
    assert_eq!(board.vias[0].net, 1, "on the pad's net");
    assert!(!board.segments.is_empty());
    assert!(all_on_inner(board, 1), "{:?}", board.segments);
    assert_eq!(board.segments[0].start, Point::new(0.0, 0.0));
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "via and tracks are one step");
}

#[test]
fn a_track_run_from_a_top_pad_on_an_inner_layer_starts_with_a_via() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), vec![pad(1, 0.0, 0.0, Some((1, "A")))]);
    let before = live(&app, &path);
    make_active(&mut app, inner());
    run(&mut app, "place_board_track");
    click(&mut app, 0.0, 0.0);
    click(&mut app, 5.0, 3.0);
    let board = board_of(&app, &path);
    assert_eq!(board.vias.len(), 1);
    assert_eq!(
        (board.vias[0].position, board.vias[0].net),
        (Point::new(0.0, 0.0), 1)
    );
    assert!(!board.segments.is_empty());
    assert!(all_on_inner(board, 1), "{:?}", board.segments);
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "via and legs are one step");
}

#[test]
fn a_route_over_bare_board_on_an_inner_layer_starts_there_with_no_via() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), vec![pad(1, 0.0, 0.0, Some((1, "A")))]);
    make_active(&mut app, inner());
    run(&mut app, "place_board_route");
    click(&mut app, 3.0, 3.0);
    click(&mut app, 6.0, 3.0);
    escape(&mut app);
    let board = board_of(&app, &path);
    assert!(board.vias.is_empty());
    assert!(!board.segments.is_empty());
    assert!(all_on_inner(board, 0), "{:?}", board.segments);
}

#[test]
fn a_polygon_pour_lands_on_the_active_inner_layer_closed_near_its_first_corner() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), Vec::new());
    make_active(&mut app, inner());
    app.ui_state.pcb_snap.to_grids = false;
    app.ui_state.pcb_snap.scope = crate::pcb_canvas::SnapScope::Off;
    run(&mut app, "place_board_polygon_pour");
    click(&mut app, 20.0, 20.0);
    click(&mut app, 25.0, 20.0);
    click(&mut app, 25.0, 25.0);
    // Off the first corner by less than half a grid step: it closes.
    click(&mut app, 20.1, 20.1);
    let zones = &board_of(&app, &path).zones;
    assert_eq!(zones.len(), 1, "closed near its first corner");
    assert_eq!(zones[0].layer, INNER);
}

#[test]
fn a_string_on_a_copper_layer_says_why_in_the_status_bar() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), Vec::new());
    let before = live(&app, &path);
    make_active(&mut app, inner());
    run(&mut app, "place_board_string");
    click(&mut app, 4.0, 4.0);
    assert_eq!(live(&app, &path), before);
    let why = status(&app);
    assert!(why.contains(NEEDS_TEXT_LAYER), "{why}");
}

#[test]
fn routing_tools_on_a_non_copper_layer_say_why_in_the_status_bar() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with(dir.path(), Vec::new());
    make_active(
        &mut app,
        PcbLayer::Technical(signex_types::pcb_layer::TechnicalLayer::SilkscreenTop),
    );
    for command in ["place_board_route", "place_board_track", "place_board_fill"] {
        run(&mut app, command);
        let why = status(&app);
        assert!(why.contains(NEEDS_COPPER), "{command}: {why}");
    }
}

/// U1's pads as the maintainer routed them: A's pad top left, the right
/// column `top_right` (the target), then C, D and C going down.
fn u1(top_right: Option<(u32, &'static str)>) -> Vec<serde_json::Value> {
    vec![
        pad(1, 0.0, 0.0, Some((1, "A"))),
        pad(5, 10.0, 0.0, top_right),
        pad(6, 10.0, 2.0, Some((3, "C"))),
        pad(7, 10.0, 4.0, Some((4, "D"))),
        pad(8, 10.0, 6.0, Some((3, "C"))),
    ]
}

/// Route from A's pad out left, down, along the bottom and up the right,
/// then aim at the top-right pad.
fn route_round_u1(app: &mut Signex) {
    run(app, "place_board_route");
    for (x, y) in [(0.0, 0.0), (-3.0, 0.0), (-3.0, 9.0), (12.0, 9.0)] {
        click(app, x, y);
    }
    hover(app, 10.0, 0.0);
}

fn placed(app: &Signex) -> usize {
    app.interaction_state
        .pcb_tool
        .as_ref()
        .map_or(0, |session| session.points.len())
}

#[test]
fn a_route_round_the_part_ends_on_the_top_right_pad_of_its_net() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), u1(Some((1, "A"))));
    route_round_u1(&mut app);
    assert_eq!(placed(&app), 4);
    click(&mut app, 10.0, 0.0);
    assert_eq!(placed(&app), 5, "the last leg reaches the pad");
    escape(&mut app);
    let tracks = &board_of(&app, &path).segments;
    assert_eq!(tracks.last().map(|t| t.end), Some(Point::new(10.0, 0.0)));
    assert!(tracks.iter().all(|t| t.net == 1));
}

#[test]
fn a_leg_aimed_at_a_pad_of_another_net_is_drawn_refused_and_named() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with(dir.path(), u1(Some((2, "B"))));
    route_round_u1(&mut app);
    let why = status(&app);
    assert!(why.contains(OTHER_NET), "{why}");
    let blocked = &app.interaction_state.pcb_canvas.highlight().blocked;
    assert!(
        !blocked.is_empty(),
        "the aimed leg is drawn in the violation colour, not dropped"
    );
    click(&mut app, 10.0, 0.0);
    assert_eq!(placed(&app), 4, "the click adds nothing");
    let why = status(&app);
    assert!(why.contains(OTHER_NET), "{why}");
}

/// Maintainer decision 2026-11-18: a pad of no net joins the route's net,
/// locked, in the route's undo step.
#[test]
fn a_route_ending_on_a_pad_of_no_net_gives_it_the_routes_net() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), u1(None));
    let before = live(&app, &path);
    route_round_u1(&mut app);
    assert!(app.interaction_state.pcb_canvas.highlight().blocked.is_empty());
    click(&mut app, 10.0, 0.0);
    assert_eq!(placed(&app), 5, "the last leg reaches the pad");
    let why = status(&app);
    assert!(why.contains(JOINED), "{why}");
    escape(&mut app);
    let board = board_of(&app, &path);
    let pad = &board.footprints[1].pads[0];
    assert_eq!(pad.net.as_ref().map(|n| n.number), Some(1), "it took net A");
    assert!(pad.net_locked, "a connection made on the board is locked");
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "route and net are one step");
}

/// A leg ending on a top pad while the route runs on an inner layer ends
/// with a via down to it, in the same step.
#[test]
fn a_route_on_an_inner_layer_ends_on_a_top_pad_with_a_via() {
    let dir = tempfile::tempdir().unwrap();
    let pads = vec![
        pad(1, 0.0, 0.0, Some((1, "A"))),
        pad(2, 10.0, 0.0, Some((1, "A"))),
    ];
    let (mut app, path) = app_with(dir.path(), pads);
    let before = live(&app, &path);
    make_active(&mut app, inner());
    run(&mut app, "place_board_route");
    click(&mut app, 0.0, 0.0);
    click(&mut app, 10.0, 0.0);
    escape(&mut app);
    let board = board_of(&app, &path);
    let mut at: Vec<Point> = board.vias.iter().map(|v| v.position).collect();
    at.sort_by(|a, b| a.x.total_cmp(&b.x));
    assert_eq!(at, vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)]);
    assert!(board.vias.iter().all(|v| v.net == 1));
    assert!(all_on_inner(board, 1), "{:?}", board.segments);
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "vias and tracks are one step");
}

#[test]
fn a_track_run_on_an_inner_layer_ends_on_a_top_pad_with_a_via() {
    let dir = tempfile::tempdir().unwrap();
    let pads = vec![
        pad(1, 0.0, 0.0, Some((1, "A"))),
        pad(2, 10.0, 0.0, Some((1, "A"))),
    ];
    let (mut app, path) = app_with(dir.path(), pads);
    make_active(&mut app, inner());
    run(&mut app, "place_board_track");
    click(&mut app, 0.0, 0.0);
    click(&mut app, 10.0, 0.0);
    let board = board_of(&app, &path);
    assert_eq!(board.vias.len(), 2, "one at each pad");
    assert!(all_on_inner(board, 1), "{:?}", board.segments);
}

/// Arming a board tool brings Properties to the front on what the next
/// click places, Via included.
#[test]
fn arming_a_board_tool_brings_properties_to_the_front() {
    use crate::panels::PanelKind;

    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with(dir.path(), Vec::new());
    for command in [
        "place_board_via",
        "place_board_track",
        "place_board_polygon_pour",
        "place_board_string",
    ] {
        app.end_board_tool();
        app.document_state.dock.show_panel(PanelKind::Messages);
        run(&mut app, command);
        let front = app.document_state.dock.is_frontmost(PanelKind::Properties);
        assert!(front, "{command}: Properties is in front");
    }
}

/// A pour placed off copper, with no net set in Properties, says so.
#[test]
fn a_pour_with_no_net_says_so_in_the_status_bar() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), Vec::new());
    run(&mut app, "place_board_polygon_pour");
    for (x, y) in [(20.0, 20.0), (25.0, 20.0), (25.0, 25.0), (20.0, 20.0)] {
        click(&mut app, x, y);
    }
    assert_eq!(board_of(&app, &path).zones.len(), 1);
    let why = status(&app);
    assert!(why.contains(POUR_NO_NET), "{why}");
}

/// A double-click closes a polygon with three corners.
#[test]
fn a_double_click_closes_a_polygon_pour() {
    use crate::pcb_canvas::PcbCanvasMsg;

    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), Vec::new());
    run(&mut app, "place_board_polygon_pour");
    for (x, y) in [(20.0, 20.0), (25.0, 20.0), (25.0, 25.0)] {
        click(&mut app, x, y);
    }
    let double = PcbCanvasMsg::DoubleClicked {
        at: Point::new(25.0, 25.0),
        tolerance_mm: 0.05,
    };
    let _ = app.update(Message::PcbCanvas(double));
    let zones = &board_of(&app, &path).zones;
    assert_eq!(zones.len(), 1, "closed by the double-click");
    assert_eq!(zones[0].outline.len(), 3);
}
