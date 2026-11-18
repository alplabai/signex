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
const NO_NET: &str = "P5.1 is on no net, the route is on net A";

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
    let preview = app.board_tool_preview().expect("a preview");
    let at_pad = |t: &Segment| t.end == Point::new(10.0, 0.0);
    assert!(
        preview.segments.iter().any(at_pad),
        "the aimed leg is drawn, not dropped: {:?}",
        preview.segments
    );
    click(&mut app, 10.0, 0.0);
    assert_eq!(placed(&app), 4, "the click adds nothing");
    let why = status(&app);
    assert!(why.contains(OTHER_NET), "{why}");
}

#[test]
fn a_leg_aimed_at_a_pad_of_no_net_is_refused_and_named() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with(dir.path(), u1(None));
    route_round_u1(&mut app);
    click(&mut app, 10.0, 0.0);
    assert_eq!(placed(&app), 4, "the click adds nothing");
    let why = status(&app);
    assert!(why.contains(NO_NET), "{why}");
}
