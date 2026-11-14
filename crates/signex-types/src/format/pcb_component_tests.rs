//! The component settings of a board footprint (#2365): saved in the
//! footprint's `[extras.footprints.<uuid>.component]` table, each key only
//! when it is not its default, and read back exactly.

use super::*;
use crate::pcb::{ComponentAttributes, ComponentType, Footprint, Point as PcbPoint};
use uuid::Uuid;

const OLD_BOARD: &str = include_str!("../../tests/corpus/pcb/footprint-schematic-link.snxpcb");

fn footprint(n: u128, component: ComponentAttributes) -> Footprint {
    Footprint {
        uuid: Uuid::from_u128(n),
        reference: format!("R{n}"),
        value: String::new(),
        footprint_id: String::new(),
        position: PcbPoint { x: 1.0, y: 2.0 },
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        pads: Vec::new(),
        graphics: Vec::new(),
        properties: Vec::new(),
        schematic_link: None,
        library_ref: None,
        component,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn every_field_set() -> ComponentAttributes {
    ComponentAttributes {
        component_type: ComponentType::Mechanical,
        height_mm: 2.5,
        body_opacity: 0.4,
        lock_primitives: true,
        lock_strings: true,
        pin_swapping: true,
        part_swapping: true,
        channel_offset: 3,
        description: "Chip resistor, \"0603\"\nthick film".into(),
    }
}

#[test]
fn every_component_setting_round_trips() {
    let mut board = super::tests::empty_board();
    board.footprints = vec![
        footprint(1, every_field_set()),
        footprint(2, ComponentAttributes::default()),
    ];
    let text = SnxPcb::new(board.clone()).write_string().unwrap();
    let back = SnxPcb::parse(&text).unwrap().board;
    assert_eq!(back.footprints[0].component, every_field_set());
    assert_eq!(back.footprints[1].component, ComponentAttributes::default());
    assert_eq!(SnxPcb::new(back).write_string().unwrap(), text, "stable");
}

#[test]
fn only_settings_off_their_default_are_written() {
    let mut board = super::tests::empty_board();
    let component = ComponentAttributes {
        lock_strings: true,
        ..ComponentAttributes::default()
    };
    board.footprints = vec![footprint(1, component)];
    let text = SnxPcb::new(board).write_string().unwrap();
    assert!(text.contains("lock_strings = true"), "{text}");
    for key in ["type =", "height_mm", "body_opacity", "lock_primitives", "description"] {
        assert!(!text.contains(key), "{key} is a default: {text}");
    }
}

#[test]
fn a_board_without_settings_resaves_byte_identical() {
    let parsed = SnxPcb::parse(OLD_BOARD).unwrap();
    assert!(
        parsed
            .board
            .footprints
            .iter()
            .all(|f| f.component.is_default())
    );
    assert_eq!(parsed.write_string().unwrap(), OLD_BOARD);
}

#[test]
fn a_non_finite_height_refuses_the_save() {
    let mut board = super::tests::empty_board();
    let component = ComponentAttributes {
        height_mm: f64::NAN,
        ..ComponentAttributes::default()
    };
    board.footprints = vec![footprint(1, component)];
    assert!(SnxPcb::new(board).write_string().is_err());
}

#[test]
fn an_unknown_type_is_refused_on_load() {
    let mut board = super::tests::empty_board();
    board.footprints = vec![footprint(1, every_field_set())];
    let text = SnxPcb::new(board).write_string().unwrap();
    let broken = text.replace("type = \"mechanical\"", "type = \"teleporter\"");
    assert_ne!(broken, text, "the type key is written as expected");
    assert!(SnxPcb::parse(&broken).is_err());
}
