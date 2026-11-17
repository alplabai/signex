//! Directives (#2420) persist in `[extras.sheet]` as `directives`, a list of
//! tables tagged by `type`. A sheet without any writes nothing, so a file
//! from a build without the field re-saves byte-identical.

use super::tests::empty_sheet;
use super::*;
use crate::schematic::{Point, SchDirective, SchematicSheet};

fn load(text: &str) -> SchematicSheet {
    SnxSchematic::parse(text).unwrap().sheet
}

fn save(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

fn all_three() -> Vec<SchDirective> {
    vec![
        SchDirective::Blanket {
            uuid: Uuid::from_u128(0x0192_a8c0_0004_7000_8000_0000_0000_0001),
            start: Point::new(10.16, 12.7),
            end: Point::new(50.8, 38.1),
            net_class: Some("HighSpeed".to_string()),
            erc_exclusions: vec![
                "builtin::unused_pin".to_string(),
                "builtin::orphan_label".to_string(),
            ],
        },
        SchDirective::CompileMask {
            uuid: Uuid::from_u128(0x0192_a8c0_0004_7000_8000_0000_0000_0002),
            start: Point::new(60.96, 12.7),
            end: Point::new(76.2, 25.4),
        },
        SchDirective::DiffPair {
            uuid: Uuid::from_u128(0x0192_a8c0_0004_7000_8000_0000_0000_0003),
            position: Point::new(30.48, 50.8),
        },
    ]
}

#[test]
fn every_directive_kind_round_trips_and_a_second_save_is_identical() {
    let mut sheet = empty_sheet();
    sheet.directives = all_three();

    let text = save(&sheet);
    assert!(text.contains("[[extras.sheet.directives]]"), "{text}");
    assert!(text.contains("type = \"blanket\""), "{text}");
    assert!(text.contains("type = \"compile_mask\""), "{text}");
    assert!(text.contains("type = \"diff_pair\""), "{text}");
    let again = load(&text);
    assert_eq!(again.directives, sheet.directives);
    assert_eq!(save(&again), text);
}

#[test]
fn a_blanket_without_parameters_writes_only_its_region() {
    let mut sheet = empty_sheet();
    sheet.directives = vec![SchDirective::Blanket {
        uuid: Uuid::from_u128(7),
        start: Point::new(0.0, 0.0),
        end: Point::new(2.54, 2.54),
        net_class: None,
        erc_exclusions: Vec::new(),
    }];

    let text = save(&sheet);
    assert!(!text.contains("net_class"), "{text}");
    assert!(!text.contains("erc_exclusions"), "{text}");
    assert_eq!(load(&text).directives, sheet.directives);
}

#[test]
fn a_sheet_without_directives_writes_nothing_and_loads_none() {
    let text = save(&empty_sheet());
    assert!(!text.contains("directives"), "{text}");
    let sheet = load(&text);
    assert!(sheet.directives.is_empty());
    assert_eq!(save(&sheet), text);
}

#[test]
fn a_directive_sharing_an_id_with_another_object_is_reminted_on_load() {
    let mut sheet = empty_sheet();
    let mut directives = all_three();
    let taken = directives[0].uuid();
    directives[1].set_uuid(taken);
    sheet.directives = directives;

    let loaded = load(&save(&sheet));
    assert_eq!(loaded.directives.len(), 3);
    assert_eq!(loaded.directives[0].uuid(), taken);
    assert_ne!(loaded.directives[1].uuid(), taken);
}
