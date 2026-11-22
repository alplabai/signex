//! Rows m95, m96 and m97: the Track, Via and Pad forms.

use signex_app::panels::pcb_primitive_properties::{
    PcbPrimitiveFacts, PrimitiveField as F, PrimitiveFlag, ViaFacts, ViaMsg, end_choices,
};
use signex_app::panels::{PanelMsg, PcbComponentMsg, PcbPrimitiveMsg};
use signex_types::pcb::{MaskExpansion, PadShape, PcbBoard, Point, Via};
use signex_types::pcb_layer::CopperLayer;
use signex_widgets::property_illustration::Cap;
use uuid::Uuid;

use super::support::{Bench, FOOTPRINT, PAD_SMD, PAD_TH, TRACK, VIA, all_kinds, assert_at, open};

// ---- m95 Track ----

fn track_of(board: &PcbBoard) -> &signex_types::pcb::Segment {
    board
        .segments
        .iter()
        .find(|s| s.uuid == TRACK)
        .expect("the track")
}

#[test]
fn a_track_width_applies_on_enter_in_one_undo_step() {
    let mut b = open(all_kinds());
    let t = b.pick(5.0, 0.0);
    b.primitive(PcbPrimitiveMsg::FieldTyped {
        target: t.clone(),
        field: F::Width,
        text: "0.4".to_string(),
    });
    assert_eq!(
        track_of(&b.board()).width,
        0.25,
        "typing alone edits nothing"
    );
    let edited = b.one_step(|b| {
        b.primitive(PcbPrimitiveMsg::FieldSubmitted {
            target: t,
            field: F::Width,
        });
    });
    assert_eq!(track_of(&edited).width, 0.4);
}

#[test]
fn a_track_width_of_zero_or_text_is_refused() {
    let mut b = open(all_kinds());
    let t = b.pick(5.0, 0.0);
    for bad in ["0", "-1", "abc"] {
        b.refused(|b| b.enter(&t, F::Width, bad));
    }
}

#[test]
fn moving_a_track_start_keeps_its_end() {
    let mut b = open(all_kinds());
    let t = b.pick(5.0, 0.0);
    let edited = b.one_step(|b| b.enter(&t, F::StartX, "2"));
    assert_at(track_of(&edited).start, 2.0, 0.0);
    assert_at(track_of(&edited).end, 10.0, 0.0);
}

#[test]
fn a_track_layer_pick_is_one_undo_step() {
    let mut b = open(all_kinds());
    let t = b.pick(5.0, 0.0);
    let edited = b.one_step(|b| {
        b.primitive(PcbPrimitiveMsg::LayerPicked {
            target: t,
            layer: CopperLayer::Bottom,
        });
    });
    assert_eq!(track_of(&edited).layer, "BottomCopper");
}

#[test]
fn a_track_net_pick_is_one_undo_step_and_the_net_readout_follows() {
    let mut b = open(all_kinds());
    let t = b.pick(5.0, 0.0);
    let edited = b.one_step(|b| {
        b.primitive(PcbPrimitiveMsg::NetPicked { target: t, net: 2 });
        let PcbPrimitiveFacts::Track(shown) = b.facts() else {
            panic!("the track form shows");
        };
        assert_eq!(shown.common.net_info.name.as_deref(), Some("VCC"));
    });
    assert_eq!(track_of(&edited).net, 2);
}

#[test]
fn a_locked_track_refuses_edits_until_unlocked() {
    let mut b = open(all_kinds());
    let t = b.pick(5.0, 0.0);
    let flag = |on| PcbPrimitiveMsg::FlagToggled {
        target: t.clone(),
        flag: PrimitiveFlag::Locked,
        on,
    };
    let edited = b.one_step(|b| b.primitive(flag(true)));
    assert!(track_of(&edited).locked);
    b.primitive(flag(true));
    b.refused(|b| b.enter(&t, F::Width, "0.5"));
    b.primitive(flag(false));
    b.enter(&t, F::Width, "0.5");
    assert_eq!(track_of(&b.board()).width, 0.5);
}

#[test]
fn a_typed_track_mask_expansion_is_manual_and_rule_puts_it_back() {
    let mut b = open(all_kinds());
    let t = b.pick(5.0, 0.0);
    let edited = b.one_step(|b| b.enter(&t, F::SolderMask, "0.05"));
    assert_eq!(
        track_of(&edited).attributes.solder_mask,
        MaskExpansion::Manual { mm: 0.05 }
    );
    b.enter(&t, F::SolderMask, "0.05");
    b.primitive(PcbPrimitiveMsg::MaskModePicked {
        target: t,
        field: F::SolderMask,
        manual: false,
    });
    assert_eq!(
        track_of(&b.board()).attributes.solder_mask,
        MaskExpansion::Rule
    );
}

#[test]
fn the_ends_row_keeps_round_and_greys_square_and_flat_on_copper() {
    let choices = end_choices();
    assert_eq!(choices.map(|c| c.cap), [Cap::Round, Cap::Square, Cap::Flat]);
    assert!(choices[0].is_on && choices[0].is_enabled);
    assert!(choices[1..].iter().all(|c| !c.is_enabled));
}

// ---- m96 Via ----

fn via_facts(b: &Bench) -> ViaFacts {
    match b.facts() {
        PcbPrimitiveFacts::Via(via) => via,
        other => panic!("the via form, not {other:?}"),
    }
}

fn via_of(board: &PcbBoard, id: Uuid) -> &Via {
    board.vias.iter().find(|v| v.uuid == id).expect("the via")
}

#[test]
fn a_via_diameter_and_drill_edit_are_one_undo_step_each() {
    let mut b = open(all_kinds());
    let t = b.pick(20.0, 0.0);
    let edited = b.one_step(|b| b.enter(&t, F::Diameter, "0.8"));
    assert_eq!(via_of(&edited, VIA).diameter, 0.8);
    let edited = b.one_step(|b| b.enter(&t, F::HoleSize, "0.4"));
    assert_eq!(via_of(&edited, VIA).drill, 0.4);
}

#[test]
fn a_via_hole_as_big_as_the_via_or_text_is_refused() {
    let mut b = open(all_kinds());
    let t = b.pick(20.0, 0.0);
    for bad in ["0.6", "0.9", "abc"] {
        b.refused(|b| b.enter(&t, F::HoleSize, bad));
    }
}

#[test]
fn a_finished_hole_sets_the_drill_that_leaves_it() {
    let mut b = open(all_kinds());
    let t = b.pick(20.0, 0.0);
    let edited = b.one_step(|b| b.enter(&t, F::FinishedHole, "0.3"));
    assert!((via_of(&edited, VIA).drill - 0.35).abs() < 1e-6);
}

#[test]
fn a_via_alone_in_its_type_never_reads_as_overridden() {
    let mut b = open(all_kinds());
    let t = b.pick(20.0, 0.0);
    b.enter(&t, F::Diameter, "0.7");
    assert_eq!(via_facts(&b).overridden, Some(false));
}

/// Through vias at x 20 and 24 at 0.6/0.3 and one at x 28 at 0.8/0.3.
fn three_vias() -> PcbBoard {
    let mut board = all_kinds();
    let mut second = board.vias[0].clone();
    second.uuid = Uuid::from_u128(0xBE1);
    second.position = Point::new(24.0, 0.0);
    let mut third = board.vias[0].clone();
    third.uuid = Uuid::from_u128(0xBE2);
    third.position = Point::new(28.0, 0.0);
    third.diameter = 0.8;
    board.vias.push(second);
    board.vias.push(third);
    board
}

#[test]
fn changing_a_via_value_overrides_and_the_types_value_clears_it() {
    let mut b = open(three_vias());
    let t = b.pick(20.0, 0.0);
    assert_eq!(via_facts(&b).overridden, Some(false));
    b.enter(&t, F::Diameter, "0.7");
    assert_eq!(
        via_facts(&b).overridden,
        Some(true),
        "a changed value overrides"
    );
    b.enter(&t, F::Diameter, "0.6");
    assert_eq!(
        via_facts(&b).overridden,
        Some(false),
        "the type's value clears it"
    );
}

#[test]
fn a_via_reset_arrow_puts_the_types_size_back_in_one_step() {
    let mut b = open(three_vias());
    let t = b.pick(28.0, 0.0);
    let edited = b.one_step(|b| {
        b.primitive(PcbPrimitiveMsg::Via(ViaMsg::SizeReset {
            target: t,
            field: F::Diameter,
        }));
    });
    assert_eq!(via_of(&edited, Uuid::from_u128(0xBE2)).diameter, 0.6);
}

#[test]
fn a_locked_via_stays_read_only() {
    let mut b = open(all_kinds());
    let t = b.pick(20.0, 0.0);
    b.primitive(PcbPrimitiveMsg::LockToggled {
        target: t.clone(),
        locked: true,
    });
    for field in [F::Diameter, F::HoleSize, F::FinishedHole] {
        b.refused(|b| b.enter(&t, field, "0.2"));
    }
}

#[test]
fn via_tenting_is_one_undo_step() {
    let mut b = open(all_kinds());
    let t = b.pick(20.0, 0.0);
    let edited = b.one_step(|b| {
        b.primitive(PcbPrimitiveMsg::FlagToggled {
            target: t,
            flag: PrimitiveFlag::TentedTop,
            on: true,
        });
    });
    assert!(via_of(&edited, VIA).attributes.top.tented);
}

// ---- m97 Pad ----

fn pad_of(board: &PcbBoard, id: Uuid) -> signex_types::pcb::Pad {
    board.footprints[0]
        .pads
        .iter()
        .find(|p| p.uuid == id)
        .expect("the pad")
        .clone()
}

#[test]
fn a_pad_size_edit_is_one_undo_step() {
    let mut b = open(all_kinds());
    let t = b.pick(62.0, 10.0);
    let edited = b.one_step(|b| b.enter(&t, F::SizeX, "1.6"));
    assert_eq!(pad_of(&edited, PAD_SMD).size, Point::new(1.6, 0.6));
}

#[test]
fn a_pad_shape_pick_is_one_undo_step() {
    let mut b = open(all_kinds());
    let t = b.pick(62.0, 10.0);
    let edited = b.one_step(|b| {
        b.primitive(PcbPrimitiveMsg::ShapePicked {
            target: t,
            shape: PadShape::Circle,
        });
    });
    assert_eq!(pad_of(&edited, PAD_SMD).shape, PadShape::Circle);
}

#[test]
fn a_through_hole_pad_hole_edit_is_one_undo_step() {
    let mut b = open(all_kinds());
    let t = b.pick(58.0, 10.0);
    let edited = b.one_step(|b| b.enter(&t, F::HoleSize, "0.7"));
    let drill = pad_of(&edited, PAD_TH).drill.expect("a hole");
    assert_eq!(drill.diameter, 0.7);
}

#[test]
fn a_surface_pad_has_no_hole_to_edit() {
    let mut b = open(all_kinds());
    let t = b.pick(62.0, 10.0);
    let PcbPrimitiveFacts::Pad(pad) = b.facts() else {
        panic!("the pad form shows");
    };
    assert!(!pad.has_hole);
    assert!(pad.layer_sizes.is_empty(), "no Middle or Bottom rows");
    b.refused(|b| b.enter(&t, F::HoleSize, "0.4"));
}

#[test]
fn a_pad_size_that_is_not_a_number_is_refused() {
    let mut b = open(all_kinds());
    let t = b.pick(62.0, 10.0);
    b.refused(|b| b.enter(&t, F::SizeX, "abc"));
}

#[test]
fn a_pad_is_locked_with_its_footprint() {
    let mut b = open(all_kinds());
    let t = b.pick(62.0, 10.0);
    let board = b.path.clone();
    b.panel(PanelMsg::PcbComponent(PcbComponentMsg::LockToggled {
        board,
        footprints: vec![FOOTPRINT],
        locked: true,
    }));
    b.refused(|b| b.enter(&t, F::SizeX, "1.6"));
}
