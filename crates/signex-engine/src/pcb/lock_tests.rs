//! #2407: track and via locks, and the via span.

use signex_types::pcb::ViaType;
use signex_types::pcb_layer::{CopperLayer, InnerCopper};

use crate::pcb::placement::{self, PlacementRefusal};
use crate::pcb::primitive_edit::{
    PrimitiveEditRefusal, TrackEdit, ViaEdit, track_edit_command, via_edit_command,
};
use crate::pcb::tests::{SEG_1, VIA_1, board, id, snapshot};
use crate::pcb::{BoardCommand, BoardCommandError, BoardItemId, BoardPatchKind};

fn lock(b: &mut signex_types::pcb::PcbBoard, items: Vec<(BoardItemId, bool)>) -> BoardCommand {
    let applied = BoardCommand::SetItemsLocked { items }.apply(b).unwrap();
    assert_eq!(applied.patch.kind, BoardPatchKind::ItemsLockSet);
    applied.inverse
}

#[test]
fn a_locked_track_refuses_an_edit_and_a_delete_until_unlocked() {
    let mut b = board();
    let track = BoardItemId::Segment(id(SEG_1));
    let before = snapshot(&b);
    // Built while the track is free: the engine itself refuses it once
    // the track is locked.
    let moved = track_edit_command(&b, &[id(SEG_1)], TrackEdit::LocationX(5.0))
        .unwrap()
        .unwrap();
    let undo = lock(&mut b, vec![(track, true)]);
    assert!(b.segments[0].locked);

    // The Properties edit of a locked track is refused before a command
    // is built (#2378).
    assert_eq!(
        track_edit_command(&b, &[id(SEG_1)], TrackEdit::LocationX(5.0)),
        Err(PrimitiveEditRefusal::ItemLocked)
    );
    assert_eq!(
        moved.apply(&mut b.clone()),
        Err(BoardCommandError::ItemLocked(track))
    );
    assert!(matches!(
        placement::delete_commands(&b, vec![track]),
        Err(PlacementRefusal::Locked(_))
    ));

    undo.apply(&mut b).unwrap();
    assert_eq!(snapshot(&b), before, "the unlock is the undo");
    assert!(placement::delete_commands(&b, vec![track]).is_ok());
}

#[test]
fn only_tracks_and_vias_take_a_lock() {
    let mut b = board();
    let footprint = BoardItemId::Footprint(id(crate::pcb::tests::FP_A));
    assert_eq!(
        BoardCommand::SetItemsLocked {
            items: vec![(footprint, true)]
        }
        .apply(&mut b),
        Err(BoardCommandError::NotLockable(footprint))
    );
    let via = BoardItemId::Via(id(VIA_1));
    assert_eq!(
        BoardCommand::SetItemsLocked {
            items: vec![(via, true), (via, false)]
        }
        .apply(&mut b),
        Err(BoardCommandError::DuplicateItem(via))
    );
}

#[test]
fn a_via_span_change_makes_it_blind_and_back_through() {
    let mut b = board();
    // The board lists no layers: Top and Bottom only, so give it four.
    b.layers = ["TopCopper", "Inner1Copper", "Inner2Copper", "BottomCopper"]
        .iter()
        .enumerate()
        .map(|(i, name)| signex_types::pcb::LayerDef {
            id: i as u8,
            name: (*name).into(),
            layer_type: "signal".into(),
            display_name: String::new(),
        })
        .collect();
    let inner2 = CopperLayer::Inner(InnerCopper::new(2).unwrap());
    let span = via_edit_command(&b, &[id(VIA_1)], ViaEdit::Span(inner2, CopperLayer::Top))
        .unwrap()
        .unwrap();
    span.apply(&mut b).unwrap();
    assert_eq!(b.vias[0].via_type, ViaType::Blind);
    assert_eq!(b.vias[0].layers, ["TopCopper", "Inner2Copper"]);

    let through = via_edit_command(
        &b,
        &[id(VIA_1)],
        ViaEdit::Span(CopperLayer::Top, CopperLayer::Bottom),
    )
    .unwrap()
    .unwrap();
    through.apply(&mut b).unwrap();
    assert_eq!(b.vias[0].via_type, ViaType::Through);
}

#[test]
fn a_via_span_off_the_board_is_refused() {
    let mut b = board();
    let inner5 = CopperLayer::Inner(InnerCopper::new(5).unwrap());
    let span = via_edit_command(&b, &[id(VIA_1)], ViaEdit::Span(CopperLayer::Top, inner5))
        .unwrap()
        .unwrap();
    assert!(matches!(
        span.apply(&mut b),
        Err(BoardCommandError::BadViaSpan(_))
    ));
}
