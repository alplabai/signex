//! Directives (#2420, #1281) persist in `[extras.sheet]` as `directives`, a
//! list of tables tagged by `type`. A sheet without any writes nothing, so a
//! file from a build without the field re-saves byte-identical.

use super::tests::empty_sheet;
use super::*;
use crate::schematic::{BlanketPour, NetRules, Point, SchDirective, SchematicSheet};

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
            track_width: None,
            pour: BlanketPour::Normal,
            include_edge_nets: false,
        },
        SchDirective::CompileMask {
            uuid: Uuid::from_u128(0x0192_a8c0_0004_7000_8000_0000_0000_0002),
            start: Point::new(60.96, 12.7),
            end: Point::new(76.2, 25.4),
        },
        SchDirective::DiffPair {
            uuid: Uuid::from_u128(0x0192_a8c0_0004_7000_8000_0000_0000_0003),
            position: Point::new(30.48, 50.8),
            class: None,
        },
    ]
}

/// Every field added with the Net Parameters directive (#1281), set.
fn every_new_field() -> Vec<SchDirective> {
    vec![
        SchDirective::Blanket {
            uuid: Uuid::from_u128(0x0192_a8c0_0004_7000_8000_0000_0000_0011),
            start: Point::new(139.7, 15.24),
            end: Point::new(190.5, 45.72),
            net_class: Some("SW_NODE".to_string()),
            erc_exclusions: Vec::new(),
            track_width: Some(1.0),
            pour: BlanketPour::KeepShort,
            include_edge_nets: true,
        },
        SchDirective::DiffPair {
            uuid: Uuid::from_u128(0x0192_a8c0_0004_7000_8000_0000_0000_0012),
            position: Point::new(30.48, 50.8),
            class: Some("USB".to_string()),
        },
        SchDirective::NetParameters {
            uuid: Uuid::from_u128(0x0192_a8c0_0004_7000_8000_0000_0000_0013),
            position: Point::new(48.26, 22.86),
            net_class: Some("POWER_IN".to_string()),
            diff_pair_class: Some("USB".to_string()),
            rules: NetRules {
                width_mm: Some(0.8),
                clearance_mm: Some(0.3),
                via_diameter_mm: Some(0.6),
                via_hole_mm: Some(0.3),
                current_a: Some(3.0),
            },
            show_on_flag: true,
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
fn every_field_added_with_net_parameters_round_trips() {
    let mut sheet = empty_sheet();
    sheet.directives = every_new_field();

    let text = save(&sheet);
    for key in [
        "type = \"net_parameters\"",
        "track_width = 1.0",
        "pour = \"keep_short\"",
        "include_edge_nets = true",
        "class = \"USB\"",
        "diff_pair_class = \"USB\"",
        "show_on_flag = true",
        "width_mm = 0.8",
        "via_diameter_mm = 0.6",
        "current_a = 3.0",
    ] {
        assert!(text.contains(key), "missing `{key}` in {text}");
    }
    let again = load(&text);
    assert_eq!(again.directives, sheet.directives);
    assert_eq!(save(&again), text);
}

#[test]
fn a_net_parameters_directive_without_parameters_writes_only_its_position() {
    let mut sheet = empty_sheet();
    sheet.directives = vec![SchDirective::NetParameters {
        uuid: Uuid::from_u128(8),
        position: Point::new(2.54, 2.54),
        net_class: None,
        diff_pair_class: None,
        rules: NetRules::default(),
        show_on_flag: false,
    }];

    let text = save(&sheet);
    for key in ["net_class", "diff_pair_class", "rules", "show_on_flag"] {
        assert!(!text.contains(key), "`{key}` written in {text}");
    }
    assert_eq!(load(&text).directives, sheet.directives);
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
        track_width: None,
        pour: BlanketPour::Normal,
        include_edge_nets: false,
    }];

    let text = save(&sheet);
    for key in [
        "net_class",
        "erc_exclusions",
        "track_width",
        "pour",
        "include_edge_nets",
    ] {
        assert!(!text.contains(key), "`{key}` written in {text}");
    }
    assert_eq!(load(&text).directives, sheet.directives);
}

#[test]
fn a_blanket_and_a_pair_saved_before_net_parameters_load_and_resave_unchanged() {
    // The sheet an older build wrote: no new key in either table.
    let mut sheet = empty_sheet();
    sheet.directives = all_three();
    let old_text = save(&sheet);
    let loaded = load(&old_text);
    let SchDirective::Blanket {
        track_width,
        pour,
        include_edge_nets,
        ..
    } = &loaded.directives[0]
    else {
        panic!("a blanket first: {:?}", loaded.directives);
    };
    assert_eq!(
        (*track_width, *pour, *include_edge_nets),
        (None, BlanketPour::Normal, false)
    );
    assert!(matches!(
        loaded.directives[2],
        SchDirective::DiffPair { class: None, .. }
    ));
    assert!(loaded.directives.iter().all(|d| !d.needs_format_1_3()));
    assert_eq!(save(&loaded), old_text);
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
