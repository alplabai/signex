//! No-ERC directives (#1699) persist in `[extras.sheet]` as
//! `no_erc_directives`. The rule list is written only when it names rules,
//! so a marker without one saves exactly as before the list existed and
//! loads as "every rule".

use super::tests::empty_sheet;
use super::*;
use crate::schematic::{NoErcDirective, Point, SchematicSheet};

fn load(text: &str) -> SchematicSheet {
    SnxSchematic::parse(text).unwrap().sheet
}

fn save(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

#[test]
fn a_marker_without_rules_writes_no_list_and_resaves_identical() {
    let mut sheet = empty_sheet();
    let marker = NoErcDirective::new(
        Uuid::from_u128(0x0192_a8c0_0005_7000_8000_0000_0000_0001),
        Point::new(10.16, 12.7),
    );
    sheet.no_erc_directives = vec![marker];

    let text = save(&sheet);
    assert!(text.contains("no_erc_directives"), "{text}");
    assert!(!text.contains("rules = "), "{text}");
    let again = load(&text);
    assert!(again.no_erc_directives[0].rules.is_empty());
    assert!(again.no_erc_directives[0].suppresses("builtin::unused_pin"));
    assert_eq!(save(&again), text);
}

#[test]
fn a_rule_list_round_trips_and_a_second_save_is_identical() {
    let mut sheet = empty_sheet();
    let mut marker = NoErcDirective::new(
        Uuid::from_u128(0x0192_a8c0_0005_7000_8000_0000_0000_0002),
        Point::new(30.48, 50.8),
    );
    marker.rules = vec![
        "builtin::unused_pin".to_string(),
        "builtin::pin_type_conflict".to_string(),
    ];
    sheet.no_erc_directives = vec![marker];

    let text = save(&sheet);
    assert!(text.contains("builtin::pin_type_conflict"), "{text}");
    let again = load(&text);
    assert_eq!(again.no_erc_directives, sheet.no_erc_directives);
    assert_eq!(save(&again), text);
}
