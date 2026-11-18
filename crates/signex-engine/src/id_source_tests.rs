//! Caller-minted ids (#2429): a command list run twice from the same sheet
//! and the same `SequentialIds` gives the same document, ids included, and a
//! new id already in the sheet is refused.

use signex_types::object_id::{IdSource, SequentialIds};
use signex_types::schematic::{
    Bus, BusEntry, Junction, Label, LabelType, LibSymbol, NoConnect, Point, SchematicSheet,
    SelectedItem, SelectedKind, Symbol, TextNote, Wire,
};
use uuid::Uuid;

use crate::test_support::test_sheet;
use crate::{
    Command, Engine, EngineError, PlacementPayload, child_sheet_from_corners, new_sheet_entry,
};

/// One item of every kind a paste places, in `fresh_place_commands` order.
fn payload() -> PlacementPayload {
    let mut f = SchematicSheet::default();
    let lib: LibSymbol = serde_json::from_str(r#"{"id":"lib:r"}"#).unwrap();
    f.lib_symbols.insert(lib.id.clone(), lib);
    let source = |n: u128| Uuid::from_u128(0xA00 + n);
    f.symbols.push(Symbol {
        uuid: source(1),
        lib_id: "lib:r".to_owned(),
        reference: "R1".to_owned(),
        position: Point::new(0.0, -20.0),
        unit: 1,
        ..Default::default()
    });
    f.wires.push(Wire {
        uuid: source(2),
        start: Point::new(0.0, 0.0),
        end: Point::new(20.0, 0.0),
        ..Default::default()
    });
    f.buses.push(Bus {
        uuid: source(3),
        start: Point::new(0.0, 30.0),
        end: Point::new(20.0, 30.0),
    });
    f.bus_entries.push(BusEntry {
        uuid: source(4),
        position: Point::new(5.0, 30.0),
        size: (2.54, 2.54),
    });
    f.junctions.push(Junction {
        uuid: source(5),
        position: Point::new(40.0, 40.0),
        ..Default::default()
    });
    f.no_connects.push(NoConnect {
        uuid: source(6),
        position: Point::new(50.0, 0.0),
    });
    f.labels.push(Label {
        uuid: source(7),
        ..Label::new("N", Point::new(0.0, -10.0), LabelType::Net)
    });
    f.text_notes.push(TextNote {
        uuid: source(8),
        text: "note".into(),
        position: Point::new(0.0, 5.0),
        rotation: 0.0,
        font_size: 1.27,
        font_family: None,
        justify_h: Default::default(),
        justify_v: Default::default(),
        locked: false,
    });
    PlacementPayload::new(f)
}

/// Paste, break the pasted wire, place a sheet symbol and an entry on it:
/// four undo steps, every new id from `ids`.
fn run_steps(start: &SchematicSheet, ids: &mut SequentialIds) -> Engine {
    let mut engine = Engine::new(start.clone()).unwrap();
    let commands = payload()
        .place_commands(Point::new(10.0, 10.0), ids)
        .unwrap();
    engine.execute_batch(commands).unwrap();
    let wire = engine.document().wires[0].uuid;
    let new_wire = ids.next_id();
    let at = Point::new(20.0, 10.0);
    engine
        .execute(Command::BreakWire { wire, at, new_wire })
        .unwrap();
    let corners = (Point::new(50.0, 50.0), Point::new(80.0, 80.0));
    let child_sheet = child_sheet_from_corners(corners.0, corners.1, "sheet1", ids).unwrap();
    let sheet_id = child_sheet.uuid;
    let pin = new_sheet_entry(&child_sheet, Point::new(48.0, 60.0), "P1", None, ids);
    engine
        .execute(Command::PlaceChildSheet { child_sheet })
        .unwrap();
    engine
        .execute(Command::PlaceSheetPin { sheet_id, pin })
        .unwrap();
    engine
}

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

#[test]
fn the_same_commands_twice_give_the_same_document_and_ids() {
    let start = test_sheet();
    let first = run_steps(&start, &mut SequentialIds::default());
    let second = run_steps(&start, &mut SequentialIds::default());
    assert_eq!(first.serialize().unwrap(), second.serialize().unwrap());
    let doc = first.document();
    assert_eq!(doc.symbols[0].uuid, id(1));
    let wire_ids: Vec<Uuid> = doc.wires.iter().map(|w| w.uuid).collect();
    assert_eq!(wire_ids, [id(2), id(9)]);
    assert_eq!(doc.buses[0].uuid, id(3));
    assert_eq!(doc.bus_entries[0].uuid, id(4));
    assert_eq!(doc.junctions[0].uuid, id(5));
    assert_eq!(doc.no_connects[0].uuid, id(6));
    assert_eq!(doc.labels[0].uuid, id(7));
    assert_eq!(doc.text_notes[0].uuid, id(8));
    assert_eq!(doc.child_sheets[0].uuid, id(10));
    assert_eq!(doc.child_sheets[0].pins[0].uuid, id(11));
}

#[test]
fn undo_then_redo_keeps_every_id() {
    let mut engine = run_steps(&test_sheet(), &mut SequentialIds::default());
    let done = engine.serialize().unwrap();
    for _ in 0..4 {
        engine.undo().unwrap().expect("an undo step");
    }
    assert!(engine.document().wires.is_empty());
    for _ in 0..4 {
        engine.redo().unwrap().expect("a redo step");
    }
    assert_eq!(engine.serialize().unwrap(), done);
}

/// `command` is refused with `IdInUse(taken)`; sheet and undo depth unchanged.
fn assert_refused(engine: &mut Engine, command: Command, taken: Uuid) {
    let before = engine.serialize().unwrap();
    let depth = engine.undo_depth();
    let refused = engine.execute(command);
    assert!(
        matches!(refused, Err(EngineError::IdInUse(got)) if got == taken),
        "{refused:?}"
    );
    assert_eq!(engine.serialize().unwrap(), before);
    assert_eq!(engine.undo_depth(), depth);
}

#[test]
fn break_wire_refuses_a_second_half_id_already_in_the_sheet() {
    let mut engine = run_steps(&test_sheet(), &mut SequentialIds::default());
    let wire = engine.document().wires[1].uuid;
    let at = Point::new(25.0, 10.0);
    assert_refused(
        &mut engine,
        Command::BreakWire {
            wire,
            at,
            new_wire: id(8),
        },
        id(8),
    );
}

#[test]
fn a_sheet_symbol_reusing_an_id_is_refused() {
    let mut engine = run_steps(&test_sheet(), &mut SequentialIds::default());
    let corners = (Point::new(100.0, 50.0), Point::new(130.0, 80.0));
    // A replay from a fresh source: the next ids are the sheet's own 1, 2, ...
    let ids = &mut SequentialIds::default();
    let mut child_sheet = child_sheet_from_corners(corners.0, corners.1, "sheet2", ids).unwrap();
    assert_refused(
        &mut engine,
        Command::PlaceChildSheet {
            child_sheet: child_sheet.clone(),
        },
        id(1),
    );
    // A fresh symbol id, but an entry that reuses one.
    child_sheet.uuid = id(500);
    child_sheet.pins.push(new_sheet_entry(
        &child_sheet,
        Point::new(98.0, 60.0),
        "A",
        None,
        ids,
    ));
    assert_refused(&mut engine, Command::PlaceChildSheet { child_sheet }, id(2));
}

#[test]
fn a_sheet_entry_reusing_an_id_is_refused() {
    let mut engine = run_steps(&test_sheet(), &mut SequentialIds::default());
    let symbol = engine.document().child_sheets[0].clone();
    let pin = new_sheet_entry(
        &symbol,
        Point::new(82.0, 60.0),
        "P2",
        None,
        &mut SequentialIds::default(),
    );
    let sheet_id = symbol.uuid;
    assert_refused(&mut engine, Command::PlaceSheetPin { sheet_id, pin }, id(1));
}

#[test]
fn a_paste_reusing_an_id_in_the_sheet_is_refused_whole() {
    let mut engine = run_steps(&test_sheet(), &mut SequentialIds::default());
    let before = engine.serialize().unwrap();
    let depth = engine.undo_depth();
    // A fresh source repeats id 1, which the first paste already placed.
    let commands = payload()
        .place_commands(Point::new(10.0, 40.0), &mut SequentialIds::default())
        .unwrap();
    let refused = engine.execute_batch(commands);
    assert!(matches!(refused, Err(EngineError::IdInUse(got)) if got == id(1)));
    assert_eq!(engine.serialize().unwrap(), before);
    assert_eq!(engine.undo_depth(), depth);
}

fn no_connect(n: u128, x: f64) -> Command {
    Command::PlaceNoConnect {
        no_connect: NoConnect {
            uuid: id(n),
            position: Point::new(x, 90.0),
        },
    }
}

/// The batch index sees ids placed earlier in the same batch.
#[test]
fn a_batch_placing_one_id_twice_is_refused() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    let refused = engine.execute_batch([no_connect(700, 0.0), no_connect(700, 5.0)]);
    assert!(matches!(refused, Err(EngineError::IdInUse(got)) if got == id(700)));
    assert!(engine.document().no_connects.is_empty());
}

/// An id deleted earlier in the batch is free again, as for single commands.
#[test]
fn a_batch_may_reuse_an_id_it_deleted() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    engine.execute(no_connect(700, 0.0)).unwrap();
    let delete = Command::DeleteSelection {
        items: vec![SelectedItem::new(id(700), SelectedKind::NoConnect)],
    };
    engine
        .execute_batch([delete, no_connect(700, 5.0)])
        .unwrap();
    assert_eq!(engine.document().no_connects[0].position.x, 5.0);
}

fn wire(n: u128, from: f64, to: f64) -> Command {
    Command::PlaceWireSegment {
        wire: Wire {
            uuid: id(n),
            start: Point::new(from, 0.0),
            end: Point::new(to, 0.0),
            ..Default::default()
        },
    }
}

fn engine_with_wire() -> Engine {
    let mut engine = Engine::new(test_sheet()).unwrap();
    engine.execute(wire(100, 0.0, 10.0)).unwrap();
    engine
}

/// A placement that folds a wire away frees its id for the rest of the batch.
#[test]
fn a_batch_may_reuse_an_id_a_wire_merge_removed() {
    let mut probe = engine_with_wire();
    probe.execute(wire(101, 10.0, 20.0)).unwrap();
    let kept: Vec<Uuid> = probe.document().wires.iter().map(|w| w.uuid).collect();
    assert_eq!(kept.len(), 1, "the collinear wires fold into one");
    let gone = if kept[0] == id(100) { 101 } else { 100 };

    let mut engine = engine_with_wire();
    engine
        .execute_batch([wire(101, 10.0, 20.0), no_connect(gone, 50.0)])
        .unwrap();
    assert_eq!(engine.document().no_connects[0].uuid, id(gone));
}

/// Every command that places an object under a caller-minted id refuses one
/// already in the sheet.
#[test]
fn every_placing_command_refuses_an_id_in_use() {
    let mut engine = run_steps(&test_sheet(), &mut SequentialIds::default());
    let taken = id(1);
    let f = payload().fragment().clone();
    let corners = (Point::new(100.0, 50.0), Point::new(130.0, 80.0));
    let ids = &mut SequentialIds::default();
    let sheet_taken = child_sheet_from_corners(corners.0, corners.1, "sheet2", ids).unwrap();
    assert_eq!(sheet_taken.uuid, taken);
    let mut pin_taken = sheet_taken.clone();
    pin_taken.uuid = id(900);
    let mut entry = new_sheet_entry(&pin_taken, Point::new(98.0, 60.0), "A", None, ids);
    entry.uuid = taken;
    pin_taken.pins.push(entry.clone());
    let at = Point::new(100.0, 100.0);
    let point = signex_types::schematic::NoConnect {
        uuid: taken,
        position: at,
    };
    let commands = vec![
        Command::BreakWire {
            wire: engine.document().wires[1].uuid,
            at: Point::new(25.0, 10.0),
            new_wire: taken,
        },
        Command::PlaceSymbol {
            symbol: Symbol {
                uuid: taken,
                ..f.symbols[0].clone()
            },
        },
        wire(1, 100.0, 110.0),
        Command::PlaceWireSegmentAsIs {
            wire: Wire {
                uuid: taken,
                ..f.wires[0].clone()
            },
        },
        Command::PlaceBus {
            bus: Bus {
                uuid: taken,
                ..f.buses[0].clone()
            },
        },
        Command::PlaceBusEntry {
            bus_entry: BusEntry {
                uuid: taken,
                ..f.bus_entries[0].clone()
            },
        },
        Command::PlaceJunction {
            junction: Junction {
                uuid: taken,
                ..f.junctions[0].clone()
            },
        },
        Command::PlaceNoConnect {
            no_connect: point.clone(),
        },
        Command::PlaceNoErcDirective {
            directive: signex_types::schematic::NoErcDirective::new(point.uuid, point.position),
        },
        Command::PlaceLabel {
            label: Label {
                uuid: taken,
                ..f.labels[0].clone()
            },
        },
        Command::PlaceTextNote {
            text_note: TextNote {
                uuid: taken,
                ..f.text_notes[0].clone()
            },
        },
        Command::PlaceSchDrawing {
            drawing: signex_types::schematic::SchDrawing::Line {
                uuid: taken,
                start: at,
                end: Point::new(110.0, 100.0),
                width: 0.2,
                stroke_color: None,
                locked: false,
            },
        },
        Command::PlaceChildSheet {
            child_sheet: sheet_taken,
        },
        Command::PlaceChildSheet {
            child_sheet: pin_taken,
        },
        Command::PlaceSheetPin {
            sheet_id: engine.document().child_sheets[0].uuid,
            pin: entry,
        },
    ];
    assert_eq!(commands.len(), 15);
    for command in commands {
        assert_refused(&mut engine, command, taken);
    }
}
