//! The component settings (#2365), parameter add and remove and the shared
//! Comment (#2369): each builds the commands of one undo step.

use std::collections::BTreeSet;

use signex_types::pcb::{ComponentType, PcbBoard};
use signex_types::property::PcbProperty;
use uuid::Uuid;

use super::*;
use crate::pcb::BoardCommandError;
use crate::pcb::placement::PlacementRefusal;
use crate::pcb::tests::{FP_A, FP_B, board, id, snapshot};

fn ids(list: &[&str]) -> BTreeSet<Uuid> {
    list.iter().map(|s| id(s)).collect()
}

/// Apply `commands` to `board` as one step and return its inverse list, in
/// the order an undo runs it.
fn run(board: &mut PcbBoard, commands: Vec<BoardCommand>) -> Vec<BoardCommand> {
    let mut inverses: Vec<BoardCommand> = commands
        .into_iter()
        .map(|c| c.apply(board).unwrap().inverse)
        .collect();
    inverses.reverse();
    inverses
}

fn undo(board: &mut PcbBoard, inverses: Vec<BoardCommand>) {
    for inverse in inverses {
        inverse.apply(board).unwrap();
    }
}

fn fp<'a>(board: &'a PcbBoard, which: &str) -> &'a signex_types::pcb::Footprint {
    board.footprints.iter().find(|f| f.uuid == id(which)).unwrap()
}

#[test]
fn typed_settings_parse_heights_opacities_and_offsets() {
    assert_eq!(
        parse_setting(TypedSetting::Height, " 1.5 mm"),
        Ok(ComponentSetting::Height(1.5))
    );
    assert_eq!(
        parse_setting(TypedSetting::BodyOpacity, "40%"),
        Ok(ComponentSetting::BodyOpacity(0.4))
    );
    assert_eq!(
        parse_setting(TypedSetting::BodyOpacity, "0.25"),
        Ok(ComponentSetting::BodyOpacity(0.25))
    );
    assert_eq!(
        parse_setting(TypedSetting::ChannelOffset, "12"),
        Ok(ComponentSetting::ChannelOffset(12))
    );
    for (setting, bad) in [
        (TypedSetting::Height, "tall"),
        (TypedSetting::BodyOpacity, "nan"),
        (TypedSetting::ChannelOffset, "-1"),
        (TypedSetting::ChannelOffset, "1.5"),
    ] {
        assert!(parse_setting(setting, bad).is_err(), "{setting:?} {bad}");
    }
}

#[test]
fn a_setting_is_written_on_every_footprint_and_undone_exactly() {
    let mut b = board();
    let before = snapshot(&b);
    let both = ids(&[FP_A, FP_B]);
    let set = ComponentSetting::Type(ComponentType::Mechanical);
    let commands = setting_commands(&b, &both, &set).unwrap();
    assert_eq!(commands.len(), 2);
    let inverses = run(&mut b, commands);
    assert_eq!(fp(&b, FP_A).component.component_type, ComponentType::Mechanical);
    assert_eq!(fp(&b, FP_B).component.component_type, ComponentType::Mechanical);
    assert!(setting_commands(&b, &both, &set).unwrap().is_empty());
    undo(&mut b, inverses);
    assert_eq!(snapshot(&b), before);
}

#[test]
fn an_out_of_range_setting_or_a_locked_footprint_is_refused() {
    let mut b = board();
    let a = ids(&[FP_A]);
    assert_eq!(
        setting_commands(&b, &a, &ComponentSetting::Height(-1.0)),
        Err(ComponentRefusal::OutOfRange("height"))
    );
    assert_eq!(
        setting_commands(&b, &a, &ComponentSetting::BodyOpacity(1.2)),
        Err(ComponentRefusal::OutOfRange("body_opacity"))
    );
    b.footprints[0].locked = true;
    assert!(matches!(
        setting_commands(&b, &a, &ComponentSetting::LockStrings(true)),
        Err(ComponentRefusal::Placement(PlacementRefusal::Locked(_)))
    ));
}

#[test]
fn the_command_refuses_settings_the_board_cannot_store() {
    let mut b = board();
    let command = BoardCommand::SetFootprintComponent {
        footprint: id(FP_A),
        component: signex_types::pcb::ComponentAttributes {
            body_opacity: f64::NAN,
            ..Default::default()
        },
    };
    assert!(matches!(
        command.apply(&mut b),
        Err(BoardCommandError::ComponentFieldInvalid {
            field: "body_opacity",
            ..
        })
    ));
}

#[test]
fn locked_strings_refuse_a_text_move_but_not_a_hide() {
    let mut b = board();
    b.footprints[0].component.lock_strings = true;
    let mut moved = b.footprints[0].properties[0].clone();
    moved.position = Some(signex_types::pcb::Point::new(5.0, 5.0));
    let move_text = BoardCommand::SetFootprintProperty {
        footprint: id(FP_A),
        index: 0,
        property: moved,
    };
    assert_eq!(
        move_text.clone().apply(&mut b).err(),
        Some(BoardCommandError::StringsLocked(id(FP_A)))
    );
    let mut hidden = b.footprints[0].properties[0].clone();
    hidden.hidden = true;
    let hide = BoardCommand::SetFootprintProperty {
        footprint: id(FP_A),
        index: 0,
        property: hidden,
    };
    assert!(hide.apply(&mut b).is_ok());
    b.footprints[0].component.lock_strings = false;
    assert!(move_text.apply(&mut b).is_ok());
}

#[test]
fn a_parameter_is_added_to_every_footprint_and_undone_exactly() {
    let mut b = board();
    let before = snapshot(&b);
    let both = ids(&[FP_A, FP_B]);
    let commands = add_parameter_commands(&b, &both, " MPN ", "RC0603").unwrap();
    let inverses = run(&mut b, commands);
    for which in [FP_A, FP_B] {
        let added = fp(&b, which).properties.last().unwrap();
        assert_eq!((added.key.as_str(), added.value.as_str()), ("MPN", "RC0603"));
        assert!(added.hidden, "a new parameter is not drawn");
    }
    assert!(matches!(
        add_parameter_commands(&b, &both, "MPN", "x"),
        Err(ComponentRefusal::ParameterTaken { ref footprints, .. }) if footprints.len() == 2
    ));
    undo(&mut b, inverses);
    assert_eq!(snapshot(&b), before);
}

#[test]
fn an_empty_or_text_parameter_name_is_refused() {
    let b = board();
    let a = ids(&[FP_A]);
    assert_eq!(
        add_parameter_commands(&b, &a, "  ", "x"),
        Err(ComponentRefusal::EmptyName)
    );
    assert!(matches!(
        add_parameter_commands(&b, &a, "Value", "x"),
        Err(ComponentRefusal::TextParameter(_))
    ));
    assert!(matches!(
        remove_parameter_commands(&b, &a, "Reference"),
        Err(ComponentRefusal::TextParameter(_))
    ));
}

#[test]
fn a_parameter_is_removed_and_undo_puts_it_back_in_its_place() {
    let mut b = board();
    let extra = |key: &str| PcbProperty {
        key: key.into(),
        value: "v".into(),
        ..PcbProperty::default()
    };
    b.footprints[0].properties.insert(0, extra("MPN"));
    b.footprints[0].properties.push(extra("MPN"));
    b.footprints[1].properties.push(extra("MPN"));
    let before = snapshot(&b);
    let both = ids(&[FP_A, FP_B]);
    let commands = remove_parameter_commands(&b, &both, "MPN").unwrap();
    let inverses = run(&mut b, commands);
    assert_eq!(fp(&b, FP_A).properties[0].key, "Reference");
    assert_eq!(fp(&b, FP_A).properties.len(), 2, "only the first MPN goes");
    assert!(fp(&b, FP_B).properties.is_empty());
    undo(&mut b, inverses);
    assert_eq!(snapshot(&b), before);
    assert!(matches!(
        remove_parameter_commands(&board(), &both, "MPN"),
        Err(ComponentRefusal::NoParameter { .. })
    ));
}

#[test]
fn one_comment_overwrites_every_footprint_and_keeps_the_designators() {
    let mut b = board();
    b.footprints[0].value = "10k".into();
    b.footprints[1].value = "4k7".into();
    let before = snapshot(&b);
    let both = ids(&[FP_A, FP_B]);
    let commands = comment_commands(&b, &both, "22k").unwrap();
    let inverses = run(&mut b, commands);
    assert_eq!(fp(&b, FP_A).value, "22k");
    assert_eq!(fp(&b, FP_B).value, "22k");
    assert_eq!(fp(&b, FP_A).reference, "U1");
    assert!(comment_commands(&b, &both, "22k").unwrap().is_empty());
    undo(&mut b, inverses);
    assert_eq!(snapshot(&b), before);
}

#[test]
fn a_replaced_footprint_keeps_board_choices_and_takes_library_facts() {
    let mut b = board();
    *b.footprints[0].component = signex_types::pcb::ComponentAttributes {
        component_type: ComponentType::Graphical,
        lock_primitives: true,
        body_opacity: 0.5,
        channel_offset: 4,
        description: "old".into(),
        ..Default::default()
    };
    let mut template = b.footprints[0].clone();
    template.uuid = Uuid::from_u128(0xfeed);
    template.pads.clear();
    template.layer = "TopCopper".into();
    *template.component = signex_types::pcb::ComponentAttributes {
        component_type: ComponentType::Standard,
        height_mm: 2.0,
        description: "new".into(),
        ..Default::default()
    };
    BoardCommand::ReplaceFootprint {
        footprint: id(FP_A),
        with: Box::new(template),
    }
    .apply(&mut b)
    .unwrap();
    let kept = &fp(&b, FP_A).component;
    assert_eq!(kept.component_type, ComponentType::Standard);
    assert_eq!((kept.height_mm, kept.description.as_str()), (2.0, "new"));
    assert!(kept.lock_primitives);
    assert_eq!((kept.body_opacity, kept.channel_offset), (0.5, 4));
}
