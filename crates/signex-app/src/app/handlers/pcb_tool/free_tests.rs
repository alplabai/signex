//! #2373, #2393 — the Pad, Arc and Dimension tools through the app: armed
//! by their command, each finished object one undo step, and the values
//! set in the Properties form while placing kept as the tool's.

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{PadShape, PadType, Point};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};

use super::form_tests::{enter, form, panel, tab, typed};
use super::tests::{app_with_board, board_of, click, escape, hover, live, run, undo};
use crate::app::pcb_tools::BoardPlaceTool;
use crate::app::{Message, PcbMsg, PcbViewMsg, Signex};
use crate::panels::PcbPrimitiveMsg;
use crate::panels::pcb_primitive_properties::{PcbPrimitiveFacts, PrimitiveField, PrimitiveTarget};

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn make_active(app: &mut Signex, layer: PcbLayer) {
    let _ = app.update(Message::Pcb(PcbMsg::View(PcbViewMsg::ActiveLayerPicked(
        layer,
    ))));
}

/// Caner 2026-11-21: a pad goes on the active layer. Top or Bottom copper
/// gives a surface pad of that side (its mask and paste, no hole); a layer
/// that is not copper (the Multi-Layer case) keeps the through-hole pad.
#[test]
fn a_placed_pad_takes_the_active_layer() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let silk = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
    for (layer, at) in [
        (PcbLayer::Copper(CopperLayer::Top), 4.0),
        (PcbLayer::Copper(CopperLayer::Bottom), 6.0),
        (silk, 8.0),
    ] {
        make_active(&mut app, layer);
        run(&mut app, "place_board_pad");
        hover(&mut app, at, 3.0);
        let ghost = app.board_tool_preview().expect("a preview");
        click(&mut app, at, 3.0);
        let placed = board_of(&app, &path).free.pads.last().expect("a pad");
        assert_eq!(&ghost.free.pads[0].layers, &placed.layers, "the ghost agrees");
        let copper: Vec<_> = placed
            .layers
            .iter()
            .filter_map(|t| parse_pcb_layer(t))
            .filter(|l| l.is_copper())
            .collect();
        if layer.is_copper() {
            assert_eq!(placed.pad_type, PadType::Smd, "{layer:?}");
            assert!(placed.drill.is_none(), "{layer:?}");
            assert_eq!(copper, [layer]);
        } else {
            assert_eq!(placed.pad_type, PadType::Thru);
            assert_eq!(placed.drill.as_ref().map(|d| d.diameter), Some(0.8));
            assert_eq!(copper.len(), 2, "top and bottom copper");
        }
    }
    let [top, bottom, _] = &board_of(&app, &path).free.pads[..] else {
        panic!("three pads");
    };
    assert!(top.layers.iter().any(|t| t == "TopSolderMask"));
    assert!(top.layers.iter().any(|t| t == "TopPaste"));
    assert!(bottom.layers.iter().any(|t| t == "BottomSolderMask"));
    assert!(bottom.layers.iter().any(|t| t == "BottomPaste"));
}

#[test]
fn the_four_tools_are_live_rows() {
    for (tool, command) in [
        (BoardPlaceTool::Pad, "place_board_pad"),
        (BoardPlaceTool::Arc, "place_board_arc"),
        (BoardPlaceTool::TextString, "place_board_string"),
        (BoardPlaceTool::Dimension, "place_board_dimension"),
    ] {
        assert_eq!(tool.command(), Some(command), "{tool:?}");
    }
}

#[test]
fn a_pad_lands_on_the_grid_on_the_net_under_it_one_step_per_click() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    run(&mut app, "place_board_pad");
    hover(&mut app, 4.2, 0.3);
    assert_eq!(live(&app, &path), before, "hovering adds nothing");

    click(&mut app, 4.2, 0.3);
    let pads = &board_of(&app, &path).free.pads;
    assert_eq!(pads.len(), 1);
    assert_eq!(pads[0].position, Point::new(4.0, 0.0), "on the 1 mm grid");
    assert_eq!(pads[0].net.as_ref().map(|n| n.number), Some(3));
    assert_eq!(pads[0].net.as_ref().map(|n| n.name.as_str()), Some("GND"));
    assert_eq!(pads[0].pad_type, PadType::Smd, "Top Layer is active");

    click(&mut app, 7.4, 6.2);
    assert_eq!(
        board_of(&app, &path).free.pads.len(),
        2,
        "the tool stays armed"
    );
    assert_eq!(board_of(&app, &path).free.pads[1].net, None, "bare board");
    undo(&mut app);
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "one undo step per pad");
}

#[test]
fn an_arc_by_centre_takes_three_clicks_and_is_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    run(&mut app, "place_board_arc");
    click(&mut app, 2.0, 5.0);
    click(&mut app, 5.0, 5.0);
    assert!(
        board_of(&app, &path).free.arcs.is_empty(),
        "two clicks place nothing"
    );
    hover(&mut app, 2.0, 2.0);
    let preview = app.board_tool_preview().expect("a preview");
    assert_eq!(preview.free.arcs.len(), 1, "the arc follows the pointer");

    click(&mut app, 2.0, 2.0);
    let arcs = &board_of(&app, &path).free.arcs;
    assert_eq!(arcs.len(), 1);
    let sweep = arcs[0].sweep().expect("an arc");
    assert!(close(sweep.radius, 3.0), "the start sets the radius");
    assert!(close(sweep.center.x, 2.0) && close(sweep.center.y, 5.0));
    assert_eq!(arcs[0].start, Point::new(5.0, 5.0));
    assert!(close(arcs[0].end.x, 2.0) && close(arcs[0].end.y, 2.0));
    assert_eq!(arcs[0].layer, "TopCopper");
    assert_eq!(arcs[0].width, 0.3, "the board's track width");
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "one undo step");
}

#[test]
fn cycle_arc_mode_draws_by_edge_on_the_net_under_the_start() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, "place_board_arc");
    run(&mut app, "cycle_arc_mode");
    click(&mut app, 0.0, 0.0);
    click(&mut app, 4.0, 0.0);
    let arcs = &board_of(&app, &path).free.arcs;
    assert_eq!(arcs.len(), 1, "two clicks make an arc by edge");
    let sweep = arcs[0].sweep().expect("an arc");
    assert!(
        close(sweep.sweep.abs(), std::f64::consts::FRAC_PI_2),
        "a quarter"
    );
    assert_eq!(arcs[0].net, 3, "the start is on the net-3 track");
}

#[test]
fn a_dimension_measures_its_two_points_and_stands_off_at_the_third() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    run(&mut app, "place_board_dimension");
    click(&mut app, 1.0, 3.0);
    click(&mut app, 6.0, 3.0);
    assert!(board_of(&app, &path).free.dimensions.is_empty());
    click(&mut app, 6.0, 5.0);
    let dims = &board_of(&app, &path).free.dimensions;
    assert_eq!(dims.len(), 1);
    assert!(close(dims[0].value_mm(), 5.0));
    assert!(close(dims[0].offset.abs(), 2.0), "{}", dims[0].offset);
    assert_eq!(
        parse_pcb_layer(&dims[0].layer),
        Some(PcbLayer::Technical(TechnicalLayer::Dimensions)),
        "copper takes no dimension, so it goes on the Dimensions layer"
    );
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "one undo step");
}

#[test]
fn arming_pad_shows_its_form_and_tab_sets_the_size_for_every_next_pad() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, "place_board_pad");
    let PcbPrimitiveFacts::Pad(pad) = form(&app) else {
        panic!("arming Pad shows the pad form");
    };
    assert!(pad.common.target.is_placement());

    tab(&mut app);
    assert!(app.is_board_tool_paused());
    typed(&mut app, &path, PrimitiveField::SizeX, "2.0");
    enter(&mut app, &path, PrimitiveField::SizeX);
    assert!(!app.is_board_tool_paused(), "Enter goes on placing");
    panel(
        &mut app,
        PcbPrimitiveMsg::ShapePicked {
            target: PrimitiveTarget::placement(path.clone()),
            shape: PadShape::Rect,
        },
    );
    click(&mut app, 7.4, 6.2);
    click(&mut app, 3.4, 6.2);
    let pads = &board_of(&app, &path).free.pads;
    assert_eq!(pads.len(), 2);
    for pad in pads {
        assert_eq!(pad.size.x, 2.0, "the form's size");
        assert_eq!(pad.shape, PadShape::Rect, "the form's shape");
    }
}

#[test]
fn a_hole_wider_than_the_pad_is_refused_and_esc_keeps_the_previous_values() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    // Off copper the pad is through-hole, so it has a hole to refuse.
    make_active(&mut app, PcbLayer::Technical(TechnicalLayer::SilkscreenTop));
    run(&mut app, "place_board_pad");
    tab(&mut app);
    typed(&mut app, &path, PrimitiveField::HoleSize, "3.0");
    enter(&mut app, &path, PrimitiveField::HoleSize);
    tab(&mut app);
    panel(
        &mut app,
        PcbPrimitiveMsg::ShapePicked {
            target: PrimitiveTarget::placement(path.clone()),
            shape: PadShape::Oval,
        },
    );
    escape(&mut app);
    assert!(!app.is_board_tool_paused(), "Esc ends the pause");
    click(&mut app, 7.4, 6.2);
    let pad = &board_of(&app, &path).free.pads[0];
    assert_eq!(pad.drill.as_ref().map(|d| d.diameter), Some(0.8), "refused");
    assert_eq!(pad.shape, PadShape::Circle, "the pause's shape is undone");
}

#[test]
fn the_arc_and_dimension_forms_set_width_and_precision() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, "place_board_arc");
    let PcbPrimitiveFacts::Arc(arc) = form(&app) else {
        panic!("arming Arc shows the arc form");
    };
    assert!(arc.mode.is_some(), "the form names how the arc is drawn");
    tab(&mut app);
    typed(&mut app, &path, PrimitiveField::Width, "0.6");
    enter(&mut app, &path, PrimitiveField::Width);
    click(&mut app, 2.0, 5.0);
    click(&mut app, 5.0, 5.0);
    click(&mut app, 2.0, 2.0);
    assert_eq!(board_of(&app, &path).free.arcs[0].width, 0.6);

    escape(&mut app);
    run(&mut app, "place_board_dimension");
    let PcbPrimitiveFacts::Dimension(_) = form(&app) else {
        panic!("arming Dimension shows the dimension form");
    };
    tab(&mut app);
    typed(&mut app, &path, PrimitiveField::Precision, "1.5");
    enter(&mut app, &path, PrimitiveField::Precision);
    tab(&mut app);
    typed(&mut app, &path, PrimitiveField::Precision, "3");
    enter(&mut app, &path, PrimitiveField::Precision);
    click(&mut app, 1.0, 3.0);
    click(&mut app, 6.0, 3.0);
    click(&mut app, 6.0, 5.0);
    let dimension = &board_of(&app, &path).free.dimensions[0];
    assert_eq!(dimension.precision, 3, "1.5 was refused, 3 taken");
    assert_eq!(dimension.value_text(), "5.000mm");
}

#[test]
fn the_string_form_sets_the_text_of_the_next_strings() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, "place_board_string");
    let PcbPrimitiveFacts::Text(_) = form(&app) else {
        panic!("arming String shows the string form");
    };
    tab(&mut app);
    // The Layer row moves the active layer to the overlay the string needs.
    panel(
        &mut app,
        PcbPrimitiveMsg::AreaLayerPicked {
            target: PrimitiveTarget::placement(path.clone()),
            layer: PcbLayer::Technical(TechnicalLayer::SilkscreenTop),
        },
    );
    typed(&mut app, &path, PrimitiveField::Text, "REV A");
    enter(&mut app, &path, PrimitiveField::Text);
    click(&mut app, 4.2, 6.3);
    let texts = &board_of(&app, &path).texts;
    assert_eq!(texts.len(), 1);
    assert_eq!(texts[0].text, "REV A");
    assert_eq!(texts[0].layer, "TopSilkscreen");
}
