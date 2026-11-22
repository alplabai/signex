//! Row m94: the Component form of a selected footprint.

use pcb_geom::ClickMode;
use signex_app::panels::{
    ComponentField, PanelMsg, PcbComponentFacts, PcbComponentMsg, PcbFootprintTextMsg, Shared,
};
use signex_engine::pcb::component::LocationField;
use signex_engine::pcb::component_edit::{ComponentSetting, TypedSetting};
use signex_engine::pcb::designators::FootprintTextField;
use signex_types::pcb::{Footprint, PcbBoard};
use signex_types::pcb_layer::BoardSide;
use uuid::Uuid;

use super::support::{Bench, FOOTPRINT, all_kinds, assert_at, open};

const SECOND: Uuid = Uuid::from_u128(0xBF1);

fn facts(b: &Bench) -> PcbComponentFacts {
    b.app
        .document_state
        .panel_ctx
        .pcb_board
        .as_ref()
        .and_then(|summary| summary.component.clone())
        .expect("the Component form shows")
}

fn selected() -> Bench {
    let mut b = open(all_kinds());
    b.click(60.0, 10.0);
    assert_eq!(facts(&b).footprints, [FOOTPRINT]);
    b
}

fn part(board: &PcbBoard) -> &Footprint {
    board
        .footprints
        .iter()
        .find(|fp| fp.uuid == FOOTPRINT)
        .expect("U1")
}

fn component(b: &mut Bench, msg: PcbComponentMsg) {
    b.panel(PanelMsg::PcbComponent(msg));
}

fn enter(b: &mut Bench, field: ComponentField, text: &str) {
    let footprints = facts(b).footprints;
    let board = b.path.clone();
    component(
        b,
        PcbComponentMsg::FieldTyped {
            board: board.clone(),
            footprints: footprints.clone(),
            field: field.clone(),
            text: text.to_string(),
        },
    );
    component(
        b,
        PcbComponentMsg::FieldSubmitted {
            board,
            footprints,
            field,
        },
    );
}

fn location(field: LocationField) -> ComponentField {
    ComponentField::Location(field)
}

#[test]
fn typing_x_moves_the_part_in_one_undo_step() {
    let mut b = selected();
    let edited = b.one_step(|b| enter(b, location(LocationField::X), "64"));
    assert_at(part(&edited).position, 64.0, 10.0);
}

#[test]
fn typing_y_counts_up_from_the_board_origin() {
    let mut b = selected();
    let edited = b.one_step(|b| enter(b, location(LocationField::Y), "-12"));
    assert_at(part(&edited).position, 60.0, 12.0);
}

#[test]
fn typing_a_rotation_turns_the_part_in_one_undo_step() {
    let mut b = selected();
    let edited = b.one_step(|b| enter(b, location(LocationField::Rotation), "90"));
    assert_eq!(part(&edited).rotation, 90.0);
}

#[test]
fn a_location_that_is_not_a_number_is_refused() {
    let mut b = selected();
    for field in [LocationField::X, LocationField::Y, LocationField::Rotation] {
        b.refused(|b| enter(b, location(field), "abc"));
    }
}

#[test]
fn the_layer_control_flips_the_part_in_one_undo_step() {
    let mut b = selected();
    let board = b.path.clone();
    let edited = b.one_step(|b| {
        component(
            b,
            PcbComponentMsg::SidePicked {
                board,
                footprints: vec![FOOTPRINT],
                side: BoardSide::Bottom,
            },
        );
        assert_eq!(facts(b).side, Shared::Same(BoardSide::Bottom));
    });
    assert_eq!(part(&edited).layer, "BottomCopper");
}

#[test]
fn the_designator_edit_is_one_undo_step() {
    let mut b = selected();
    let board = b.path.clone();
    let edited = b.one_step(|b| {
        b.panel(PanelMsg::PcbFootprintText(PcbFootprintTextMsg::Typed {
            board: board.clone(),
            footprint: FOOTPRINT,
            field: FootprintTextField::Designator,
            text: "U7".to_string(),
        }));
        b.panel(PanelMsg::PcbFootprintText(PcbFootprintTextMsg::Submitted {
            board,
            footprint: FOOTPRINT,
            field: FootprintTextField::Designator,
        }));
    });
    assert_eq!(part(&edited).reference, "U7");
}

#[test]
fn the_comment_edit_is_one_undo_step() {
    let mut b = selected();
    let edited = b.one_step(|b| enter(b, ComponentField::Comment, "STM32G431"));
    assert_eq!(part(&edited).value, "STM32G431");
}

#[test]
fn a_locked_part_refuses_a_move_until_unlocked() {
    let mut b = selected();
    let board = b.path.clone();
    let lock = |locked: bool| PcbComponentMsg::LockToggled {
        board: board.clone(),
        footprints: vec![FOOTPRINT],
        locked,
    };
    let edited = b.one_step(|b| component(b, lock(true)));
    assert!(part(&edited).locked);

    component(&mut b, lock(true));
    b.refused(|b| enter(b, location(LocationField::X), "70"));
    component(&mut b, lock(false));
    enter(&mut b, location(LocationField::X), "70");
    assert_at(part(&b.board()).position, 70.0, 10.0);
}

#[test]
fn a_typed_height_is_stored_in_one_undo_step() {
    let mut b = selected();
    let field = ComponentField::Setting(TypedSetting::Height);
    let edited = b.one_step(|b| enter(b, field, "0.9"));
    assert_eq!(part(&edited).component.height_mm, 0.9);
}

#[test]
fn a_negative_height_is_refused() {
    let mut b = selected();
    let field = ComponentField::Setting(TypedSetting::Height);
    b.refused(|b| enter(b, field, "-1"));
}

#[test]
fn a_body_opacity_outside_zero_to_one_is_refused() {
    let mut b = selected();
    for bad in ["1.5", "-0.1", "abc"] {
        let field = ComponentField::Setting(TypedSetting::BodyOpacity);
        b.refused(|b| enter(b, field, bad));
    }
}

#[test]
fn the_lock_primitives_box_is_one_undo_step() {
    let mut b = selected();
    let board = b.path.clone();
    let edited = b.one_step(|b| {
        component(
            b,
            PcbComponentMsg::SettingPicked {
                board,
                footprints: vec![FOOTPRINT],
                setting: ComponentSetting::LockPrimitives(true),
            },
        );
    });
    assert!(part(&edited).component.lock_primitives);
}

#[test]
fn the_designator_eye_hides_it_in_one_undo_step() {
    let mut board = all_kinds();
    board.footprints[0].properties = vec![signex_types::property::PcbProperty {
        key: "Reference".into(),
        value: "U1".into(),
        layer: Some("TopSilkscreen".into()),
        ..Default::default()
    }];
    let mut b = open(board);
    b.click(60.0, 10.0);
    let path = b.path.clone();
    let edited = b.one_step(|b| {
        component(
            b,
            PcbComponentMsg::TextShownToggled {
                board: path,
                footprints: vec![FOOTPRINT],
                field: FootprintTextField::Designator,
                shown: false,
            },
        );
        assert_eq!(facts(b).designator_shown, Some(Shared::Same(false)));
    });
    assert!(part(&edited).properties[0].hidden);
}

/// U1 and a second part, both top side, at x 60 and 80.
fn two_parts() -> Bench {
    let mut board = all_kinds();
    let mut second = board.footprints[0].clone();
    second.uuid = SECOND;
    second.reference = "U2".into();
    second.value = "ADC".into();
    second.position.x = 80.0;
    for (i, pad) in second.pads.iter_mut().enumerate() {
        pad.uuid = Uuid::from_u128(0xBF2 + i as u128);
    }
    board.footprints.push(second);
    let mut b = open(board);
    b.click(60.0, 10.0);
    b.click_with(80.0, 10.0, ClickMode::Add);
    b
}

#[test]
fn two_parts_show_differing_values_as_mixed() {
    let b = two_parts();
    let shown = facts(&b);
    assert_eq!(shown.footprints.len(), 2);
    assert_eq!(shown.x, Shared::Mixed);
    assert_eq!(shown.y, Shared::Same(-10.0));
    assert_eq!(shown.designator, Shared::Mixed);
    assert_eq!(shown.comment, Shared::Mixed);
}

#[test]
fn one_comment_typed_on_two_parts_is_one_undo_step() {
    let mut b = two_parts();
    let edited = b.one_step(|b| enter(b, ComponentField::Comment, "Shared"));
    assert!(edited.footprints.iter().all(|fp| fp.value == "Shared"));
}
