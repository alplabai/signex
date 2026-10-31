//! The symbol lock is enforced by every command that would change the
//! symbol (#1250).

use crate::*;
use signex_types::schematic::{InstancePath, Point, SelectedKind, Symbol};
use uuid::Uuid;

fn lib_symbol_named(id: &str) -> signex_types::schematic::LibSymbol {
    serde_json::from_value(serde_json::json!({ "id": id })).unwrap()
}

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// R1 (locked) and R2 (free), side by side.
fn engine() -> Engine {
    let symbol = |n: u128, reference: &str, locked: bool| Symbol {
        uuid: id(n),
        reference: reference.into(),
        position: Point::new(10.0 * n as f64, 10.0),
        unit: 1,
        locked,
        ..Default::default()
    };
    let sheet = SchematicSheet {
        symbols: vec![symbol(1, "R1", true), symbol(2, "R2", false)],
        ..Default::default()
    };
    Engine::new(sheet).expect("engine")
}

fn item(n: u128) -> SelectedItem {
    SelectedItem::new(id(n), SelectedKind::Symbol)
}

fn locked_error(result: Result<CommandResult, EngineError>) -> String {
    match result {
        Err(error @ EngineError::ItemLocked { .. }) => error.to_string(),
        other => panic!("expected a lock refusal, got {other:?}"),
    }
}

/// Every command a locked R1 must refuse, by the path that issues it.
fn refused_commands() -> Vec<(&'static str, Command)> {
    vec![
        (
            "move",
            Command::MoveSelection {
                items: vec![item(1)],
                dx: 5.0,
                dy: 0.0,
                keep_wires: true,
            },
        ),
        (
            "move each (align / distribute)",
            Command::MoveEach {
                moves: vec![(item(1), 5.0, 0.0)],
                keep_wires: true,
            },
        ),
        (
            "move the reference text",
            Command::MoveSelection {
                items: vec![SelectedItem::new(id(1), SelectedKind::SymbolRefField)],
                dx: 5.0,
                dy: 0.0,
                keep_wires: false,
            },
        ),
        (
            "rotate",
            Command::RotateSelection {
                items: vec![item(1)],
                angle_degrees: 90.0,
                keep_wires: true,
            },
        ),
        (
            "mirror",
            Command::MirrorSelection {
                items: vec![item(1)],
                axis: MirrorAxis::Horizontal,
                keep_wires: true,
            },
        ),
        (
            "transform (rotate)",
            Command::TransformSelection {
                items: vec![item(1)],
                centre: Point::new(0.0, 0.0),
                transform: SelectionTransform::RotateClockwise,
                keep_wires: true,
            },
        ),
        (
            "transform (mirror)",
            Command::TransformSelection {
                items: vec![item(1)],
                centre: Point::new(0.0, 0.0),
                transform: SelectionTransform::Mirror(MirrorAxis::Vertical),
                keep_wires: true,
            },
        ),
        (
            "delete",
            Command::DeleteSelection {
                items: vec![item(1)],
            },
        ),
        (
            "set rotation",
            Command::SetSymbolRotation {
                symbol_id: id(1),
                rotation_degrees: 90.0,
            },
        ),
        (
            "move to X/Y",
            Command::MoveSymbolAbsolute {
                symbol_id: id(1),
                x: 1.0,
                y: 1.0,
            },
        ),
        (
            "edit the reference text",
            Command::UpdateText {
                target: TextTarget::SymbolReference(id(1)),
                value: "R9".into(),
            },
        ),
        (
            "edit the value text",
            Command::UpdateText {
                target: TextTarget::SymbolValue(id(1)),
                value: "10k".into(),
            },
        ),
        (
            "edit fields",
            Command::UpdateSymbolFields {
                symbol_id: id(1),
                reference: "R9".into(),
                value: "10k".into(),
                footprint: "0603".into(),
            },
        ),
        (
            "edit footprint",
            Command::UpdateSymbolFootprint {
                symbol_id: id(1),
                footprint: "0603".into(),
            },
        ),
        (
            "edit a parameter",
            Command::SetSymbolField {
                symbol_id: id(1),
                key: "MPN".into(),
                value: "X".into(),
            },
        ),
        (
            "delete a parameter",
            Command::RemoveSymbolField {
                symbol_id: id(1),
                key: "MPN".into(),
            },
        ),
        (
            "edit the text size",
            Command::UpdateSymbolTextSize {
                symbol_id: id(1),
                field: SymbolTextField::Reference,
                font_size_mm: 2.0,
            },
        ),
        (
            "edit the designator on a sheet placement",
            Command::SetSymbolDesignatorIn {
                symbol: id(1),
                placement: InstancePath::from_chain(vec![id(100), id(101)]),
                reference: "R9".into(),
            },
        ),
        (
            "change the component",
            Command::ChangeComponent {
                symbol_id: id(1),
                replacement: crate::ComponentReplacement {
                    lib_symbol: lib_symbol_named("Device:C"),
                    library_id: None,
                    row_id: None,
                    library_version: String::new(),
                    value: String::new(),
                    footprint: String::new(),
                    datasheet: String::new(),
                    fields: Vec::new(),
                },
            },
        ),
        (
            "change the library id",
            Command::UpdateSymbolLibId {
                symbol_id: id(1),
                lib_id: "Device:C".into(),
            },
        ),
    ]
}

#[test]
fn every_path_refuses_a_locked_symbol_and_leaves_the_sheet_alone() {
    for (path, command) in refused_commands() {
        let mut engine = engine();
        let before = engine.document().clone();
        let revision = engine.revision();

        let message = locked_error(engine.execute(command));

        assert!(message.contains("R1 is locked"), "{path}: {message}");
        assert!(message.contains("unlock it"), "{path}: {message}");
        assert_eq!(
            format!("{:?}", engine.document()),
            format!("{before:?}"),
            "{path}: the document changed"
        );
        assert_eq!(engine.revision(), revision, "{path}: moved the revision");
        assert!(
            engine.undo().is_ok_and(|undone| undone.is_none()),
            "{path}: left an undo step"
        );
    }
}

#[test]
fn a_mixed_selection_is_refused_whole() {
    let mut engine = engine();
    let free_before = engine.document().symbols[1].position;

    let result = engine.execute(Command::MoveSelection {
        items: vec![item(2), item(1)],
        dx: 5.0,
        dy: 0.0,
        keep_wires: true,
    });

    locked_error(result);
    assert_eq!(engine.document().symbols[1].position, free_before);
}

#[test]
fn a_batch_with_a_refused_command_rolls_back() {
    let mut engine = engine();
    let free_before = engine.document().symbols[1].position;

    let result = engine.execute_batch([
        Command::MoveSelection {
            items: vec![item(2)],
            dx: 5.0,
            dy: 0.0,
            keep_wires: true,
        },
        Command::DeleteSelection {
            items: vec![item(1)],
        },
    ]);

    assert!(matches!(result, Err(EngineError::ItemLocked { .. })));
    assert_eq!(engine.document().symbols[1].position, free_before);
    assert_eq!(engine.document().symbols.len(), 2);
}

#[test]
fn an_unlocked_symbol_still_moves_and_deletes() {
    let mut engine = engine();
    let moved = engine
        .execute(Command::MoveSelection {
            items: vec![item(2)],
            dx: 5.0,
            dy: 0.0,
            keep_wires: true,
        })
        .expect("move");
    assert!(moved.changed);
    engine
        .execute(Command::DeleteSelection {
            items: vec![item(2)],
        })
        .expect("delete");
    assert_eq!(engine.document().symbols.len(), 1);
}

#[test]
fn unlocking_lets_the_symbol_move_and_delete_again_and_undo_relocks() {
    let mut engine = engine();
    let unlock = |locked| Command::SetSymbolLocked {
        symbol_id: id(1),
        locked,
    };
    engine.execute(unlock(false)).expect("unlock");
    engine
        .execute(Command::MoveSelection {
            items: vec![item(1)],
            dx: 5.0,
            dy: 0.0,
            keep_wires: true,
        })
        .expect("move once unlocked");
    engine
        .execute(Command::DeleteSelection {
            items: vec![item(1)],
        })
        .expect("delete once unlocked");
    assert_eq!(engine.document().symbols.len(), 1);
    engine.undo().expect("undo delete");
    engine.undo().expect("undo move");
    engine.undo().expect("undo unlock");
    assert!(engine.document().symbols[0].locked);
    locked_error(engine.execute(Command::DeleteSelection {
        items: vec![item(1)],
    }));
}

#[test]
fn the_lock_and_dnp_flags_stay_editable_on_a_locked_symbol() {
    let mut engine = engine();
    let dnp = engine
        .execute(Command::SetSymbolDnp {
            symbol_id: id(1),
            dnp: true,
        })
        .expect("dnp");
    assert!(dnp.changed);
    let unlock = engine
        .execute(Command::SetSymbolLocked {
            symbol_id: id(1),
            locked: false,
        })
        .expect("unlock");
    assert!(unlock.changed);
}

#[test]
fn without_locked_drops_locked_items_and_names_them() {
    let engine = engine();
    let (pruned, skipped) = engine.without_locked(Command::MoveSelection {
        items: vec![item(1), item(2)],
        dx: 5.0,
        dy: 0.0,
        keep_wires: true,
    });
    assert_eq!(skipped, vec!["R1".to_string()]);
    match pruned {
        Some(Command::MoveSelection { items, .. }) => assert_eq!(items, vec![item(2)]),
        other => panic!("expected the free item to stay, got {other:?}"),
    }
}

#[test]
fn without_locked_leaves_nothing_of_an_all_locked_command() {
    let engine = engine();
    let (pruned, skipped) = engine.without_locked(Command::DeleteSelection {
        items: vec![item(1)],
    });
    assert!(pruned.is_none());
    assert_eq!(skipped, vec!["R1".to_string()]);

    let (pruned, skipped) = engine.without_locked(Command::UpdateText {
        target: TextTarget::SymbolValue(id(1)),
        value: "x".into(),
    });
    assert!(pruned.is_none());
    assert_eq!(skipped, vec!["R1".to_string()]);
}

#[test]
fn without_locked_keeps_a_command_that_touches_no_locked_symbol() {
    let engine = engine();
    let cmd = Command::MoveEach {
        moves: vec![(item(2), 5.0, 0.0)],
        keep_wires: true,
    };
    let (pruned, skipped) = engine.without_locked(cmd);
    assert!(skipped.is_empty());
    assert!(matches!(pruned, Some(Command::MoveEach { .. })));
}

#[test]
fn several_locked_symbols_are_all_named() {
    let mut engine = engine();
    engine
        .execute(Command::SetSymbolLocked {
            symbol_id: id(2),
            locked: true,
        })
        .expect("lock R2");
    let message = locked_error(engine.execute(Command::DeleteSelection {
        items: vec![item(2), item(1)],
    }));
    assert_eq!(
        message,
        "R1, R2 are locked and cannot be deleted; unlock them in Properties first"
    );
}

#[test]
fn the_drag_ghost_skips_a_locked_item_like_the_commit() {
    let engine = engine();
    let preview = MovePreview::new(engine.document());

    let ghost = preview.moved(&[(item(1), 5.0, 0.0), (item(2), 5.0, 0.0)], true);

    assert_eq!(
        ghost.symbols[0].position,
        engine.document().symbols[0].position
    );
    assert_eq!(
        ghost.symbols[1].position.x,
        engine.document().symbols[1].position.x + 5.0
    );
}

/// Locked `R1` at (50, 50) with a wire into its left pin, and a free wire
/// elsewhere.
fn locked_with_wires() -> (Engine, Uuid, Uuid, Uuid) {
    use crate::rubber_band_tests::{add_resistor, add_wire};
    let mut sheet = SchematicSheet::default();
    let symbol = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    sheet.symbols[0].locked = true;
    let attached = add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    let free = add_wire(&mut sheet, (0.0, 0.0), (10.0, 0.0));
    (Engine::new(sheet).expect("engine"), symbol, attached, free)
}

fn wire_item(uuid: Uuid) -> SelectedItem {
    SelectedItem::new(uuid, SelectedKind::Wire)
}

#[test]
fn a_group_move_leaves_wires_on_a_locked_pin_where_they_are() {
    let (mut engine, symbol, attached, free) = locked_with_wires();
    let (pruned, skipped) = engine.without_locked(Command::MoveSelection {
        items: vec![item_of(symbol), wire_item(attached), wire_item(free)],
        dx: 0.0,
        dy: 5.0,
        keep_wires: true,
    });
    assert_eq!(skipped, vec!["R1".to_string()]);
    engine
        .execute(pruned.expect("the free wire moves"))
        .unwrap();

    let doc = engine.document();
    let wire = |id| doc.wires.iter().find(|w| w.uuid == id).unwrap();
    assert_eq!((wire(attached).start.y, wire(attached).end.y), (50.0, 50.0));
    assert_eq!(wire(free).start.y, 5.0);
    assert_eq!(doc.symbols[0].position, Point::new(50.0, 50.0));
}

#[test]
fn deleting_a_wire_on_a_locked_pin_is_still_allowed() {
    let (engine, _, attached, _) = locked_with_wires();
    let (pruned, skipped) = engine.without_locked(Command::DeleteSelection {
        items: vec![wire_item(attached)],
    });
    assert!(skipped.is_empty());
    assert!(pruned.is_some());
}

fn item_of(uuid: Uuid) -> SelectedItem {
    SelectedItem::new(uuid, SelectedKind::Symbol)
}

/// U1 unit 1 (free) and unit 2 (locked), one part drawn as two units.
fn engine_with_a_locked_second_unit() -> Engine {
    let unit = |n: u128, unit: u32, locked: bool| Symbol {
        uuid: id(n),
        reference: "U1".into(),
        lib_id: "L:OLD".into(),
        position: Point::new(10.0 * n as f64, 10.0),
        unit,
        locked,
        ..Default::default()
    };
    let sheet = SchematicSheet {
        symbols: vec![unit(1, 1, false), unit(2, 2, true)],
        ..Default::default()
    };
    Engine::new(sheet).expect("engine")
}

#[test]
fn changing_the_component_of_a_free_unit_is_refused_while_a_sibling_unit_is_locked() {
    let mut engine = engine_with_a_locked_second_unit();
    let before = format!("{:?}", engine.document());
    let message = locked_error(engine.execute(Command::ChangeComponent {
        symbol_id: id(1),
        replacement: crate::ComponentReplacement {
            lib_symbol: lib_symbol_named("L:NEW"),
            library_id: None,
            row_id: None,
            library_version: String::new(),
            value: "NEW".into(),
            footprint: String::new(),
            datasheet: String::new(),
            fields: Vec::new(),
        },
    }));
    assert!(message.contains("U1 is locked"), "{message}");
    assert_eq!(format!("{:?}", engine.document()), before);
    assert!(engine.undo().is_ok_and(|undone| undone.is_none()));
}
