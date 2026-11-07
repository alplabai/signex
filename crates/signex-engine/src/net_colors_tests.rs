//! `Command::SetNetColors`: the sheet's net colours are a step of its undo
//! history, and a save writes them (#661).

use crate::test_support::test_sheet;
use crate::{Command, DocumentPatch, Engine, EngineError};
use signex_types::schematic::{Point, SchematicSheet, Wire};
use signex_types::theme::Color;
use std::collections::HashMap;
use uuid::Uuid;

fn red() -> Color {
    Color::new(255, 0, 0, 255)
}

fn sheet_with_wire() -> (SchematicSheet, Uuid) {
    let id = Uuid::now_v7();
    let mut sheet = test_sheet();
    sheet.wires.push(Wire {
        uuid: id,
        start: Point::new(0.0, 0.0),
        end: Point::new(20.0, 0.0),
        ..Default::default()
    });
    (sheet, id)
}

fn paint(engine: &mut Engine, id: Uuid, color: Color) {
    let mut colors = engine.document().net_colors.clone();
    colors.insert(id, color);
    engine
        .execute(Command::SetNetColors { colors })
        .expect("colour");
}

#[test]
fn a_colour_is_one_undo_step_with_its_own_patch_flag() {
    let (sheet, wire) = sheet_with_wire();
    let mut engine = Engine::new(sheet).unwrap();

    let done = engine
        .execute(Command::SetNetColors {
            colors: HashMap::from([(wire, red())]),
        })
        .unwrap();
    assert!(done.changed);
    assert_eq!(
        done.patch_pair.map(|p| p.document),
        Some(DocumentPatch::NET_COLORS)
    );
    assert_eq!(engine.document().net_colors.get(&wire), Some(&red()));

    engine.undo().unwrap().expect("one step");
    assert!(engine.document().net_colors.is_empty());
    assert!(!engine.can_undo(), "the colour was the only step");
    engine.redo().unwrap().expect("redo");
    assert_eq!(engine.document().net_colors.get(&wire), Some(&red()));
}

#[test]
fn the_same_colours_again_change_nothing_and_record_nothing() {
    let (sheet, wire) = sheet_with_wire();
    let mut engine = Engine::new(sheet).unwrap();
    paint(&mut engine, wire, red());
    let revision = engine.revision();

    let again = engine
        .execute(Command::SetNetColors {
            colors: HashMap::from([(wire, red())]),
        })
        .unwrap();

    assert!(!again.changed);
    assert_eq!(engine.revision(), revision);
    engine.undo().unwrap();
    assert!(!engine.can_undo(), "no second step was recorded");
}

#[test]
fn undoing_a_colour_back_to_the_save_point_is_at_the_save_point() {
    let (sheet, wire) = sheet_with_wire();
    let mut engine = Engine::new(sheet).unwrap();
    engine.mark_saved();

    paint(&mut engine, wire, red());
    assert!(!engine.is_at_save_point());
    engine.undo().unwrap();
    assert!(engine.is_at_save_point());
}

#[test]
fn colours_fall_off_the_history_with_the_other_steps() {
    let (sheet, wire) = sheet_with_wire();
    let mut engine = Engine::new(sheet).unwrap();
    engine.set_undo_depth(1);
    paint(&mut engine, wire, red());
    engine
        .execute(Command::SetPaperSize {
            paper_size: "A3".into(),
        })
        .unwrap();

    engine.undo().unwrap().expect("the paper");
    assert!(
        engine.undo().unwrap().is_none(),
        "the colour step was evicted"
    );
    assert_eq!(engine.document().net_colors.get(&wire), Some(&red()));
}

#[test]
fn a_save_writes_the_colours_of_wires_the_sheet_holds_and_drops_the_rest() {
    let (sheet, wire) = sheet_with_wire();
    let mut engine = Engine::new(sheet).unwrap();
    let gone = Uuid::now_v7();
    engine
        .execute(Command::SetNetColors {
            colors: HashMap::from([(wire, red()), (gone, red())]),
        })
        .unwrap();

    for text in [
        engine.serialize().unwrap(),
        engine.serialize_with_extents(|_| None).unwrap(),
    ] {
        let saved = signex_types::format::SnxSchematic::parse(&text)
            .unwrap()
            .sheet;
        assert_eq!(saved.net_colors, HashMap::from([(wire, red())]));
    }
    assert!(
        engine.document().net_colors.contains_key(&gone),
        "the engine keeps it for an undo"
    );
}

#[test]
fn a_read_only_sheet_refuses_a_colour() {
    let (mut sheet, wire) = sheet_with_wire();
    sheet.unknown.newer_minor_source = Some("snxsch/1.7".into());
    let mut engine = Engine::new(sheet).unwrap();
    let refused = engine.execute(Command::SetNetColors {
        colors: HashMap::from([(wire, red())]),
    });
    assert!(matches!(refused, Err(EngineError::ReadOnly)));
}

#[test]
fn breaking_a_coloured_wire_paints_the_new_half_in_the_same_step() {
    let (sheet, wire) = sheet_with_wire();
    let mut engine = Engine::new(sheet).unwrap();
    paint(&mut engine, wire, red());

    let done = engine
        .execute(Command::BreakWire {
            wire,
            at: Point::new(8.0, 0.0),
        })
        .unwrap();

    assert!(
        done.patch_pair
            .is_some_and(|p| p.document.contains(DocumentPatch::NET_COLORS))
    );
    assert_eq!(engine.document().wires.len(), 2);
    assert!(
        engine
            .document()
            .wires
            .iter()
            .all(|w| engine.document().net_colors.get(&w.uuid) == Some(&red()))
    );
    engine.undo().unwrap().expect("one step");
    assert_eq!(engine.document().wires.len(), 1);
    assert_eq!(
        engine.document().net_colors.len(),
        1,
        "one undo undoes both"
    );
}
