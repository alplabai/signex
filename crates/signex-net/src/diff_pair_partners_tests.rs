//! #2951 / #2952: lone differential pair halves and the project's pair list.

use signex_types::diff_pair_class::DiffPairClass;
use signex_types::schematic::{
    LabelType, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchDirective,
    SchematicSheet, Symbol, Wire,
};
use uuid::Uuid;

use super::*;
use crate::fixtures::{empty_sheet, label, pt};

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn test_point(sheet: &mut SchematicSheet) {
    let pin = LibPin {
        unit: 0,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: pt(0.0, 0.0),
            rotation: 0.0,
            length: 0.0,
            name: String::new(),
            number: "1".to_string(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    };
    sheet.lib_symbols.insert(
        "TP".to_string(),
        LibSymbol {
            id: "TP".to_string(),
            reference: String::new(),
            value: String::new(),
            footprint: String::new(),
            datasheet: String::new(),
            description: String::new(),
            keywords: String::new(),
            fp_filters: String::new(),
            in_bom: true,
            on_board: true,
            in_pos_files: true,
            duplicate_pin_numbers_are_jumpers: false,
            graphics: Vec::new(),
            pins: vec![pin],
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.0,
        },
    );
}

/// One net per name: a wire from (0, 10·i) to (10, 10·i), its label at the
/// left end with uuid `100 + i`, and a test point at each end.
fn nets(names: &[&str]) -> SchematicSheet {
    let mut sheet = empty_sheet();
    test_point(&mut sheet);
    for (i, name) in names.iter().enumerate() {
        let y = 10.0 * i as f64;
        sheet.wires.push(Wire {
            uuid: id(1 + i as u128),
            start: pt(0.0, y),
            end: pt(10.0, y),
            ..Default::default()
        });
        sheet.labels.push(signex_types::schematic::Label {
            uuid: id(100 + i as u128),
            ..label(name, pt(0.0, y), LabelType::Net)
        });
        for x in [0.0, 10.0] {
            sheet.symbols.push(Symbol {
                uuid: Uuid::now_v7(),
                lib_id: "TP".to_string(),
                reference: format!("TP{}", sheet.symbols.len() + 1),
                position: pt(x, y),
                ..Default::default()
            });
        }
    }
    sheet
}

fn pair_directive(n: u128, at: Point, class: Option<&str>) -> SchDirective {
    SchDirective::DiffPair {
        uuid: id(n),
        position: at,
        class: class.map(str::to_string),
    }
}

fn lone_nets(sheet: &SchematicSheet) -> Vec<(String, String)> {
    lone_pair_members(sheet)
        .into_iter()
        .map(|m| (m.net, m.partner))
        .collect()
}

#[test]
fn a_positive_half_without_its_partner_is_reported_in_every_suffix_style() {
    for (net, partner, pair) in [
        ("USB_D_P", "USB_D_N", "USB_D"),
        ("USB_DP", "USB_DN", "USB_D"),
        ("CLK+", "CLK-", "CLK"),
    ] {
        let sheet = nets(&[net, "OTHER"]);
        let found = lone_pair_members(&sheet);
        assert_eq!(found.len(), 1, "{net}: {found:?}");
        let m = &found[0];
        assert_eq!((m.net.as_str(), m.partner.as_str()), (net, partner));
        assert_eq!(m.pair, pair);
        assert_eq!(m.directive, None, "found by name");
        assert_eq!(m.label, Some((id(100), pt(0.0, 0.0))));
    }
}

#[test]
fn a_complete_pair_is_not_reported_in_any_suffix_style() {
    for (p, n) in [
        ("USB_D_P", "USB_D_N"),
        ("USB_DP", "USB_DN"),
        ("CLK+", "CLK-"),
    ] {
        assert!(lone_nets(&nets(&[p, n])).is_empty(), "{p}/{n}");
    }
}

/// A negative name alone is an active-low signal far more often than half
/// a pair: by name it is never reported.
#[test]
fn a_negative_half_alone_is_not_reported_by_name() {
    assert!(lone_nets(&nets(&["RESET_N", "PWRDN", "AIN-"])).is_empty());
}

#[test]
fn a_directive_on_a_lone_negative_half_is_reported_at_the_directive() {
    let mut sheet = nets(&["USB_D_N", "OTHER"]);
    sheet
        .directives
        .push(pair_directive(90, pt(5.0, 0.0), None));
    let found = lone_pair_members(&sheet);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(
        (found[0].net.as_str(), found[0].partner.as_str()),
        ("USB_D_N", "USB_D_P")
    );
    assert_eq!(found[0].directive, Some((id(90), pt(5.0, 0.0))));
}

/// A directive on a lone positive half is one finding, the directive's.
#[test]
fn a_net_a_directive_and_its_name_both_make_lone_is_reported_once() {
    let mut sheet = nets(&["USB_D_P"]);
    sheet
        .directives
        .push(pair_directive(90, pt(5.0, 0.0), None));
    let found = lone_pair_members(&sheet);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].directive.is_some());
}

#[test]
fn a_power_net_is_never_a_pair_half_by_name() {
    let mut sheet = nets(&["V+"]);
    sheet.labels[0].label_type = LabelType::Power;
    assert!(lone_nets(&sheet).is_empty());
}

#[test]
fn a_sheet_with_no_pair_names_builds_no_netlist_and_finds_nothing() {
    let sheet = nets(&["A", "B"]);
    assert!(!may_have_pairs(&sheet));
    assert!(lone_pair_members(&sheet).is_empty());
}

/// A listed pair as compared: name, P net, N net, class, how found, sheet.
type Row<'a> = (
    &'a str,
    Option<&'a str>,
    Option<&'a str>,
    Option<&'a str>,
    PairFoundBy,
    &'a str,
);

#[test]
fn the_list_has_directive_pairs_then_pairs_found_by_name_on_each_sheet() {
    let mut a = nets(&["USB_D_P", "USB_D_N", "CLK+", "CLK-"]);
    a.directives
        .push(pair_directive(90, pt(5.0, 0.0), Some("USB")));
    let b = nets(&["CLK+", "CLK-", "LVDS_DP", "LVDS_DN", "TX_P"]);
    let classes = [DiffPairClass::named("USB")];
    let sheets = [("a.snxsch".to_string(), &a), ("b.snxsch".to_string(), &b)];
    let listed = listed_pairs(&sheets, &classes);
    let rows: Vec<Row> = listed
        .iter()
        .map(|p| {
            (
                p.name.as_str(),
                p.positive.as_deref(),
                p.negative.as_deref(),
                p.class.as_deref(),
                p.found_by,
                p.sheet.as_str(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            (
                "USB_D",
                Some("USB_D_P"),
                Some("USB_D_N"),
                Some("USB"),
                PairFoundBy::Directive(id(90)),
                "a.snxsch"
            ),
            (
                "CLK",
                Some("CLK+"),
                Some("CLK-"),
                None,
                PairFoundBy::Suffix,
                "a.snxsch"
            ),
            (
                "LVDS_D",
                Some("LVDS_DP"),
                Some("LVDS_DN"),
                None,
                PairFoundBy::Suffix,
                "b.snxsch"
            ),
        ],
        "CLK on b is the same two nets, listed once; a lone TX_P is no pair"
    );
}

#[test]
fn an_incomplete_directive_pair_is_listed_with_its_missing_half_empty() {
    let mut sheet = nets(&["USB_D_N"]);
    sheet
        .directives
        .push(pair_directive(90, pt(5.0, 0.0), None));
    let listed = listed_pairs(&[("a.snxsch".to_string(), &sheet)], &[]);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].positive, None);
    assert_eq!(listed[0].negative.as_deref(), Some("USB_D_N"));
}
