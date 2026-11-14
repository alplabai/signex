use signex_types::pcb::{PadShape, Point};
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

use super::*;
use crate::pcb::BoardCommand;
use crate::pcb::tests::{FP_A, SEG_1, SEG_2, VIA_1, board, id};

const PAD_A1: &str = "00000000-0000-0000-0000-0000000000a1";

fn run(board: &mut signex_types::pcb::PcbBoard, command: Option<BoardCommand>) {
    command.expect("a change").apply(board).expect("applies");
}

#[test]
fn moving_the_location_moves_every_track_by_one_step() {
    let mut b = board();
    let tracks = [id(SEG_1), id(SEG_2)];
    let corner = track_location(b.segments.iter().take(2)).unwrap();
    assert_eq!(corner, Point::new(0.0, 0.0));
    let command = track_edit_command(&b, &tracks, TrackEdit::LocationX(5.0)).unwrap();
    run(&mut b, command);
    assert_eq!(b.segments[0].start.x, 5.0);
    assert_eq!(b.segments[0].end.x, 6.0);
    assert_eq!(b.segments[1].start.x, 6.0);
    assert_eq!(b.segments[1].end.x, 7.0);
    assert_eq!(b.segments[2].start.x, 2.0, "an unselected track stays");
}

#[test]
fn an_edit_that_changes_nothing_builds_no_command() {
    let b = board();
    let same = track_edit_command(&b, &[id(SEG_1)], TrackEdit::Layer(CopperLayer::Top));
    assert_eq!(same, Ok(None));
    let gone = track_edit_command(&b, &[Uuid::from_u128(0xdead)], TrackEdit::Width(1.0));
    assert_eq!(gone, Err(PrimitiveEditRefusal::Gone));
    let nan = via_edit_command(&b, &[id(VIA_1)], ViaEdit::X(f64::NAN));
    assert_eq!(nan, Err(PrimitiveEditRefusal::NonFinite));
}

#[test]
fn a_pad_moves_and_turns_on_the_board_through_its_footprint() {
    let mut b = board();
    b.footprints[0].rotation = 90.0;
    b.footprints[0].position = Point::new(10.0, 20.0);
    let pad = [(id(FP_A), id(PAD_A1))];
    let command = pad_edit_command(&b, &pad, &PadEdit::X(12.0)).unwrap();
    run(&mut b, command);
    let fp = &b.footprints[0];
    let world = pcb_geom::pad_world_center(fp, &fp.pads[0]).unwrap().to_mm();
    assert!((world.x - 12.0).abs() < 1e-6, "{world:?}");
    let command = pad_edit_command(&b, &pad, &PadEdit::Rotation(135.0)).unwrap();
    run(&mut b, command);
    let fp = &b.footprints[0];
    assert_eq!(fp.pads[0].rotation, 45.0);
    assert_eq!(
        pcb_geom::pad_world_rotation_deg(fp, &fp.pads[0]).unwrap(),
        135.0
    );
}

#[test]
fn a_pad_of_a_locked_footprint_or_without_a_hole_is_refused() {
    let mut b = board();
    let pad = [(id(FP_A), id(PAD_A1))];
    assert_eq!(
        pad_edit_command(&b, &pad, &PadEdit::HoleSize(0.3)),
        Err(PrimitiveEditRefusal::NoHole("1".into()))
    );
    b.footprints[0].locked = true;
    assert_eq!(
        pad_edit_command(&b, &pad, &PadEdit::Shape(PadShape::Circle)),
        Err(PrimitiveEditRefusal::Locked("U1".into()))
    );
}
