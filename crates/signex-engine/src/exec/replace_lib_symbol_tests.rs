//! `Command::ReplaceLibSymbol` (#912): replace + one undo step, and the
//! refusal when a connected pin would vanish or move.

use crate::test_support::test_sheet;
use crate::{Command, Engine, EngineError};
use signex_types::schematic::{
    Junction, LibPin, LibSymbol, LinkStamp, NoConnect, Pin, PinDirection, PinShapeStyle, Point,
    RowStamp, StampValues, Symbol, Wire,
};

const ID: &str = "L:S";

fn lib_pin(number: &str, x: f64) -> LibPin {
    LibPin {
        unit: 1,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(x, 0.0),
            rotation: 0.0,
            length: 2.54,
            name: number.to_string(),
            number: number.to_string(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    }
}

fn lib_symbol(description: &str, pins: Vec<LibPin>) -> LibSymbol {
    LibSymbol {
        id: ID.to_string(),
        reference: "U".into(),
        value: String::new(),
        footprint: String::new(),
        datasheet: String::new(),
        description: description.to_string(),
        keywords: String::new(),
        fp_filters: String::new(),
        in_bom: true,
        on_board: true,
        in_pos_files: true,
        duplicate_pin_numbers_are_jumpers: false,
        graphics: Vec::new(),
        pins,
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

/// The description of the body the first symbol draws with.
fn drawn(engine: &Engine) -> String {
    let doc = engine.document();
    doc.body_of(&doc.symbols[0])
        .expect("body")
        .description
        .clone()
}

/// A sheet with the symbol at (10, 10) and a wire on pin "2" (world 12.54, 10).
fn engine_with_wired_pin_2() -> Engine {
    let mut sheet = test_sheet();
    let old_ref = sheet
        .embed_body(lib_symbol(
            "old",
            vec![lib_pin("1", -2.54), lib_pin("2", 2.54)],
        ))
        .unwrap();
    sheet.symbols.push(Symbol {
        uuid: uuid::Uuid::now_v7(),
        lib_id: ID.into(),
        body_ref: old_ref,
        reference: "U1".into(),
        position: Point::new(10.0, 10.0),
        unit: 1,
        ..Default::default()
    });
    sheet.wires.push(Wire {
        start: Point::new(12.54, 10.0),
        end: Point::new(20.0, 10.0),
        ..Default::default()
    });
    Engine::new(sheet).unwrap()
}

#[test]
fn replace_swaps_the_copy_and_one_undo_restores_it() {
    let mut engine = engine_with_wired_pin_2();
    let fresh = lib_symbol("new", vec![lib_pin("1", -2.54), lib_pin("2", 2.54)]);
    let result = engine
        .execute(Command::ReplaceLibSymbol {
            only: None,
            lib_symbol: fresh,
            library_versions: vec![],
        })
        .unwrap();
    assert!(result.changed);
    assert_eq!(drawn(&engine), "new");

    engine.undo().unwrap();
    assert_eq!(drawn(&engine), "old");
    assert!(!engine.can_undo());
}

#[test]
fn replacing_with_an_identical_symbol_records_nothing() {
    let mut engine = engine_with_wired_pin_2();
    let same = lib_symbol("old", vec![lib_pin("1", -2.54), lib_pin("2", 2.54)]);
    let result = engine
        .execute(Command::ReplaceLibSymbol {
            only: None,
            lib_symbol: same,
            library_versions: vec![],
        })
        .unwrap();
    assert!(!result.changed);
    assert!(!engine.can_undo());
}

#[test]
fn a_vanishing_unconnected_pin_is_allowed() {
    let mut engine = engine_with_wired_pin_2();
    let fresh = lib_symbol("new", vec![lib_pin("2", 2.54)]);
    assert!(
        engine
            .execute(Command::ReplaceLibSymbol {
                only: None,
                lib_symbol: fresh,
                library_versions: vec![],
            })
            .unwrap()
            .changed
    );
}

#[test]
fn a_vanishing_connected_pin_is_refused_and_nothing_changes() {
    let mut engine = engine_with_wired_pin_2();
    let fresh = lib_symbol("new", vec![lib_pin("1", -2.54)]);
    let err = engine
        .execute(Command::ReplaceLibSymbol {
            only: None,
            lib_symbol: fresh,
            library_versions: vec![],
        })
        .unwrap_err();
    match err {
        EngineError::LibSymbolPinsConnected { pins, .. } => {
            assert_eq!(pins, vec!["U1 pin 2".to_string()]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(drawn(&engine), "old");
    assert!(!engine.can_undo());
}

#[test]
fn a_connected_pin_that_moves_is_refused() {
    let mut engine = engine_with_wired_pin_2();
    let fresh = lib_symbol("new", vec![lib_pin("1", -2.54), lib_pin("2", 5.08)]);
    assert!(
        engine
            .execute(Command::ReplaceLibSymbol {
                only: None,
                lib_symbol: fresh,
                library_versions: vec![],
            })
            .is_err()
    );
}

#[test]
fn a_label_on_a_pin_tip_counts_as_a_connection() {
    let mut sheet = engine_with_wired_pin_2().document().clone();
    // Pin "1" tip (7.46, 10) carries only a label, no wire.
    sheet.labels.push(signex_types::schematic::Label {
        justify: signex_types::schematic::HAlign::Left,
        justify_v: signex_types::schematic::VAlign::Bottom,
        ..signex_types::schematic::Label::new(
            "N",
            Point::new(7.46, 10.0),
            signex_types::schematic::LabelType::Net,
        )
    });
    let mut engine = Engine::new(sheet).unwrap();
    let fresh = lib_symbol("new", vec![lib_pin("2", 2.54)]);
    assert!(
        engine
            .execute(Command::ReplaceLibSymbol {
                only: None,
                lib_symbol: fresh,
                library_versions: vec![],
            })
            .is_err()
    );
}

#[test]
fn a_new_pin_landing_on_an_existing_connection_is_refused() {
    let mut engine = engine_with_wired_pin_2();
    // The wire's far end (20, 10) is a connection no old pin occupied.
    let fresh = lib_symbol(
        "new",
        vec![lib_pin("1", -2.54), lib_pin("2", 2.54), lib_pin("3", 10.0)],
    );
    match engine
        .execute(Command::ReplaceLibSymbol {
            only: None,
            lib_symbol: fresh,
            library_versions: vec![],
        })
        .unwrap_err()
    {
        EngineError::LibSymbolPinsConnected { pins, .. } => {
            assert_eq!(pins, vec!["U1 pin 3".to_string()]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_removed_unit_is_refused() {
    let mut engine = engine_with_wired_pin_2();
    let mut p = lib_pin("1", -2.54);
    p.unit = 2;
    let fresh = lib_symbol("new", vec![p]);
    assert!(matches!(
        engine
            .execute(Command::ReplaceLibSymbol {
                only: None,
                lib_symbol: fresh,
                library_versions: vec![],
            })
            .unwrap_err(),
        EngineError::LibSymbolUnitRemoved { .. }
    ));
}

/// A sheet with the symbol at (10, 10) and no wire: pin "1" tip is (7.46, 10),
/// pin "2" tip is (12.54, 10).
fn engine_with_bare_symbol() -> Engine {
    let mut sheet = engine_with_wired_pin_2().document().clone();
    sheet.wires.clear();
    Engine::new(sheet).unwrap()
}

fn drop_pin_2(engine: &mut Engine) -> Result<crate::CommandResult, EngineError> {
    let fresh = lib_symbol("new", vec![lib_pin("1", -2.54)]);
    engine.execute(Command::ReplaceLibSymbol {
        only: None,
        lib_symbol: fresh,
        library_versions: vec![],
    })
}

#[test]
fn a_junction_alone_on_a_pin_tip_counts_as_a_connection() {
    let mut engine = engine_with_bare_symbol();
    assert!(drop_pin_2(&mut engine).unwrap().changed, "bare pin is free");

    let mut sheet = engine_with_bare_symbol().document().clone();
    sheet.junctions.push(Junction {
        uuid: uuid::Uuid::now_v7(),
        position: Point::new(12.54, 10.0),
        ..Default::default()
    });
    let mut engine = Engine::new(sheet).unwrap();
    assert!(matches!(
        drop_pin_2(&mut engine).unwrap_err(),
        EngineError::LibSymbolPinsConnected { .. }
    ));
}

#[test]
fn a_no_connect_on_a_pin_tip_counts_as_a_connection() {
    let mut sheet = engine_with_bare_symbol().document().clone();
    sheet.no_connects.push(NoConnect {
        uuid: uuid::Uuid::now_v7(),
        position: Point::new(12.54, 10.0),
    });
    let mut engine = Engine::new(sheet).unwrap();
    assert!(matches!(
        drop_pin_2(&mut engine).unwrap_err(),
        EngineError::LibSymbolPinsConnected { .. }
    ));
}

#[test]
fn a_wire_passing_through_a_pin_without_a_junction_counts_as_a_connection() {
    let mut sheet = engine_with_bare_symbol().document().clone();
    // Vertical wire through (12.54, 10): its endpoints are not on the pin.
    sheet.wires.push(Wire {
        start: Point::new(12.54, 5.0),
        end: Point::new(12.54, 15.0),
        ..Default::default()
    });
    assert!(sheet.junctions.is_empty());
    let mut engine = Engine::new(sheet).unwrap();
    match drop_pin_2(&mut engine).unwrap_err() {
        EngineError::LibSymbolPinsConnected { pins, .. } => {
            assert_eq!(pins, vec!["U1 pin 2".to_string()]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_new_pin_on_a_connection_is_refused_when_the_sheet_has_no_embedded_copy() {
    let mut sheet = engine_with_wired_pin_2().document().clone();
    sheet.lib_symbols.clear();
    let mut engine = Engine::new(sheet).unwrap();
    // Pin "2" lands on the wire end (12.54, 10); no old pin occupied it.
    let fresh = lib_symbol("new", vec![lib_pin("1", -2.54), lib_pin("2", 2.54)]);
    match engine
        .execute(Command::ReplaceLibSymbol {
            only: None,
            lib_symbol: fresh,
            library_versions: vec![],
        })
        .unwrap_err()
    {
        EngineError::LibSymbolPinsConnected { pins, .. } => {
            assert_eq!(pins, vec!["U1 pin 2".to_string()]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert!(engine.document().lib_symbols.is_empty());
}

#[test]
fn a_replace_with_no_embedded_copy_succeeds_when_no_fresh_pin_is_connected() {
    let mut sheet = engine_with_bare_symbol().document().clone();
    sheet.lib_symbols.clear();
    let mut engine = Engine::new(sheet).unwrap();
    let fresh = lib_symbol("new", vec![lib_pin("1", -2.54), lib_pin("2", 2.54)]);
    assert!(
        engine
            .execute(Command::ReplaceLibSymbol {
                only: None,
                lib_symbol: fresh,
                library_versions: vec![],
            })
            .unwrap()
            .changed
    );
    assert_eq!(drawn(&engine), "new");
}

#[test]
fn replace_stamps_library_versions_in_the_same_undo_step() {
    let mut engine = engine_with_wired_pin_2();
    let uuid = engine.document().symbols[0].uuid;
    let fresh = lib_symbol("new", vec![lib_pin("1", -2.54), lib_pin("2", 2.54)]);
    engine
        .execute(Command::ReplaceLibSymbol {
            only: None,
            lib_symbol: fresh,
            library_versions: vec![(uuid, LinkStamp::version("2.0.0"))],
        })
        .unwrap();
    assert_eq!(engine.document().symbols[0].library_version, "2.0.0");
    engine.undo().unwrap();
    assert_eq!(engine.document().symbols[0].library_version, "");
    assert_eq!(drawn(&engine), "old");
    assert!(!engine.can_undo());
}

#[test]
fn only_instances_of_the_replaced_lib_id_are_stamped() {
    let mut engine = engine_with_wired_pin_2();
    let mut doc = engine.document().clone();
    let mut other = doc.symbols[0].clone();
    other.uuid = uuid::Uuid::now_v7();
    other.lib_id = "L:other".into();
    other.reference = "U2".into();
    other.position = Point::new(50.0, 50.0);
    let (mine, theirs) = (doc.symbols[0].uuid, other.uuid);
    doc.symbols.push(other);
    engine.set_document(doc);
    let fresh = lib_symbol("new", vec![lib_pin("1", -2.54), lib_pin("2", 2.54)]);
    engine
        .execute(Command::ReplaceLibSymbol {
            only: None,
            lib_symbol: fresh,
            library_versions: vec![
                (mine, LinkStamp::version("2.0.0")),
                (theirs, LinkStamp::version("2.0.0")),
            ],
        })
        .unwrap();
    assert_eq!(engine.document().symbols[0].library_version, "2.0.0");
    assert_eq!(engine.document().symbols[1].library_version, "");
}

#[test]
fn refusal_leaves_the_version_untouched() {
    let mut engine = engine_with_wired_pin_2();
    let uuid = engine.document().symbols[0].uuid;
    let fresh = lib_symbol("new", vec![lib_pin("1", -2.54)]);
    let err = engine.execute(Command::ReplaceLibSymbol {
        only: None,
        lib_symbol: fresh,
        library_versions: vec![(uuid, LinkStamp::version("2.0.0"))],
    });
    assert!(err.is_err());
    assert_eq!(engine.document().symbols[0].library_version, "");
    assert_eq!(drawn(&engine), "old");
}

/// A pin of `unit` numbered "1" at local x = 2.54, the tip the wired helpers
/// below sit on.
fn unit_pin(unit: u32) -> LibPin {
    LibPin {
        unit,
        ..lib_pin("1", 2.54)
    }
}

/// U1 (unit 1) at (10, 10) with a wire on world (12.54, 10); the part's only
/// pin is shared by every unit (unit 0).
fn engine_with_shared_pin() -> Engine {
    let mut sheet = test_sheet();
    sheet
        .lib_symbols
        .insert(ID.into(), lib_symbol("old", vec![unit_pin(0)]));
    sheet.symbols.push(Symbol {
        uuid: uuid::Uuid::now_v7(),
        lib_id: ID.into(),
        reference: "U1".into(),
        position: Point::new(10.0, 10.0),
        unit: 1,
        ..Default::default()
    });
    sheet.wires.push(Wire {
        start: Point::new(12.54, 10.0),
        end: Point::new(20.0, 10.0),
        ..Default::default()
    });
    Engine::new(sheet).unwrap()
}

fn replace(engine: &mut Engine, pins: Vec<LibPin>) -> Result<bool, EngineError> {
    engine
        .execute(Command::ReplaceLibSymbol {
            only: None,
            lib_symbol: lib_symbol("new", pins),
            library_versions: vec![],
        })
        .map(|r| r.changed)
}

#[test]
fn a_pin_relabelled_unit_0_to_1_at_the_same_tip_is_accepted() {
    let mut engine = engine_with_shared_pin();
    assert!(replace(&mut engine, vec![unit_pin(1)]).unwrap());
}

#[test]
fn the_same_relabel_is_refused_when_unit_2s_instance_has_that_tip_wired() {
    let mut engine = engine_with_shared_pin();
    // U2 is unit 2 of the same part, its tip (world 32.54, 10) wired too.
    let mut doc = engine.document().clone();
    doc.symbols.push(Symbol {
        uuid: uuid::Uuid::now_v7(),
        lib_id: ID.into(),
        reference: "U2".into(),
        position: Point::new(30.0, 10.0),
        unit: 2,
        ..Default::default()
    });
    doc.wires.push(Wire {
        start: Point::new(32.54, 10.0),
        end: Point::new(40.0, 10.0),
        ..Default::default()
    });
    engine.set_document(doc);
    // Unit 2 keeps a different, unwired pin so the swap is not refused as
    // removing the unit; only the relabelled shared pin is at stake.
    let unit_2_pin = LibPin {
        unit: 2,
        ..lib_pin("2", -2.54)
    };
    match replace(&mut engine, vec![unit_pin(1), unit_2_pin]).unwrap_err() {
        EngineError::LibSymbolPinsConnected { pins, .. } => {
            assert_eq!(pins, vec!["U2 pin 1".to_string()]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn undo_snapshots_share_lib_symbols_and_a_replace_does_not_bleed_into_them() {
    let mut engine = engine_with_wired_pin_2();
    let held = engine.document().lib_symbols.clone();

    // An edit that leaves the table alone: live document and the history
    // snapshot still share one allocation.
    engine
        .execute(Command::PlaceNoConnect {
            no_connect: signex_types::schematic::NoConnect {
                uuid: uuid::Uuid::now_v7(),
                position: Point::new(0.0, 0.0),
            },
        })
        .unwrap();
    assert!(engine.document().lib_symbols.shares_storage_with(&held));

    // Mutating the table copies it on write: the held copy and the undo
    // snapshot keep the old description.
    engine
        .execute(Command::ReplaceLibSymbol {
            only: None,
            lib_symbol: lib_symbol("new", vec![lib_pin("1", -2.54), lib_pin("2", 2.54)]),
            library_versions: vec![],
        })
        .unwrap();
    assert_eq!(held.values().next().unwrap().description, "old");
    assert_eq!(drawn(&engine), "new");

    engine.undo().unwrap();
    assert_eq!(drawn(&engine), "old");
    assert!(engine.document().lib_symbols.shares_storage_with(&held));
    engine.redo().unwrap();
    assert_eq!(drawn(&engine), "new");
    assert_eq!(held.values().next().unwrap().description, "old");
}

/// The stamp alone moved: the copy is the same, the version is the same, the
/// part's last-agreed values are not (#2004).
#[test]
fn a_stamp_only_replace_records_an_undo_step_and_undo_restores_the_old_stamp() {
    let mut engine = engine_with_wired_pin_2();
    let mut doc = engine.document().clone();
    let (uuid, row) = (doc.symbols[0].uuid, uuid::Uuid::from_u128(9));
    doc.symbols[0].row_id = Some(row);
    doc.symbols[0].library_version = "1.0.0".into();
    let old = RowStamp {
        mpn: Some("OLD".into()),
        version: Some("1.0.0".into()),
        ..RowStamp::new(row)
    };
    doc.symbols[0].library_stamp = Some(old.clone());
    engine.set_document(doc);
    let same_copy = lib_symbol("old", vec![lib_pin("1", -2.54), lib_pin("2", 2.54)]);
    let result = engine
        .execute(Command::ReplaceLibSymbol {
            only: None,
            lib_symbol: same_copy,
            library_versions: vec![(
                uuid,
                LinkStamp {
                    version: "1.0.0".into(),
                    values: StampValues {
                        mpn: Some("NEW".into()),
                        ..Default::default()
                    },
                },
            )],
        })
        .unwrap();
    assert!(result.changed, "a stamp-only change is a change");
    let stamp = engine.document().symbols[0].library_stamp.clone().unwrap();
    assert_eq!(stamp.mpn.as_deref(), Some("NEW"));
    assert!(engine.can_undo());
    engine.undo().unwrap();
    assert_eq!(engine.document().symbols[0].library_stamp, Some(old));
}

/// Two parts of one library symbol: R1 at (10, 10), R2 at (40, 10).
fn engine_with_two_parts() -> Engine {
    let mut sheet = engine_with_bare_symbol().document().clone();
    let mut second = sheet.symbols[0].clone();
    second.uuid = uuid::Uuid::now_v7();
    second.reference = "U2".into();
    second.position = Point::new(40.0, 10.0);
    sheet.symbols.push(second);
    Engine::new(sheet).unwrap()
}

fn drawn_by(engine: &Engine, index: usize) -> String {
    let doc = engine.document();
    doc.body_of(&doc.symbols[index])
        .expect("body")
        .description
        .clone()
}

fn replace_cmd(only: Option<Vec<uuid::Uuid>>, description: &str) -> Command {
    Command::ReplaceLibSymbol {
        only,
        lib_symbol: lib_symbol(description, vec![lib_pin("1", -2.54), lib_pin("2", 2.54)]),
        library_versions: vec![],
    }
}

#[test]
fn registering_a_changed_body_adds_a_version_and_the_old_instance_keeps_its_drawing() {
    let mut engine = engine_with_bare_symbol();
    let newer = lib_symbol("new", vec![lib_pin("1", -2.54), lib_pin("2", 2.54)]);
    let new_ref = newer.body_ref();
    engine
        .execute_batch(vec![
            Command::RegisterLibSymbol {
                lib_symbol: newer.clone(),
            },
            Command::PlaceSymbol {
                symbol: Symbol {
                    uuid: uuid::Uuid::now_v7(),
                    lib_id: ID.into(),
                    body_ref: new_ref.clone(),
                    reference: "U2".into(),
                    position: Point::new(40.0, 10.0),
                    unit: 1,
                    ..Default::default()
                },
            },
        ])
        .unwrap();
    assert_eq!(engine.document().lib_symbols.len(), 2);
    assert_eq!(drawn_by(&engine, 0), "old");
    assert_eq!(drawn_by(&engine, 1), "new");
    engine.undo().unwrap();
    assert_eq!(engine.document().lib_symbols.len(), 1);
    assert_eq!(drawn_by(&engine, 0), "old");
}

#[test]
fn registering_a_different_body_under_a_taken_ref_is_a_collision() {
    let mut engine = engine_with_bare_symbol();
    let newer = lib_symbol("new", vec![]);
    let mut doc = engine.document().clone();
    doc.lib_symbols
        .insert(newer.body_ref(), lib_symbol("other", vec![]));
    engine = Engine::new(doc).unwrap();
    let err = engine
        .execute(Command::RegisterLibSymbol { lib_symbol: newer })
        .expect_err("collision");
    assert!(
        matches!(err, EngineError::LibSymbolBodyCollision(_)),
        "{err}"
    );
}

#[test]
fn update_all_moves_every_instance_drops_the_old_version_and_one_undo_restores_both() {
    let mut engine = engine_with_two_parts();
    engine.execute(replace_cmd(None, "new")).unwrap();
    assert_eq!(
        (drawn_by(&engine, 0), drawn_by(&engine, 1)),
        ("new".into(), "new".into())
    );
    assert_eq!(
        engine.document().lib_symbols.len(),
        1,
        "the old version is dropped"
    );
    engine.undo().unwrap();
    assert_eq!(
        (drawn_by(&engine, 0), drawn_by(&engine, 1)),
        ("old".into(), "old".into())
    );
    assert_eq!(engine.document().lib_symbols.len(), 1);
    assert!(!engine.can_undo());
}

#[test]
fn update_only_this_one_leaves_the_other_part_on_the_old_version() {
    let mut engine = engine_with_two_parts();
    let first = engine.document().symbols[0].uuid;
    engine
        .execute(replace_cmd(Some(vec![first]), "new"))
        .unwrap();
    assert_eq!(
        (drawn_by(&engine, 0), drawn_by(&engine, 1)),
        ("new".into(), "old".into())
    );
    assert_eq!(
        engine.document().lib_symbols.len(),
        2,
        "both versions coexist"
    );
    engine.undo().unwrap();
    assert_eq!(
        (drawn_by(&engine, 0), drawn_by(&engine, 1)),
        ("old".into(), "old".into())
    );
}

#[test]
fn update_refuses_when_a_connected_pin_would_be_lost_and_changes_nothing() {
    let mut engine = engine_with_wired_pin_2();
    let err = drop_pin_2(&mut engine).expect_err("refused");
    assert!(
        matches!(err, EngineError::LibSymbolPinsConnected { .. }),
        "{err}"
    );
    assert_eq!(drawn(&engine), "old");
    assert!(!engine.can_undo());
}
