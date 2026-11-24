//! #2869 — the Graphics tools armed from the PCB Active Bar, through
//! `Signex::update`: Line and Arc on copper place a track and a track arc on
//! the net they start on; on an overlay every Graphics tool places one
//! drawing. Full Circle, Rectangle and Text Frame place nothing on copper
//! yet and say why. One undo step each; the tool stays armed.

use std::path::{Path, PathBuf};

use serde_json::json;
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use uuid::Uuid;

use super::form_tests::form;
use super::tests::{board_of, click, live, undo};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::pcb_tools::BoardPlaceTool;
use crate::app::{Message, PcbMsg, PcbViewMsg, Signex};
use crate::panels::pcb_primitive_properties::PcbPrimitiveFacts;

const TOP: PcbLayer = PcbLayer::Copper(CopperLayer::Top);
const OVERLAY: PcbLayer = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);

/// A 0.5 mm grid, 0.25 mm tracks, and footprint P1 with one round 1 mm top
/// pad on net 1 (A) at the origin.
fn app_with_pad(dir: &Path) -> (Signex, PathBuf) {
    let board: PcbBoard = serde_json::from_value(json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 0.5, "trace_width": 0.25, "clearance": 0.2},
        "nets": [{"number": 1, "name": "A"}],
        "footprints": [{
            "uuid": Uuid::from_u128(0x101), "reference": "P1", "value": "",
            "footprint_id": "Pad", "layer": "TopCopper", "rotation": 0.0,
            "position": {"x": 0.0, "y": 0.0},
            "pads": [{
                "uuid": Uuid::from_u128(0x201), "number": "1", "pad_type": "smd",
                "shape": "circle", "position": {"x": 0.0, "y": 0.0},
                "size": {"x": 1.0, "y": 1.0}, "drill": null, "layers": ["TopCopper"],
                "net": {"number": 1, "name": "A"}
            }]
        }]
    }))
    .unwrap();
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board);
    (app, path)
}

/// `layer` active, then `tool` armed as the PCB Active Bar arms it.
fn arm_on(app: &mut Signex, layer: PcbLayer, tool: BoardPlaceTool) {
    let _ = app.update(Message::Pcb(PcbMsg::View(PcbViewMsg::ActiveLayerPicked(
        layer,
    ))));
    let _ = app.update(Message::Pcb(PcbMsg::PlaceToolPicked(tool)));
}

fn clicks(app: &mut Signex, points: &[(f64, f64)]) {
    for &(x, y) in points {
        click(app, x, y);
    }
}

fn armed(app: &Signex) -> Option<BoardPlaceTool> {
    app.interaction_state.pcb_tool.as_ref().map(|s| s.tool)
}

#[test]
fn a_line_on_copper_is_a_track_on_the_net_of_the_pad_it_starts_on() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_pad(dir.path());
    let before = live(&app, &path);
    arm_on(&mut app, TOP, BoardPlaceTool::Line);
    assert!(
        matches!(form(&app), PcbPrimitiveFacts::Track(_)),
        "Properties shows the track the line places"
    );
    clicks(&mut app, &[(0.0, 0.0), (5.0, 0.0), (5.0, 0.0)]);
    let board = board_of(&app, &path);
    assert_eq!(
        board.segments.len(),
        1,
        "one track; a click on its end ends the run"
    );
    let track = &board.segments[0];
    assert_eq!((track.layer.as_str(), track.net), ("TopCopper", 1));
    assert_eq!(track.width, 0.25, "the board's track width");
    assert!(board.graphics.is_empty(), "not a drawing");
    assert_eq!(armed(&app), Some(BoardPlaceTool::Line), "stays armed");
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "one undo step");
}

#[test]
fn an_arc_on_copper_is_a_track_arc_on_the_net_under_its_start() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_pad(dir.path());
    let before = live(&app, &path);
    arm_on(&mut app, TOP, BoardPlaceTool::GraphicArc);
    assert!(
        matches!(form(&app), PcbPrimitiveFacts::Arc(_)),
        "Properties shows the arc it places"
    );
    // Centre (5, 0), start on the pad at the origin, end toward (5, 5).
    clicks(&mut app, &[(5.0, 0.0), (0.0, 0.0), (5.0, 5.0)]);
    let board = board_of(&app, &path);
    assert_eq!(board.free.arcs.len(), 1, "one track arc");
    let arc = &board.free.arcs[0];
    assert_eq!((arc.layer.as_str(), arc.net), ("TopCopper", 1));
    assert!(
        arc.start.x.abs() < 1e-9 && arc.start.y.abs() < 1e-9,
        "{arc:?}"
    );
    assert!(board.graphics.is_empty(), "not a drawing");
    assert_eq!(armed(&app), Some(BoardPlaceTool::GraphicArc), "stays armed");
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "one undo step");
}

/// On the overlay each Graphics tool places one drawing of its kind, with
/// the width its form shows, as one undo step.
/// A tool, its clicks and the drawing kind it should place.
type DrawCase = (BoardPlaceTool, &'static [(f64, f64)], &'static str);

#[test]
fn on_an_overlay_each_graphics_tool_places_one_drawing() {
    let cases: [DrawCase; 5] = [
        (BoardPlaceTool::Line, &[(1.0, 2.0), (5.0, 2.0)], "line"),
        (
            BoardPlaceTool::GraphicArc,
            &[(10.0, 10.0), (15.0, 10.0), (10.0, 15.0)],
            "arc",
        ),
        (
            BoardPlaceTool::FullCircle,
            &[(20.0, 20.0), (23.0, 20.0)],
            "circle",
        ),
        (
            BoardPlaceTool::Rectangle,
            &[(30.0, 30.0), (34.0, 33.0)],
            "rect",
        ),
        (
            BoardPlaceTool::TextFrame,
            &[(40.0, 40.0), (50.0, 45.0)],
            "text_frame",
        ),
    ];
    for (tool, points, kind) in cases {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, path) = app_with_pad(dir.path());
        let before = live(&app, &path);
        arm_on(&mut app, OVERLAY, tool);
        let PcbPrimitiveFacts::Graphic(shown) = form(&app) else {
            panic!("{tool:?}: the drawing form");
        };
        clicks(&mut app, points);
        let board = board_of(&app, &path);
        assert_eq!(board.graphics.len(), 1, "{tool:?}: one drawing");
        let drawing = &board.graphics[0];
        assert_eq!(drawing.graphic_type, kind);
        assert_eq!(drawing.layer, "TopSilkscreen");
        assert_eq!(
            Some(drawing.width),
            shown.width,
            "{tool:?}: the form's width"
        );
        assert!(board.segments.is_empty() && board.free.arcs.is_empty());
        assert_eq!(armed(&app), Some(tool), "{tool:?} stays armed");
        undo(&mut app);
        assert_eq!(live(&app, &path), before, "{tool:?}: one undo step");
    }
}

/// Full Circle, Rectangle and Text Frame on copper place nothing yet: the
/// design rule check and pour fill read no board drawing. The status bar
/// says why and the tool stays armed.
#[test]
fn copper_shapes_place_nothing_yet_and_say_why() {
    let cases: [(BoardPlaceTool, &[(f64, f64)]); 3] = [
        (BoardPlaceTool::FullCircle, &[(20.0, 20.0), (23.0, 20.0)]),
        (BoardPlaceTool::Rectangle, &[(30.0, 30.0), (34.0, 33.0)]),
        (BoardPlaceTool::TextFrame, &[(40.0, 40.0), (50.0, 45.0)]),
    ];
    for (tool, points) in cases {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, path) = app_with_pad(dir.path());
        let before = live(&app, &path);
        arm_on(&mut app, TOP, tool);
        clicks(&mut app, points);
        assert_eq!(live(&app, &path), before, "{tool:?}: nothing on copper");
        let why = app.board_tool_status().expect("a status line");
        assert!(
            why.contains("make a non-copper layer other than a keep-out active"),
            "{tool:?}: {why}"
        );
        assert_eq!(armed(&app), Some(tool));
    }
}

fn pick(app: &mut Signex, layer: PcbLayer) {
    let _ = app.update(Message::Pcb(PcbMsg::View(PcbViewMsg::ActiveLayerPicked(
        layer,
    ))));
}

/// A Line run begun on copper does not go on as a drawing once an overlay
/// is active, nor one begun on the overlay as a track: the flip drops the
/// points, and the next drawing starts at its own first click.
#[test]
fn a_layer_flip_mid_run_starts_a_new_line() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_pad(dir.path());
    // #2879's default object and axis snaps would pull these clicks.
    app.ui_state.pcb_snap.objects.clear();
    app.ui_state.pcb_snap.to_axes = false;
    arm_on(&mut app, TOP, BoardPlaceTool::Line);
    clicks(&mut app, &[(0.0, 0.0)]);
    pick(&mut app, OVERLAY);
    clicks(&mut app, &[(5.0, 5.0), (9.0, 5.0)]);
    let board = board_of(&app, &path);
    assert!(board.segments.is_empty(), "no track from the copper point");
    assert_eq!(board.graphics.len(), 1, "one line");
    let line = &board.graphics[0];
    let p = signex_types::pcb::Point::new;
    assert_eq!(
        (line.start, line.end),
        (Some(p(5.0, 5.0)), Some(p(9.0, 5.0)))
    );

    pick(&mut app, TOP);
    clicks(&mut app, &[(5.0, 10.0), (8.0, 10.0)]);
    let board = board_of(&app, &path);
    assert_eq!(board.graphics.len(), 1, "no drawing from the overlay point");
    assert_eq!(board.segments.len(), 1, "one track");
    let track = &board.segments[0];
    assert_eq!((track.start, track.end), (p(5.0, 10.0), p(8.0, 10.0)));
}
