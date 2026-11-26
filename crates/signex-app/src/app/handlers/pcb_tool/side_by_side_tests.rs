//! #2389 — Differential Pair Routing, Multi-Route and Quick Route through
//! the app: a pair starts on both nets from one click, walks around an
//! other-net pad with both tracks at the rule clearance, and lands as one
//! undo step; a net in no pair starts nothing; Multi-Route runs the
//! selected nets side by side; Quick Route goes straight to the target.

use std::path::{Path, PathBuf};

use pcb_geom::ItemRef;
use serde_json::json;
use signex_drc::ViolationKind;
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::tests::{board_of, click, escape, has_ghost, hover, live, run, undo};
use crate::app::Signex;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::pcb_tools::BoardPlaceTool;

/// A footprint holding one round 1 mm top pad at (`x`, `y`) on `net`.
fn pad(n: u128, x: f64, y: f64, net: (u32, &str)) -> serde_json::Value {
    json!({
        "uuid": Uuid::from_u128(0x100 + n), "reference": format!("P{n}"), "value": "",
        "footprint_id": "Pad", "layer": "TopCopper", "rotation": 0.0,
        "position": {"x": x, "y": y},
        "pads": [{
            "uuid": Uuid::from_u128(0x200 + n), "number": "1", "pad_type": "smd",
            "shape": "circle", "position": {"x": 0.0, "y": 0.0}, "size": {"x": 1.0, "y": 1.0},
            "drill": null, "layers": ["TopCopper"], "net": {"number": net.0, "name": net.1}
        }]
    })
}

fn track(n: u128, a: (f64, f64), b: (f64, f64), net: u32) -> serde_json::Value {
    json!({
        "uuid": Uuid::from_u128(n), "start": {"x": a.0, "y": a.1}, "end": {"x": b.0, "y": b.1},
        "width": 0.25, "layer": "TopCopper", "net": net
    })
}

/// Nets `names[0]` (1) and `names[1]` (2) end in stubs at (1, 0.5) and
/// (1, -0.5); net C (3) has a pad on the way at (6, 0). A 0.5 mm grid,
/// 0.25 mm tracks and a 0.2 mm clearance.
fn app_with(dir: &Path, names: [&str; 2]) -> (Signex, PathBuf) {
    let board: PcbBoard = serde_json::from_value(json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 0.5, "trace_width": 0.25, "clearance": 0.2},
        "nets": [
            {"number": 1, "name": names[0]},
            {"number": 2, "name": names[1]},
            {"number": 3, "name": "C"}
        ],
        "footprints": [pad(1, 6.0, 0.0, (3, "C"))],
        "segments": [
            track(21, (0.0, 0.5), (1.0, 0.5), 1),
            track(22, (0.0, -0.5), (1.0, -0.5), 2)
        ]
    }))
    .unwrap();
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board);
    (app, path)
}

/// No copper of two nets closer than the rules allow.
fn assert_legal(app: &Signex, path: &Path) {
    let board = board_of(app, path);
    let (rules, _) = app.board_design_rules(path, board).expect("usable rules");
    let report = signex_drc::run(board, &rules);
    let broken = report.count(ViolationKind::Clearance) + report.count(ViolationKind::Short);
    assert_eq!(broken, 0, "{:?}", report.violations);
}

fn tracks_of(app: &Signex, path: &Path, net: u32) -> usize {
    let stub = 1;
    board_of(app, path)
        .segments
        .iter()
        .filter(|s| s.net == net)
        .count()
        - stub
}

#[test]
fn a_pair_routes_both_nets_around_an_other_net_pad_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), ["USB_P", "USB_N"]);
    let before = live(&app, &path);
    run(&mut app, "place_board_diff_pair");
    click(&mut app, 1.0, 0.5);
    let session = app.interaction_state.pcb_tool.as_ref().expect("armed");
    assert_eq!(session.tool, BoardPlaceTool::DiffPairRouting);
    assert_eq!(
        session.points,
        vec![Point::new(1.0, 0.0)],
        "the centre line starts between the two stubs"
    );
    hover(&mut app, 11.0, 0.0);
    assert!(has_ghost(&app), "both tracks follow the pointer");
    click(&mut app, 11.0, 0.0);
    assert_eq!(live(&app, &path), before, "nothing lands mid-route");
    escape(&mut app);

    assert!(tracks_of(&app, &path, 1) >= 2, "USB_P routed");
    assert!(tracks_of(&app, &path, 2) >= 2, "USB_N routed");
    let ends: Vec<Point> = board_of(&app, &path)
        .segments
        .iter()
        .filter(|s| s.net == 1)
        .map(|s| s.start)
        .collect();
    assert!(
        ends.contains(&Point::new(1.0, 0.5)),
        "USB_P starts at its stub"
    );
    assert_legal(&app, &path);
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "one undo removes the pair");
}

#[test]
fn a_net_in_no_pair_starts_no_pair_route() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), ["D0", "D1"]);
    let before = live(&app, &path);
    run(&mut app, "place_board_diff_pair");
    click(&mut app, 1.0, 0.5);
    let session = app.interaction_state.pcb_tool.as_ref().expect("armed");
    assert!(session.points.is_empty() && session.router.is_none());
    click(&mut app, 0.5, 3.0);
    assert!(
        app.interaction_state
            .pcb_tool
            .as_ref()
            .is_some_and(|s| s.points.is_empty()),
        "no copper there either"
    );
    assert_eq!(live(&app, &path), before);
}

#[test]
fn multi_route_runs_the_selected_nets_side_by_side() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), ["D0", "D1"]);
    let before = live(&app, &path);
    run(&mut app, "place_board_multi_route");
    click(&mut app, 1.0, 0.0);
    assert!(
        app.interaction_state
            .pcb_tool
            .as_ref()
            .is_some_and(|s| s.points.is_empty()),
        "nothing selected: no bus"
    );
    let stubs = [21, 22].map(|n| ItemRef::Track(Uuid::from_u128(n)));
    app.with_pcb_selection(|state, board| state.select_items(board, &stubs))
        .expect("a board in view");
    click(&mut app, 1.0, 0.0);
    click(&mut app, 4.0, 0.0);
    escape(&mut app);
    assert!(tracks_of(&app, &path, 1) >= 2 && tracks_of(&app, &path, 2) >= 2);
    let d0: Vec<f64> = board_of(&app, &path)
        .segments
        .iter()
        .filter(|s| s.net == 1 && s.end.x > 1.0)
        .map(|s| s.end.y)
        .collect();
    assert!(!d0.is_empty(), "D0 runs to the second click");
    assert!(d0.iter().all(|y| *y > 0.0), "D0 keeps its side: {d0:?}");
    assert_legal(&app, &path);
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "one undo removes the bus");
}

/// With track vertices snapping, a click between two stubs still starts the
/// centre line between them, and its next point is not pulled onto a pad.
#[test]
fn a_bus_centre_line_is_not_yanked_onto_an_object() {
    use signex_engine::pcb::snap::SnapObject;
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with(dir.path(), ["D0", "D1"]);
    app.ui_state
        .pcb_snap
        .objects
        .insert(SnapObject::TrackVertices);
    run(&mut app, "place_board_multi_route");
    let stubs = [21, 22].map(|n| ItemRef::Track(Uuid::from_u128(n)));
    app.with_pcb_selection(|state, board| state.select_items(board, &stubs))
        .expect("a board in view");
    click(&mut app, 1.0, 0.0);
    let session = app.interaction_state.pcb_tool.as_ref().expect("armed");
    assert_eq!(session.points, vec![Point::new(1.0, 0.0)]);
    // 2 mm from net C's pad at (6, 0), inside 8 px at the default 3 px/mm.
    click(&mut app, 4.0, 0.0);
    let session = app.interaction_state.pcb_tool.as_ref().expect("armed");
    assert_eq!(session.points.last(), Some(&Point::new(4.0, 0.0)));
}

/// A coarse view grid does not coarsen the walkaround's search.
#[test]
fn the_route_search_step_does_not_grow_with_a_coarse_grid() {
    use super::router::search_grid_mm;
    let on = |grid: f64| {
        let board: PcbBoard = serde_json::from_value(json!({
            "uuid": Uuid::from_u128(1), "setup": {"grid_size": grid}
        }))
        .unwrap();
        search_grid_mm(&board)
    };
    assert_eq!(on(5.08), signex_types::pcb::PCB_GRID_MM);
    assert_eq!(on(0.1), 0.1);
}

#[test]
fn quick_route_goes_straight_to_the_target_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), ["D0", "D1"]);
    let before = live(&app, &path);
    run(&mut app, "place_board_quick_route");
    click(&mut app, 1.0, 0.5);
    assert_eq!(live(&app, &path), before, "the start adds nothing");
    click(&mut app, 9.0, 0.5);
    let added: Vec<_> = board_of(&app, &path)
        .segments
        .iter()
        .filter(|s| s.uuid != Uuid::from_u128(21) && s.uuid != Uuid::from_u128(22))
        .cloned()
        .collect();
    assert_eq!(added.len(), 1, "one straight track: {added:?}");
    assert_eq!(
        (added[0].start, added[0].end),
        (Point::new(1.0, 0.5), Point::new(9.0, 0.5))
    );
    assert_eq!(added[0].net, 1, "on the net it started on");
    assert!(
        app.interaction_state
            .pcb_tool
            .as_ref()
            .is_some_and(|s| s.tool == BoardPlaceTool::QuickRoute && s.points.is_empty()),
        "the route ended; the tool stays armed"
    );
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "one undo removes it");
}

/// Maintainer 2026-11-18: with an inner layer active, a pair whose copper
/// is on Top starts each net with a via and runs on the inner layer, as
/// one undo step.
#[test]
fn a_pair_from_top_copper_on_an_inner_layer_starts_with_a_via_per_net() {
    use crate::app::{Message, PcbMsg, PcbViewMsg};
    use signex_types::pcb_layer::{CopperLayer, InnerCopper, PcbLayer};

    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), ["USB_P", "USB_N"]);
    let before = live(&app, &path);
    let inner = CopperLayer::Inner(InnerCopper::new(1).expect("inner 1"));
    let picked = PcbViewMsg::ActiveLayerPicked(PcbLayer::Copper(inner));
    let _ = app.update(Message::Pcb(PcbMsg::View(picked)));
    run(&mut app, "place_board_diff_pair");
    click(&mut app, 1.0, 0.5);
    click(&mut app, 11.0, 0.0);
    escape(&mut app);

    let board = board_of(&app, &path);
    let mut nets: Vec<u32> = board.vias.iter().map(|v| v.net).collect();
    nets.sort_unstable();
    assert_eq!(nets, vec![1, 2], "one via per net: {:?}", board.vias);
    // The two stubs come first; the pair's tracks follow them.
    let routed = &board.segments[2..];
    assert!(!routed.is_empty());
    assert!(
        routed.iter().all(|s| s.layer == "Inner1Copper"),
        "{routed:?}"
    );
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "vias and tracks are one step");
}
