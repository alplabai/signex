//! #2719 step 1: the Interactive Routing options through `route_options`, in
//! the window and headless. The command reads what the routing keys set
//! (`cycle_route_mode`, Shift+Space) and sets what the router reads: a mode
//! set by the command routes the next leg, a width set mid-route lands on
//! the route's tracks. A bad token or size changes nothing.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use signex_engine::pcb::route::RouteMode;
use signex_types::corner::CornerMode;
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::RouteOptionsEdit;
use super::tests::{board_of, click, escape, run};
use crate::app::Signex;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::keymap::AppCommandId;

/// A footprint holding one round 1 mm top pad at (`x`, 0) on net A.
fn pad(n: u128, x: f64) -> Value {
    json!({
        "uuid": Uuid::from_u128(0x100 + n), "reference": format!("P{n}"), "value": "",
        "footprint_id": "Pad", "layer": "TopCopper", "rotation": 0.0,
        "position": {"x": x, "y": 0.0},
        "pads": [{
            "uuid": Uuid::from_u128(0x200 + n), "number": "1", "pad_type": "smd",
            "shape": "circle", "position": {"x": 0.0, "y": 0.0}, "size": {"x": 1.0, "y": 1.0},
            "drill": null, "layers": ["TopCopper"], "net": {"number": 1, "name": "A"}
        }]
    })
}

/// Net A's pads at (0, 0) and (10, 0), a 0.5 mm grid, 0.25 mm tracks, a
/// 0.6/0.3 via and a 0.2 mm clearance.
fn app(dir: &Path) -> (Signex, PathBuf) {
    let board: PcbBoard = serde_json::from_value(json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 0.5, "trace_width": 0.25, "clearance": 0.2,
                  "via_diameter": 0.6, "via_drill": 0.3},
        "nets": [{"number": 1, "name": "A"}],
        "footprints": [pad(1, 0.0), pad(2, 10.0)]
    }))
    .unwrap();
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board);
    (app, path)
}

/// `route_options` on the board at `path` with `operands`, through the
/// window's operand path or the headless one.
fn options(app: &mut Signex, path: &Path, operands: &[&str], headless: bool) -> CommandOutcome {
    let id = AppCommandId::new("route_options").expect("a catalog id");
    let document = format!("document={}", path.display());
    let mut all = vec![document.as_str()];
    all.extend_from_slice(operands);
    let args = CommandArgs::parse_operands(&id, &all).expect("operands parse");
    if headless {
        app.run_headless_command_in_test(&id, args).0
    } else {
        app.run_operands_settled(&id, &args).0
    }
}

fn answer(outcome: CommandOutcome) -> Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

#[test]
fn the_options_follow_the_board_until_set_in_the_window_and_headless() {
    for headless in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, path) = app(dir.path());
        let got = answer(options(&mut app, &path, &[], headless));
        assert_eq!(got["conflict_mode"], "walkaround", "headless={headless}");
        let modes = json!(["walkaround", "push_and_shove"]);
        assert_eq!(got["conflict_modes"], modes);
        let corners = json!(["90", "45_start", "45_end", "any_angle"]);
        assert_eq!(got["corner_modes"], corners);
        assert_eq!(got["corner_mode"], "90");
        assert_eq!(got["corner_flipped"], false);
        assert_eq!(got["layer"], "TopCopper");
        assert_eq!(got["width_mm"], 0.25);
        assert_eq!(got["width_from"], "net_class_or_board");
        assert_eq!(got["via_diameter_mm"], 0.6);
        assert_eq!(got["via_hole_mm"], 0.3);
        assert_eq!(got["via_from"], "board");
        assert_eq!(got["route"], Value::Null, "no route is drawn");
    }
}

#[test]
fn set_options_are_the_ones_the_router_reads_in_the_window_and_headless() {
    for headless in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, path) = app(dir.path());
        let operands = [
            "conflict_mode=push_and_shove",
            "corner_mode=45_start",
            "width=0.4",
            "via_diameter=0.8",
            "via_hole=0.4",
        ];
        let got = answer(options(&mut app, &path, &operands, headless));
        let mode = &got["conflict_mode"];
        assert_eq!(mode, "push_and_shove", "headless={headless}");
        assert_eq!(got["corner_mode"], "45_start");
        assert_eq!(got["width_mm"], 0.4);
        assert_eq!(got["width_from"], "set");
        assert_eq!(got["via_diameter_mm"], 0.8);
        assert_eq!(got["via_hole_mm"], 0.4);
        assert_eq!(got["via_from"], "set");
        let interaction = &app.interaction_state;
        assert_eq!(interaction.route_mode, RouteMode::Shove);
        assert_eq!(interaction.corner_rule.mode, CornerMode::Angle45Start);
        let defaults = &interaction.pcb_tool_defaults;
        assert_eq!(defaults.track_width_mm, Some(0.4));
        assert_eq!(defaults.via_diameter_mm, Some(0.8));
        assert_eq!(defaults.via_hole_mm, Some(0.4));
    }
}

/// The window's keys and the command set one state: `cycle_route_mode`
/// shows in the answer, and the answer's mode is what the key steps from.
#[test]
fn the_routing_keys_and_the_command_set_one_state() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app(dir.path());
    run(&mut app, "cycle_route_mode");
    let got = answer(options(&mut app, &path, &[], false));
    assert_eq!(got["conflict_mode"], "push_and_shove");
    answer(options(&mut app, &path, &["corner_mode=any_angle"], false));
    run(&mut app, "place_board_route");
    click(&mut app, 0.0, 0.0);
    assert!(app.bend_board_route(true), "Shift+Space bends the route");
    let got = answer(options(&mut app, &path, &[], false));
    assert_eq!(got["corner_mode"], "90", "any angle steps on to 90");
}

#[test]
fn a_bad_mode_or_size_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app(dir.path());
    let bad = ["conflict_mode=shove", "corner_mode=45_end"];
    let refused = options(&mut app, &path, &bad, true);
    let is_refused = matches!(refused, CommandOutcome::Disabled { .. });
    assert!(is_refused, "{refused:?}");
    assert_eq!(app.interaction_state.corner_rule.mode, CornerMode::Ortho90);

    let edit = RouteOptionsEdit {
        conflict_mode: Some(RouteMode::Shove),
        width_mm: Some(f64::NAN),
        ..RouteOptionsEdit::default()
    };
    assert!(app.route_options(&path, edit).is_err());
    assert_eq!(app.interaction_state.route_mode, RouteMode::Walkaround);
    assert_eq!(app.interaction_state.pcb_tool_defaults.track_width_mm, None);
}

/// While a route is drawn the answer names its net and layer, and a width
/// set mid-route is the width its tracks land with.
#[test]
fn a_route_being_drawn_is_named_and_takes_a_width_set_mid_route() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app(dir.path());
    run(&mut app, "place_board_route");
    click(&mut app, 0.0, 0.0);
    let got = answer(options(&mut app, &path, &[], false));
    let route = &got["route"];
    assert_eq!(route["net"], 1, "{got}");
    assert_eq!(route["name"], "A");
    assert_eq!(route["class"], Value::Null);
    assert_eq!(route["layer"], "TopCopper");
    assert_eq!(got["width_from"], "route");

    let got = answer(options(&mut app, &path, &["width=0.3"], false));
    assert_eq!(got["width_mm"], 0.3);
    click(&mut app, 10.0, 0.0);
    let drawn = answer(options(&mut app, &path, &[], false));
    let draft = drawn["route"]["draft_length_mm"].as_f64();
    assert!(draft.is_some_and(|mm| mm >= 10.0 - 1e-6), "{drawn}");
    escape(&mut app);
    let tracks = &board_of(&app, &path).segments;
    assert!(!tracks.is_empty(), "the route landed");
    let widths_ok = tracks.iter().all(|t| (t.width - 0.3).abs() < 1e-9);
    assert!(widths_ok, "{tracks:?}");
}

/// A size is read by the one `operand_mm`: a doubled suffix, another unit,
/// a value past the file's range and one that would be stored as 0 nm are
/// refused, and nothing changes.
#[test]
fn a_size_the_board_cannot_store_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app(dir.path());
    for size in [
        "width=1mmmm",
        "width=0.25°",
        "width=1e13",
        "via_diameter=4e-7",
    ] {
        let refused = options(&mut app, &path, &[size], true);
        let is_refused = matches!(refused, CommandOutcome::Disabled { .. });
        assert!(is_refused, "{size}: {refused:?}");
    }
    assert_eq!(app.interaction_state.pcb_tool_defaults.track_width_mm, None);
    let got = answer(options(&mut app, &path, &["width=0.3mm"], true));
    assert_eq!(got["width_mm"], 0.3);
}
