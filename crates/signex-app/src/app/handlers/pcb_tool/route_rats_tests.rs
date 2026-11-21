//! Caner 2026-11-21: the ratsnest follows the route end while routing.
//! While a route of net A is drawn from a pad, the board leaves out net A's
//! line from that pad and the route draws one from its end to the nearest
//! pad of A it has not reached; net B keeps its line. When the route ends
//! the board's lines are whole again. Driven through `Signex::update`.

use std::path::{Path, PathBuf};

use pcb_geom::{PointNm, RatLine};
use serde_json::json;
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::tests::{click, escape, hover, run};
use crate::app::Signex;
use crate::app::handlers::menu::export::tests::app_workspace;

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

/// Net A's pads at (0, 0) and (10, 0), net B's at (0, 10) and (5, 10).
fn app_with(dir: &Path) -> (Signex, PathBuf) {
    let board: PcbBoard = serde_json::from_value(json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 0.5, "trace_width": 0.25, "clearance": 0.2},
        "nets": [{"number": 1, "name": "A"}, {"number": 2, "name": "B"}],
        "footprints": [
            pad(1, 0.0, 0.0, 1), pad(2, 10.0, 0.0, 1),
            pad(3, 0.0, 10.0, 2), pad(4, 5.0, 10.0, 2)
        ]
    }))
    .unwrap();
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board);
    app.sync_pcb_canvas_from_visible_board();
    (app, path)
}

fn at(x: f64, y: f64) -> PointNm {
    PointNm::from_mm(signex_types::pcb::Point::new(x, y), "test").unwrap()
}

/// The lines the board draws: its own, less the routed net's from the
/// start while a route is drawn.
fn board_lines(app: &Signex, path: &Path) -> Vec<RatLine> {
    let ratsnest = &app.interaction_state.pcb_ratsnest;
    match ratsnest.route_on(path) {
        Some(route) => route.without_start(ratsnest.lines()),
        None => ratsnest.lines().to_vec(),
    }
}

fn route_line(app: &Signex) -> Option<RatLine> {
    let session = app.own_board_tool()?;
    let preview = app.board_tool_preview()?;
    app.route_ratsnest_line(session, &preview)
}

fn net_b(lines: &[RatLine]) -> Vec<RatLine> {
    lines.iter().filter(|l| l.net == 2).copied().collect()
}

#[test]
fn while_routing_the_routed_nets_line_starts_at_the_route_end() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    let before = board_lines(&app, &path);
    let own = RatLine {
        net: 1,
        from: at(0.0, 0.0),
        to: at(10.0, 0.0),
    };
    assert!(before.contains(&own), "{before:?}");

    run(&mut app, "place_board_route");
    click(&mut app, 0.0, 0.0);
    hover(&mut app, 4.0, 2.0);
    let line = route_line(&app).expect("the routed net's line");
    assert_eq!(line.net, 1);
    assert_eq!(line.from, at(4.0, 2.0), "from the route's end");
    assert_eq!(line.to, at(10.0, 0.0), "to the pad not reached yet");
    let ghost = app.board_tool_ghost().expect("the route on the pointer");
    assert_eq!(ghost.ratsnest_lines.len(), 1, "the route draws the line");
    let during = board_lines(&app, &path);
    assert!(during.iter().all(|l| l.net != 1), "no line from the start pad");
    assert_eq!(net_b(&during), net_b(&before), "other nets are unchanged");

    hover(&mut app, 6.0, 3.0);
    assert_eq!(route_line(&app).map(|l| l.from), Some(at(6.0, 3.0)), "it follows");

    escape(&mut app);
    assert!(app.interaction_state.pcb_ratsnest.route_key().is_none());
    assert!(app.interaction_state.pcb_ratsnest.route_on(&path).is_none());
    assert_eq!(board_lines(&app, &path), before, "the route ended: lines as before");
}

/// A Track run that lands leg by leg: after a leg lands the split is read
/// again for the board's new revision; Esc ends it.
#[test]
fn a_track_run_keeps_the_line_at_its_end_and_esc_puts_the_lines_back() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path());
    run(&mut app, "place_board_track");
    click(&mut app, 0.0, 0.0);
    click(&mut app, 3.0, 0.0);
    hover(&mut app, 5.0, 1.0);
    let line = route_line(&app).expect("the routed net's line");
    assert_eq!((line.from, line.to), (at(5.0, 1.0), at(10.0, 0.0)));
    escape(&mut app);
    assert!(app.interaction_state.pcb_ratsnest.route_key().is_none());
    let lines = board_lines(&app, &path);
    let own = RatLine {
        net: 1,
        from: at(0.0, 0.0),
        to: at(10.0, 0.0),
    };
    assert!(lines.contains(&own), "the board's own line is back: {lines:?}");
}
