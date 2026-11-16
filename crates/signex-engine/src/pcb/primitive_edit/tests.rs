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

#[test]
fn a_pad_of_a_footprint_that_locks_its_primitives_is_refused() {
    let mut b = board();
    let pad = [(id(FP_A), id(PAD_A1))];
    b.footprints[0].component.lock_primitives = true;
    assert_eq!(
        pad_edit_command(&b, &pad, &PadEdit::SizeX(2.0)),
        Err(PrimitiveEditRefusal::PrimitivesLocked("U1".into()))
    );
    b.footprints[0].component.lock_primitives = false;
    assert!(
        pad_edit_command(&b, &pad, &PadEdit::SizeX(2.0))
            .unwrap()
            .is_some()
    );
}

/// #2384: the location is the corner at the bottom left of the screen, the
/// greatest model y, so it reads as the lower-left once Y is shown upward.
#[test]
fn the_location_is_the_lower_left_corner_as_shown() {
    let mut b = board();
    b.segments[0].end = Point::new(1.0, 4.0);
    let corner = track_location(b.segments.iter().take(1)).unwrap();
    assert_eq!(corner, Point::new(0.0, 4.0));
    let command = track_edit_command(&b, &[id(SEG_1)], TrackEdit::LocationY(10.0)).unwrap();
    run(&mut b, command);
    assert_eq!((b.segments[0].start.y, b.segments[0].end.y), (6.0, 10.0));
}

/// #2378: a locked track takes only its unlock; a mask edit is one replace
/// whose inverse puts the old setting back.
#[test]
fn a_locked_track_refuses_edits_but_its_unlock() {
    use signex_types::pcb::MaskExpansion;
    let mut b = board();
    let track = [id(SEG_1)];
    let lock = track_edit_command(&b, &track, TrackEdit::Locked(true)).unwrap();
    run(&mut b, lock);
    assert!(b.segments[0].locked);
    assert_eq!(
        track_edit_command(&b, &track, TrackEdit::Width(1.0)),
        Err(PrimitiveEditRefusal::ItemLocked)
    );
    let unlock = track_edit_command(&b, &track, TrackEdit::Locked(false)).unwrap();
    run(&mut b, unlock);
    let mask = MaskExpansion::Manual { mm: 0.05 };
    let command = track_edit_command(&b, &track, TrackEdit::SolderMask(mask))
        .unwrap()
        .expect("a change");
    let applied = command.apply(&mut b).expect("applies");
    assert_eq!(b.segments[0].attributes.solder_mask, mask);
    applied.inverse.apply(&mut b).expect("undo applies");
    assert_eq!(b.segments[0].attributes.solder_mask, MaskExpansion::Rule);
}

#[test]
fn via_tenting_and_masks_are_stored_and_a_nan_mask_is_refused() {
    use signex_types::pcb::MaskExpansion;
    let mut b = board();
    let via = [id(VIA_1)];
    let tent = via_edit_command(&b, &via, ViaEdit::TentedBottom(true)).unwrap();
    run(&mut b, tent);
    let top = MaskExpansion::Manual { mm: 0.1 };
    let mask = via_edit_command(&b, &via, ViaEdit::SolderMaskTop(top)).unwrap();
    run(&mut b, mask);
    let a = b.vias[0].attributes;
    assert!(a.bottom.tented && !a.top.tented);
    assert_eq!(a.top.expansion, top);
    let nan = MaskExpansion::Manual { mm: f64::NAN };
    assert_eq!(
        via_edit_command(&b, &via, ViaEdit::SolderMaskBottom(nan)),
        Err(PrimitiveEditRefusal::NonFinite)
    );
    let lock = via_edit_command(&b, &via, ViaEdit::Locked(true)).unwrap();
    run(&mut b, lock);
    assert_eq!(
        via_edit_command(&b, &via, ViaEdit::X(3.0)),
        Err(PrimitiveEditRefusal::ItemLocked)
    );
}
