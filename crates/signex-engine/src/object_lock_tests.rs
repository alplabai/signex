//! The lock of wires, labels, text notes and drawings (#1573) is enforced by
//! the same command-level path as the symbol lock.

use crate::rubber_band_tests::{add_resistor, add_wire};
use crate::*;
use signex_types::schematic::{
    HAlign, Label, LabelType, Point, SchDrawing, SelectedKind, TextNote, VAlign, Wire,
};
use uuid::Uuid;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn wire_at(n: u128, y: f64, locked: bool) -> Wire {
    Wire {
        uuid: id(n),
        start: Point::new(0.0, y),
        end: Point::new(10.0, y),
        locked,
        ..Default::default()
    }
}

fn label_at(n: u128, y: f64, locked: bool) -> Label {
    Label {
        uuid: id(n),
        text: format!("NET{n}"),
        position: Point::new(0.0, y),
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        font_family: None,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        locked,
        unknown_columns: Default::default(),
    }
}

fn note_at(n: u128, y: f64, locked: bool) -> TextNote {
    TextNote {
        uuid: id(n),
        text: "note".into(),
        position: Point::new(0.0, y),
        rotation: 0.0,
        font_size: 1.27,
        font_family: None,
        justify_h: HAlign::Left,
        justify_v: VAlign::Bottom,
        locked,
    }
}

fn line_at(n: u128, y: f64, locked: bool) -> SchDrawing {
    SchDrawing::Line {
        uuid: id(n),
        start: Point::new(0.0, y),
        end: Point::new(10.0, y),
        width: 0.2,
        stroke_color: None,
        locked,
    }
}

/// Ids 1-4 are locked (wire, label, note, line), 11-14 are their free twins.
fn engine() -> Engine {
    let sheet = SchematicSheet {
        wires: vec![wire_at(1, 0.0, true), wire_at(11, 20.0, false)],
        labels: vec![label_at(2, 5.0, true), label_at(12, 25.0, false)],
        text_notes: vec![note_at(3, 8.0, true), note_at(13, 28.0, false)],
        drawings: vec![line_at(4, 12.0, true), line_at(14, 32.0, false)],
        ..Default::default()
    };
    Engine::new(sheet).expect("engine")
}

fn item(n: u128, kind: SelectedKind) -> SelectedItem {
    SelectedItem::new(id(n), kind)
}

fn locked_items() -> [SelectedItem; 4] {
    [
        item(1, SelectedKind::Wire),
        item(2, SelectedKind::Label),
        item(3, SelectedKind::TextNote),
        item(4, SelectedKind::Drawing),
    ]
}

fn free_items() -> [SelectedItem; 4] {
    [
        item(11, SelectedKind::Wire),
        item(12, SelectedKind::Label),
        item(13, SelectedKind::TextNote),
        item(14, SelectedKind::Drawing),
    ]
}

fn assert_refused(engine: &mut Engine, cmd: Command, what: &str) {
    let before = format!("{:?}", engine.document());
    match engine.execute(cmd) {
        Err(EngineError::ItemLocked { .. }) => {}
        other => panic!("{what}: expected a lock refusal, got {other:?}"),
    }
    assert_eq!(format!("{:?}", engine.document()), before, "{what}");
    assert!(engine.undo().is_ok_and(|undone| undone.is_none()), "{what}");
}

#[test]
fn every_edit_of_a_locked_object_is_refused_and_leaves_the_sheet_alone() {
    let mut engine = engine();
    for locked in locked_items() {
        let kind = format!("{:?}", locked.kind);
        let items = vec![locked];
        assert_refused(
            &mut engine,
            Command::MoveSelection {
                items: items.clone(),
                dx: 5.0,
                dy: 0.0,
                keep_wires: true,
            },
            &format!("move {kind}"),
        );
        assert_refused(
            &mut engine,
            Command::MoveEach {
                moves: vec![(locked, 5.0, 0.0)],
                keep_wires: false,
            },
            &format!("move each {kind}"),
        );
        assert_refused(
            &mut engine,
            Command::DeleteSelection {
                items: items.clone(),
            },
            &format!("delete {kind}"),
        );
        assert_refused(
            &mut engine,
            Command::RotateSelection {
                items: items.clone(),
                angle_degrees: 90.0,
                keep_wires: false,
            },
            &format!("rotate {kind}"),
        );
        assert_refused(
            &mut engine,
            Command::MirrorSelection {
                items: items.clone(),
                axis: MirrorAxis::Horizontal,
                keep_wires: false,
            },
            &format!("mirror {kind}"),
        );
        assert_refused(
            &mut engine,
            Command::TransformSelection {
                items,
                centre: Point::new(0.0, 0.0),
                transform: SelectionTransform::RotateClockwise,
                keep_wires: false,
            },
            &format!("transform {kind}"),
        );
    }
}

#[test]
fn a_property_edit_of_a_locked_label_note_or_drawing_is_refused() {
    let mut engine = engine();
    assert_refused(
        &mut engine,
        Command::UpdateText {
            target: TextTarget::Label(id(2)),
            value: "X".into(),
        },
        "label text",
    );
    assert_refused(
        &mut engine,
        Command::UpdateText {
            target: TextTarget::TextNote(id(3)),
            value: "X".into(),
        },
        "note text",
    );
    assert_refused(
        &mut engine,
        Command::UpdateLabelProps {
            label_id: id(2),
            font_size_mm: Some(2.0),
            justify: None,
            rotation_degrees: None,
        },
        "label props",
    );
    assert_refused(
        &mut engine,
        Command::UpdateSchDrawing {
            drawing: line_at(4, 99.0, true),
        },
        "drawing",
    );
    assert_refused(
        &mut engine,
        Command::SetWireWidth {
            wire_id: id(1),
            width_mm: 0.5,
        },
        "wire width",
    );
    // The free twin's width still changes.
    assert!(
        engine
            .execute(Command::SetWireWidth {
                wire_id: id(11),
                width_mm: 0.5,
            })
            .is_ok_and(|r| r.changed)
    );
}

#[test]
fn the_free_twins_still_edit() {
    let mut engine = engine();
    for free in free_items() {
        let result = engine.execute(Command::MoveSelection {
            items: vec![free],
            dx: 1.0,
            dy: 0.0,
            keep_wires: false,
        });
        assert!(result.is_ok_and(|r| r.changed), "{:?}", free.kind);
    }
}

#[test]
fn a_group_edit_skips_the_locked_objects_and_names_them() {
    let engine = engine();
    let items: Vec<SelectedItem> = locked_items().into_iter().chain(free_items()).collect();
    let (pruned, skipped) = engine.without_locked(Command::DeleteSelection { items });

    let Some(Command::DeleteSelection { items: kept }) = pruned else {
        panic!("the free objects remain");
    };
    assert_eq!(kept, free_items().to_vec());
    assert_eq!(skipped.len(), 4, "{skipped:?}");
    assert!(skipped.iter().any(|name| name.starts_with("wire ")));
    assert!(skipped.iter().any(|name| name.starts_with("label ")));
    assert!(skipped.iter().any(|name| name.starts_with("text ")));
    assert!(skipped.iter().any(|name| name.starts_with("line ")));
}

#[test]
fn a_command_of_nothing_but_locked_objects_prunes_to_none() {
    let engine = engine();
    let (pruned, skipped) = engine.without_locked(Command::MoveSelection {
        items: locked_items().to_vec(),
        dx: 1.0,
        dy: 1.0,
        keep_wires: true,
    });
    assert!(pruned.is_none());
    assert_eq!(skipped.len(), 4);
}

#[test]
fn set_item_locked_locks_and_unlocks_in_one_undo_step_each() {
    let mut engine = engine();
    for free in free_items() {
        let lock = Command::SetItemLocked {
            item: free,
            locked: true,
        };
        assert!(engine.execute(lock).is_ok_and(|r| r.changed));
        assert_eq!(engine.item_locked(&free), Some(true), "{:?}", free.kind);
        assert!(matches!(
            engine.execute(Command::DeleteSelection { items: vec![free] }),
            Err(EngineError::ItemLocked { .. })
        ));
        // Locking again is a no-op; unlocking is the way out of the lock.
        let again = Command::SetItemLocked {
            item: free,
            locked: true,
        };
        assert!(engine.execute(again).is_ok_and(|r| !r.changed));
        let unlock = Command::SetItemLocked {
            item: free,
            locked: false,
        };
        assert!(engine.execute(unlock).is_ok_and(|r| r.changed));
        assert_eq!(engine.item_locked(&free), Some(false));
    }
    assert!(engine.undo().is_ok_and(|undone| undone.is_some()));
    assert_eq!(
        engine.item_locked(&item(14, SelectedKind::Drawing)),
        Some(true)
    );
}

#[test]
fn set_item_locked_ignores_a_kind_that_cannot_be_locked() {
    let mut engine = engine();
    let junction = item(1, SelectedKind::Junction);
    let result = engine.execute(Command::SetItemLocked {
        item: junction,
        locked: true,
    });
    assert!(result.is_ok_and(|r| !r.changed));
    assert_eq!(engine.item_locked(&junction), None);
}

#[test]
fn a_drawing_edit_keeps_the_lock_it_was_given() {
    let mut engine = engine();
    let free = item(14, SelectedKind::Drawing);
    engine
        .execute(Command::SetItemLocked {
            item: free,
            locked: true,
        })
        .unwrap();
    engine
        .execute(Command::SetItemLocked {
            item: free,
            locked: false,
        })
        .unwrap();
    engine
        .execute(Command::UpdateSchDrawing {
            drawing: line_at(14, 40.0, true),
        })
        .unwrap();
    assert_eq!(
        engine.item_locked(&free),
        Some(false),
        "an edit never locks"
    );
}

/// `R1` at (50, 50) with a wire into its left pin.
fn symbol_with_a_wire(wire_locked: bool) -> (Engine, SelectedItem) {
    let mut sheet = SchematicSheet::default();
    let symbol = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    let wire = add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    sheet.wires[0].locked = wire_locked;
    assert_eq!(sheet.wires[0].uuid, wire);
    (
        Engine::new(sheet).expect("engine"),
        item(symbol.as_u128(), SelectedKind::Symbol),
    )
}

#[test]
fn a_symbol_whose_pin_carries_a_locked_wire_does_not_drag_it() {
    let (mut engine, symbol) = symbol_with_a_wire(true);
    let wire_before = engine.document().wires[0].clone();
    let result = engine.execute(Command::MoveSelection {
        items: vec![symbol],
        dx: 0.0,
        dy: 5.0,
        keep_wires: true,
    });
    match result {
        Err(EngineError::ItemLocked { references, .. }) => {
            assert!(references[0].starts_with("R1 "), "{references:?}")
        }
        other => panic!("expected a lock refusal, got {other:?}"),
    }
    assert_eq!(engine.document().wires[0], wire_before);

    let (pruned, skipped) = engine.without_locked(Command::MoveSelection {
        items: vec![symbol],
        dx: 0.0,
        dy: 5.0,
        keep_wires: true,
    });
    assert!(pruned.is_none());
    assert_eq!(skipped.len(), 1);
}

#[test]
fn a_plain_move_leaves_a_locked_wire_alone_and_is_allowed() {
    let (mut engine, symbol) = symbol_with_a_wire(true);
    let wire_before = engine.document().wires[0].clone();
    let result = engine.execute(Command::MoveSelection {
        items: vec![symbol],
        dx: 0.0,
        dy: 5.0,
        keep_wires: false,
    });
    assert!(result.is_ok_and(|r| r.changed));
    assert_eq!(engine.document().wires[0], wire_before);
}

#[test]
fn an_unlocked_wire_is_still_followed_by_the_rubber_band() {
    let (mut engine, symbol) = symbol_with_a_wire(false);
    let result = engine.execute(Command::MoveSelection {
        items: vec![symbol],
        dx: 0.0,
        dy: 5.0,
        keep_wires: true,
    });
    assert!(result.is_ok_and(|r| r.changed));
}

#[test]
fn the_selection_details_of_every_lockable_kind_carry_a_locked_row() {
    let engine = engine();
    let row = |item: SelectedItem| {
        engine
            .describe_single_selection(&[item])
            .expect("details")
            .info
            .into_iter()
            .find(|(key, _)| key == "Locked")
            .map(|(_, value)| value)
    };
    for locked in locked_items() {
        assert_eq!(row(locked).as_deref(), Some("Yes"), "{:?}", locked.kind);
    }
    for free in free_items() {
        assert_eq!(row(free).as_deref(), Some("No"), "{:?}", free.kind);
    }
}

#[test]
fn a_sheet_pin_whose_wire_is_locked_does_not_drag_it() {
    let (mut engine, _, pin_id) = crate::rubber_band_tests::child_fixture();
    let lock = SelectedItem::new(engine.document().wires[0].uuid, SelectedKind::Wire);
    engine
        .execute(Command::SetItemLocked {
            item: lock,
            locked: true,
        })
        .unwrap();
    let wire_before = engine.document().wires[0].clone();
    let slide = Command::MoveSelection {
        items: vec![SelectedItem::new(pin_id, SelectedKind::SheetPin)],
        dx: 0.0,
        dy: 8.0,
        keep_wires: true,
    };
    assert!(matches!(
        engine.execute(slide.clone()),
        Err(EngineError::ItemLocked { .. })
    ));
    let (pruned, skipped) = engine.without_locked(slide);
    assert!(pruned.is_none());
    assert_eq!(skipped.len(), 1);
    assert_eq!(engine.document().wires, vec![wire_before]);
}

/// A leg the rubber band adds is a new wire: it never inherits a lock.
#[test]
fn a_rubber_band_leg_is_not_born_locked() {
    let (engine, _, pin_id) = crate::rubber_band_tests::child_fixture();
    let item = SelectedItem::new(pin_id, SelectedKind::SheetPin);
    let tips = moved_pin_tips(engine.document(), &[item]);
    let mut doc = engine.document().clone();
    doc.wires[0].locked = true;
    doc.child_sheets[0].pins[0].position.y += 8.0;
    let follow = follow_moved_pins(&mut doc, &tips, &[item], &Policy::default());
    assert!(!follow.touched.is_empty(), "the wire followed");
    for touched in &follow.touched {
        let wire = doc.wires.iter().find(|w| w.uuid == touched.uuid);
        if let Some(wire) = wire.filter(|w| w.uuid != engine.document().wires[0].uuid) {
            assert!(!wire.locked, "a new leg was born locked");
        }
    }
}

/// R1 at (50, 50): pin 1 (45, 50). W1 runs from (30, 50) to the pin; W2 hangs
/// from W1's far end, (30, 50)-(30, 70), and is locked.
fn elbow_with_a_locked_leg() -> (Engine, Uuid, Uuid) {
    use crate::rubber_band_tests::{add_resistor, add_wire};
    let mut sheet = SchematicSheet::default();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    let w2 = add_wire(&mut sheet, (30.0, 50.0), (30.0, 70.0));
    sheet.wires[1].locked = true;
    (Engine::new(sheet).expect("engine"), r1, w2)
}

#[test]
fn a_keep_wires_move_does_not_slide_a_locked_elbow_leg() {
    let (mut engine, r1, w2) = elbow_with_a_locked_leg();
    let leg_before = engine.document().wires[1].clone();
    let result = engine.execute(Command::MoveSelection {
        items: vec![item(r1.as_u128(), SelectedKind::Symbol)],
        dx: 0.0,
        dy: 5.0,
        keep_wires: true,
    });
    assert!(result.is_ok_and(|r| r.changed));
    let leg = engine
        .document()
        .wires
        .iter()
        .find(|w| w.uuid == w2)
        .unwrap();
    assert_eq!(*leg, leg_before, "the locked leg did not move");
    let pin = engine.document().symbols[0].position;
    assert_eq!(pin, Point::new(50.0, 55.0), "the symbol did move");
}

#[test]
fn the_skip_and_refuse_paths_agree_on_a_locked_elbow_leg() {
    let (engine, r1, _) = elbow_with_a_locked_leg();
    let command = Command::MoveSelection {
        items: vec![item(r1.as_u128(), SelectedKind::Symbol)],
        dx: 0.0,
        dy: 5.0,
        keep_wires: true,
    };
    // Neither path objects: the leg is simply not slid.
    let (pruned, skipped) = engine.without_locked(command);
    assert!(pruned.is_some(), "the move is kept whole");
    assert!(skipped.is_empty());
}

/// R1 at (50, 50), pin 1 at (45, 50). W1 (30,50)-(45,50); locked L hangs from
/// W1's far end, (30,50)-(30,70). Moving R1 by (-15, +5) with the wires
/// following brings W1 onto L.
fn wire_lands_on_a_locked_one(l_end_y: f64) -> (Engine, Uuid, crate::rubber_band_tests::NetShape) {
    use crate::rubber_band_tests::{add_resistor, add_wire, net_shape};
    let mut sheet = SchematicSheet::default();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (30.0, l_end_y));
    sheet.wires[1].locked = true;
    let before = net_shape(&sheet);
    (Engine::new(sheet).expect("engine"), r1, before)
}

fn overlapping_pairs(engine: &Engine) -> usize {
    let wires = &engine.document().wires;
    let mut n = 0;
    for (i, a) in wires.iter().enumerate() {
        for b in &wires[i + 1..] {
            n += usize::from(crate::transform::rubber_band::wires_overlap(a, b));
        }
    }
    n
}

/// A route along a locked wire is not a route, so the pass cannot keep R1's
/// pin on W1 here: it falls back to a plain move for that pin and says so.
/// Nothing is stacked on L, no dot is added, and L is untouched. The
/// trimmed-remainder control is in `overlap::tests`.
#[test]
fn a_wire_dragged_onto_a_locked_one_falls_back_to_a_plain_move() {
    let (mut engine, r1, _) = wire_lands_on_a_locked_one(70.0);
    let wires_before = engine.document().wires.clone();
    engine
        .execute(Command::MoveSelection {
            items: vec![item(r1.as_u128(), SelectedKind::Symbol)],
            dx: -15.0,
            dy: 5.0,
            keep_wires: true,
        })
        .unwrap();
    let doc = engine.document();
    assert_eq!(doc.wires, wires_before, "no wire was rerouted or trimmed");
    assert!(doc.junctions.is_empty(), "no dot was added");
    assert_eq!(overlapping_pairs(&engine), 0);
    assert_eq!(doc.symbols[0].position, Point::new(35.0, 55.0), "R1 moved");
    let warnings = engine.take_warnings();
    assert!(
        warnings.iter().any(|w| w.contains("could not keep")),
        "the detached pin is reported: {warnings:?}"
    );
}

/// A dot left at the pin would outlive it: after R1 moved on, a wire crossing
/// L at that point would join R2 and R3 into one net.
#[test]
fn moving_off_a_locked_wire_leaves_nothing_that_joins_nets_later() {
    use crate::rubber_band_tests::{add_resistor, add_wire, net_shape};
    let mut sheet = SchematicSheet::default();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (30.0, 70.0));
    sheet.wires[1].locked = true;
    // R3's pin 1 sits on L's far end, R2's pin 1 at (105, 55).
    let r3 = add_resistor(&mut sheet, "R3", Point::new(35.0, 70.0));
    let r2 = add_resistor(&mut sheet, "R2", Point::new(110.0, 55.0));
    let mut engine = Engine::new(sheet).expect("engine");
    let mv = |dx, dy, keep_wires| Command::MoveSelection {
        items: vec![item(r1.as_u128(), SelectedKind::Symbol)],
        dx,
        dy,
        keep_wires,
    };
    engine.execute(mv(-15.0, 5.0, true)).unwrap();
    engine.execute(mv(40.0, 40.0, false)).unwrap();
    engine
        .execute(Command::PlaceWireSegment {
            wire: Wire {
                uuid: Uuid::new_v4(),
                start: Point::new(20.0, 55.0),
                end: Point::new(105.0, 55.0),
                ..Default::default()
            },
        })
        .unwrap();
    let joined = net_shape(engine.document())
        .iter()
        .any(|(_, pins)| pins.iter().any(|(u, _)| *u == r2) && pins.iter().any(|(u, _)| *u == r3));
    assert!(!joined, "R2 and R3 stay in separate nets");
}

/// Bus (0,0)-(0,50) with an entry at (0,10) whose far end (2.54, 12.54) holds
/// the start of a locked wire.
fn bus_with_a_locked_wire_on_an_entry() -> (Engine, SelectedItem) {
    use signex_types::schematic::{Bus, BusEntry};
    let mut sheet = SchematicSheet::default();
    let bus = Uuid::new_v4();
    sheet.buses.push(Bus {
        uuid: bus,
        start: Point::new(0.0, 0.0),
        end: Point::new(0.0, 50.0),
    });
    sheet.bus_entries.push(BusEntry {
        uuid: Uuid::new_v4(),
        position: Point::new(0.0, 10.0),
        size: (2.54, 2.54),
    });
    crate::rubber_band_tests::add_wire(&mut sheet, (2.54, 12.54), (20.0, 12.54));
    sheet.wires[0].locked = true;
    (
        Engine::new(sheet).expect("engine"),
        SelectedItem::new(bus, SelectedKind::Bus),
    )
}

#[test]
fn a_bus_whose_entry_far_end_holds_a_locked_wire_does_not_drag_it() {
    let (mut engine, bus) = bus_with_a_locked_wire_on_an_entry();
    let wire_before = engine.document().wires[0].clone();
    let drag = Command::MoveSelection {
        items: vec![bus],
        dx: 5.0,
        dy: 0.0,
        keep_wires: true,
    };

    assert!(matches!(
        engine.execute(drag.clone()),
        Err(EngineError::ItemLocked { .. })
    ));
    let (pruned, skipped) = engine.without_locked(drag);
    assert!(pruned.is_none(), "the bus is skipped");
    assert_eq!(skipped.len(), 1);
    assert!(skipped[0].starts_with("bus"), "{skipped:?}");
    assert_eq!(engine.document().wires[0], wire_before);

    // Without the wires following, the bus moves and the wire stays.
    let plain = Command::MoveSelection {
        items: vec![bus],
        dx: 5.0,
        dy: 0.0,
        keep_wires: false,
    };
    assert!(engine.execute(plain).is_ok_and(|r| r.changed));
    assert_eq!(engine.document().wires[0], wire_before);
}
