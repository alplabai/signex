//! No-ERC directives (#1699) persist in `[extras.sheet]` as
//! `no_erc_directives`. The rule list is written only when it names rules,
//! so a marker without one saves exactly as before the list existed and
//! loads as "every rule". A sheet with a list writes `snxsch/1.2`, which a
//! 1.1 build opens read-only instead of dropping the list on its next save.

use super::tests::empty_sheet;
use super::*;
use crate::schematic::{NoErcDirective, Point, SchematicSheet};

fn token(text: &str) -> &str {
    text.lines()
        .next()
        .and_then(|line| line.strip_prefix("format = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or_default()
}

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
    assert_eq!(token(&text), SNXSCH_FORMAT_V1, "{text}");
    let file = SnxSchematic::parse(&text).unwrap();
    assert_eq!(file.format, SNXSCH_FORMAT_V1);
    assert!(!file.is_from_newer_minor());
    let again = file.sheet;
    assert!(again.no_erc_directives[0].rules.is_empty());
    assert!(again.no_erc_directives[0].suppresses("builtin::unused_pin"));
    assert_eq!(save(&again), text);
}

#[test]
fn a_rule_list_writes_1_2_which_a_1_1_build_opens_read_only() {
    let mut sheet = empty_sheet();
    let mut marker = NoErcDirective::new(
        Uuid::from_u128(0x0192_a8c0_0005_7000_8000_0000_0000_0003),
        Point::new(5.08, 7.62),
    );
    marker.rules = vec!["builtin::unused_pin".to_string()];
    sheet.no_erc_directives = vec![marker];

    let text = save(&sheet);
    assert_eq!(token(&text), SNXSCH_FORMAT_NO_ERC_RULES, "{text}");
    assert_eq!(SNXSCH_FORMAT_NO_ERC_RULES, "snxsch/1.2");
    let file = SnxSchematic::parse(&text).unwrap();
    assert_eq!(file.format, SNXSCH_FORMAT_NO_ERC_RULES);
    assert!(!file.is_from_newer_minor(), "this build edits 1.2");
    assert_eq!(file.sheet.no_erc_directives, sheet.no_erc_directives);
    assert_eq!(save(&file.sheet), text);

    // The 1.1 policy this build replaced: 1.2 is a newer minor there, so it
    // loads for viewing and every save is refused.
    let one_one = FormatVersion {
        current: SNXSCH_FORMAT_V1,
        readable: &["snxsch/1", SNXSCH_FORMAT_V1],
        ..SNXSCH_VERSION
    };
    assert_eq!(
        one_one.read(Some(SNXSCH_FORMAT_NO_ERC_RULES)).as_deref(),
        Ok(SNXSCH_FORMAT_NO_ERC_RULES)
    );
    assert!(one_one.is_newer_minor(SNXSCH_FORMAT_NO_ERC_RULES));
    assert!(one_one.refuse_save(SNXSCH_FORMAT_NO_ERC_RULES).is_err());

    // Clearing the last list takes the sheet back to 1.1.
    let mut cleared = file.sheet;
    cleared.no_erc_directives[0].rules.clear();
    assert_eq!(token(&save(&cleared)), SNXSCH_FORMAT_V1);
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
