//! #2869 — the String and Graphics tools armed from the PCB Active Bar while
//! a copper layer is active (as a board opens) place from their clicks on
//! the layer Properties shows, one object and one undo step each, and stay
//! armed.

use std::path::Path;

use signex_types::format::pcb_layer_token;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};

use super::form_tests::form;
use super::tests::{app_with_board, board_of, click, live, undo};
use crate::app::pcb_tools::BoardPlaceTool;
use crate::app::{Message, PcbMsg, PcbViewMsg, Signex};
use crate::panels::pcb_primitive_properties::PcbPrimitiveFacts;

/// What the form of the armed tool says the next object is placed with:
/// its layer, and its text height or line width.
#[derive(Debug, PartialEq)]
struct Shown {
    layer: PcbLayer,
    size: f64,
}

/// Open a board with Top Layer copper active and arm `tool` as the PCB
/// Active Bar does; what its form shows.
fn armed_on_copper(dir: &Path, tool: BoardPlaceTool) -> (Signex, std::path::PathBuf, Shown) {
    let (mut app, path) = app_with_board(dir);
    let top = PcbLayer::Copper(CopperLayer::Top);
    let _ = app.update(Message::Pcb(PcbMsg::View(PcbViewMsg::ActiveLayerPicked(
        top,
    ))));
    let _ = app.update(Message::Pcb(PcbMsg::PlaceToolPicked(tool)));
    let shown = match form(&app) {
        PcbPrimitiveFacts::Text(f) => Shown {
            layer: f.layer.expect("a layer").layer,
            size: f.height.expect("a height"),
        },
        PcbPrimitiveFacts::Graphic(f) => Shown {
            layer: f.layer.expect("a layer").layer,
            size: f.width.expect("a width"),
        },
        other => panic!("{tool:?}: its placement form, not {other:?}"),
    };
    assert_ne!(shown.layer, top, "{tool:?}: the form never shows copper");
    assert_eq!(
        app.pcb_layer_view(&path).active(),
        shown.layer,
        "{tool:?}: the layer the form shows is the active layer"
    );
    (app, path, shown)
}

/// `tool` clicked at `clicks` places exactly one drawing with what its form
/// showed; the tool stays armed; one undo takes it away.
fn places_one_drawing(tool: BoardPlaceTool, clicks: &[(f64, f64)], kind: &str) {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path, shown) = armed_on_copper(dir.path(), tool);
    let before = live(&app, &path);
    for &(x, y) in clicks {
        click(&mut app, x, y);
    }
    let graphics = &board_of(&app, &path).graphics;
    assert_eq!(graphics.len(), 1, "{tool:?}: one drawing from its clicks");
    let drawing = &graphics[0];
    assert_eq!(drawing.graphic_type, kind);
    assert_eq!(drawing.layer, pcb_layer_token(shown.layer));
    assert_eq!(drawing.width, shown.size, "{tool:?}: the form's width");
    let armed = app.interaction_state.pcb_tool.as_ref().map(|s| s.tool);
    assert_eq!(armed, Some(tool), "{tool:?} stays armed");
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "{tool:?}: one undo step");
}

#[test]
fn the_string_tool_places_a_string_on_the_form_layer_with_one_click() {
    let dir = tempfile::tempdir().unwrap();
    let tool = BoardPlaceTool::TextString;
    let (mut app, path, shown) = armed_on_copper(dir.path(), tool);
    let before = live(&app, &path);
    click(&mut app, 4.0, 6.0);
    let texts = &board_of(&app, &path).texts;
    assert_eq!(texts.len(), 1, "one string from one click");
    assert_eq!(texts[0].layer, pcb_layer_token(shown.layer));
    assert_eq!(texts[0].font_size, shown.size, "the form's height");
    let armed = app.interaction_state.pcb_tool.as_ref().map(|s| s.tool);
    assert_eq!(armed, Some(tool), "the tool stays armed");
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "one undo step");
}

#[test]
fn the_line_tool_places_a_line_from_two_clicks() {
    places_one_drawing(BoardPlaceTool::Line, &[(1.0, 2.0), (5.0, 2.0)], "line");
}

#[test]
fn the_graphic_arc_tool_places_an_arc_from_centre_start_and_end() {
    let clicks = [(10.0, 10.0), (15.0, 10.0), (10.0, 15.0)];
    places_one_drawing(BoardPlaceTool::GraphicArc, &clicks, "arc");
}

#[test]
fn the_full_circle_tool_places_a_circle_from_centre_and_radius() {
    let clicks = [(20.0, 20.0), (23.0, 20.0)];
    places_one_drawing(BoardPlaceTool::FullCircle, &clicks, "circle");
}

#[test]
fn the_rectangle_tool_places_a_rectangle_from_two_corners() {
    let clicks = [(30.0, 30.0), (34.0, 33.0)];
    places_one_drawing(BoardPlaceTool::Rectangle, &clicks, "rect");
}

#[test]
fn the_text_frame_tool_places_a_frame_from_two_corners() {
    let clicks = [(40.0, 40.0), (50.0, 45.0)];
    places_one_drawing(BoardPlaceTool::TextFrame, &clicks, "text_frame");
}
