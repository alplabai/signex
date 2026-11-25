//! #2373: the Properties edits of free arcs, pads, strings and dimensions.

use signex_types::pcb::{BoardText, PcbBoard, Point};
use signex_types::pcb_arc::TrackArc;
use signex_types::pcb_dimension::LinearDimension;
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use uuid::Uuid;

use super::*;
use crate::pcb::draw::new_free_pad;

fn board() -> PcbBoard {
    let mut board: PcbBoard = serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "nets": [{"number": 3, "name": "GND"}]
    }))
    .unwrap();
    let h = std::f64::consts::FRAC_1_SQRT_2;
    board.free.arcs.push(TrackArc {
        uuid: Uuid::from_u128(2),
        start: Point::new(1.0, 0.0),
        mid: Point::new(h, h),
        end: Point::new(0.0, 1.0),
        width: 0.25,
        layer: "TopCopper".into(),
        net: 3,
    });
    board.texts.push(BoardText {
        uuid: Uuid::from_u128(3),
        text: "J1".into(),
        position: Point::new(5.0, 5.0),
        layer: "TopSilkscreen".into(),
        font_size: 1.0,
        rotation: 0.0,
    });
    board.free.dimensions.push(LinearDimension::new(
        Uuid::from_u128(4),
        Point::new(0.0, 10.0),
        Point::new(10.0, 10.0),
        2.0,
    ));
    let pad = new_free_pad(
        &board,
        Uuid::from_u128(5),
        Point::new(20.0, 20.0),
        "1".into(),
    );
    board.free.pads.push(pad);
    board
}

fn run_command(board: &mut PcbBoard, command: Option<crate::pcb::BoardCommand>) {
    command.expect("a change").apply(board).expect("applies");
}

/// Runs the command built from the board before the board is borrowed
/// for the change.
macro_rules! run {
    ($board:expr, $command:expr $(,)?) => {{
        let command = $command;
        run_command($board, command)
    }};
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn an_arc_keeps_its_circle_when_its_radius_or_end_changes() {
    let mut b = board();
    let ids = [Uuid::from_u128(2)];
    let span = ArcSpan::of(&b.free.arcs[0]).unwrap();
    assert!(
        close(span.from_deg, 0.0) && close(span.to_deg, 90.0),
        "{span:?}"
    );
    run!(
        &mut b,
        arc_edit_command(&b, &ids, ArcEdit::Radius(2.0)).unwrap(),
    );
    let span = ArcSpan::of(&b.free.arcs[0]).unwrap();
    assert!(close(span.radius, 2.0) && close(span.to_deg, 90.0));
    run!(
        &mut b,
        arc_edit_command(&b, &ids, ArcEdit::ToAngle(180.0)).unwrap(),
    );
    let span = ArcSpan::of(&b.free.arcs[0]).unwrap();
    assert!(close(span.to_deg, 180.0) && close(span.center.x, 0.0));
    let none = arc_edit_command(&b, &ids, ArcEdit::Radius(0.0));
    assert_eq!(none, Err(PrimitiveEditRefusal::NoArc), "no radius left");
}

#[test]
fn an_arc_moved_off_copper_drops_its_net() {
    let mut b = board();
    let ids = [Uuid::from_u128(2)];
    let overlay = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
    run!(
        &mut b,
        arc_edit_command(&b, &ids, ArcEdit::Layer(overlay)).unwrap(),
    );
    assert_eq!(
        (b.free.arcs[0].layer.as_str(), b.free.arcs[0].net),
        ("TopSilkscreen", 0)
    );
    let same = arc_edit_command(&b, &ids, ArcEdit::Net(3));
    assert_eq!(same, Ok(None), "a drawing has no net to set");
    let top = PcbLayer::Copper(CopperLayer::Top);
    run!(
        &mut b,
        arc_edit_command(&b, &ids, ArcEdit::Layer(top)).unwrap(),
    );
    run!(&mut b, arc_edit_command(&b, &ids, ArcEdit::Net(3)).unwrap());
    assert_eq!(b.free.arcs[0].net, 3);
}

#[test]
fn a_string_takes_its_text_height_and_text_layers_only() {
    let mut b = board();
    let ids = [Uuid::from_u128(3)];
    run!(
        &mut b,
        text_edit_command(&b, &ids, &TextEdit::Text("U7".into())).unwrap(),
    );
    run!(
        &mut b,
        text_edit_command(&b, &ids, &TextEdit::Height(2.5)).unwrap(),
    );
    run!(
        &mut b,
        text_edit_command(&b, &ids, &TextEdit::Rotation(-90.0)).unwrap(),
    );
    let t = &b.texts[0];
    assert_eq!(
        (t.text.as_str(), t.font_size, t.rotation),
        ("U7", 2.5, 270.0)
    );
    // Copper was refused here; a string goes on copper since Caner's
    // 2026-11-24 decision (#2883). A solder mask still takes no text.
    let copper = TextEdit::Layer(PcbLayer::Copper(CopperLayer::Top));
    assert!(matches!(text_edit_command(&b, &ids, &copper), Ok(Some(_))));
    let mask = TextEdit::Layer(PcbLayer::Technical(TechnicalLayer::SolderMaskTop));
    assert_eq!(
        text_edit_command(&b, &ids, &mask),
        Err(PrimitiveEditRefusal::LayerNotAllowed)
    );
    let zero = text_edit_command(&b, &ids, &TextEdit::Height(0.0)).unwrap();
    assert!(
        zero.unwrap().apply(&mut b).is_err(),
        "the engine refuses a zero height"
    );
}

#[test]
fn a_dimension_keeps_two_ends_and_a_sane_precision() {
    let mut b = board();
    let ids = [Uuid::from_u128(4)];
    run!(
        &mut b,
        dimension_edit_command(&b, &ids, DimensionEdit::Precision(3)).unwrap(),
    );
    run!(
        &mut b,
        dimension_edit_command(&b, &ids, DimensionEdit::EndX(20.0)).unwrap(),
    );
    assert_eq!(b.free.dimensions[0].value_text(), "20.000mm");
    let collapse = dimension_edit_command(&b, &ids, DimensionEdit::EndX(0.0));
    assert_eq!(collapse, Err(PrimitiveEditRefusal::NoLength));
    let wild = dimension_edit_command(&b, &ids, DimensionEdit::Precision(9));
    assert_eq!(wild, Err(PrimitiveEditRefusal::OutOfRange));
}

#[test]
fn a_free_pad_moves_and_turns_on_the_board() {
    let mut b = board();
    let ids = [Uuid::from_u128(5)];
    run!(
        &mut b,
        free_pad_edit_command(&b, &ids, &PadEdit::X(25.0)).unwrap(),
    );
    run!(
        &mut b,
        free_pad_edit_command(&b, &ids, &PadEdit::Rotation(30.0)).unwrap(),
    );
    run!(
        &mut b,
        free_pad_edit_command(&b, &ids, &PadEdit::SizeX(2.0)).unwrap(),
    );
    let pad = &b.free.pads[0];
    assert!(close(pad.position.x, 25.0) && close(pad.position.y, 20.0));
    assert!(close(pad.rotation, 30.0));
    assert_eq!(pad.size.x, 2.0);
    let too_big = free_pad_edit_command(&b, &ids, &PadEdit::HoleSize(5.0)).unwrap();
    assert!(
        too_big.unwrap().apply(&mut b).is_err(),
        "a hole wider than the pad"
    );
}
