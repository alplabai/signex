//! #2951: a differential pair half whose partner net is missing is reported.

use signex_types::schematic::{
    Label, LabelType, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchDirective,
    SchematicSheet, SelectedKind, Symbol, Wire,
};
use uuid::Uuid;

use crate::context::ErcContext;
use crate::waivers::violation::{new_waiver, split_waived, waiver_key};
use crate::{RuleKind, Severity, Violation, engine};

fn found(sheet: &SchematicSheet) -> Vec<crate::Diagnostic> {
    engine::run_all(&ErcContext::from_snapshot(sheet))
        .into_iter()
        .filter(|d| d.rule_kind == RuleKind::DiffPairPartnerMissing)
        .collect()
}

/// The one-pin test point's library body.
fn test_point_body() -> LibSymbol {
    LibSymbol {
        id: "TP".into(),
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
        pins: vec![LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: Point::new(0.0, 0.0),
                rotation: 0.0,
                length: 0.0,
                name: String::new(),
                number: "1".into(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        }],
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

/// A one-pin part at `at`, so the wire under it forms a net.
fn test_point(sheet: &mut SchematicSheet, at: Point) {
    sheet
        .lib_symbols
        .entry("TP".into())
        .or_insert_with(test_point_body);
    let n = sheet.symbols.len() + 1;
    sheet.symbols.push(Symbol {
        uuid: Uuid::from_u128(200 + n as u128),
        lib_id: "TP".into(),
        reference: format!("TP{n}"),
        position: at,
        ..Default::default()
    });
}

/// One net per name: a wire from (0, 10·i) to (10, 10·i) with a test point
/// at each end and its label (uuid `100 + i`) at the left end.
fn sheet(names: &[&str]) -> SchematicSheet {
    let mut sheet = SchematicSheet {
        paper_size: "A4".into(),
        ..Default::default()
    };
    for (i, name) in names.iter().enumerate() {
        let y = 10.0 * i as f64;
        sheet.wires.push(Wire {
            uuid: Uuid::from_u128(1 + i as u128),
            start: Point::new(0.0, y),
            end: Point::new(10.0, y),
            ..Default::default()
        });
        sheet.labels.push(Label {
            uuid: Uuid::from_u128(100 + i as u128),
            ..Label::new(*name, Point::new(0.0, y), LabelType::Net)
        });
        test_point(&mut sheet, Point::new(0.0, y));
        test_point(&mut sheet, Point::new(10.0, y));
    }
    sheet
}

#[test]
fn a_positive_net_with_no_partner_is_one_warning_at_its_label() {
    let hits = found(&sheet(&["USB_D_P", "OTHER"]));
    assert_eq!(hits.len(), 1, "{hits:?}");
    let d = &hits[0];
    assert_eq!(d.severity, Severity::Warning);
    assert_eq!(d.rule_id.as_str(), "builtin::diff_pair_partner_missing");
    assert_eq!(
        d.message,
        "Net USB_D_P is named as the positive half of differential pair USB_D, but its \
         partner USB_D_N is not on this sheet"
    );
    assert_eq!(d.location, Point::new(0.0, 0.0));
    let primary = d.primary.expect("primary");
    assert_eq!(
        (primary.uuid, primary.kind),
        (Uuid::from_u128(100), SelectedKind::Label)
    );
    assert_eq!(d.tag.as_deref(), Some("pair:USB_D"));
}

#[test]
fn every_suffix_style_is_reported_alone_and_quiet_as_a_pair() {
    for (p, n) in [
        ("USB_D_P", "USB_D_N"),
        ("USB_DP", "USB_DN"),
        ("CLK+", "CLK-"),
    ] {
        assert_eq!(found(&sheet(&[p])).len(), 1, "{p} alone");
        assert!(found(&sheet(&[p, n])).is_empty(), "{p} with {n}");
    }
}

#[test]
fn a_directive_on_a_lone_negative_net_is_reported() {
    let mut s = sheet(&["USB_D_N"]);
    s.directives.push(SchDirective::DiffPair {
        uuid: Uuid::from_u128(90),
        position: Point::new(5.0, 0.0),
        class: None,
    });
    let hits = found(&s);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(
        hits[0].message,
        "Net USB_D_N is in differential pair USB_D by a directive, but its partner USB_D_P \
         is not on this sheet"
    );
    assert!(
        found(&sheet(&["USB_D_N"])).is_empty(),
        "by name alone: quiet"
    );
}

#[test]
fn a_blanket_that_excludes_the_rule_silences_it() {
    let mut s = sheet(&["USB_D_P"]);
    s.directives.push(SchDirective::Blanket {
        uuid: Uuid::from_u128(91),
        start: Point::new(-1.0, -1.0),
        end: Point::new(11.0, 1.0),
        net_class: None,
        erc_exclusions: vec!["builtin::diff_pair_partner_missing".into()],
        track_width: None,
        pour: Default::default(),
        include_edge_nets: false,
    });
    assert!(found(&s).is_empty());
}

/// The finding names its label, so it is waivable, and the waiver lapses
/// when the label is renamed.
#[test]
fn the_finding_is_waivable_and_the_waiver_lapses_with_the_label() {
    let s = sheet(&["USB_D_P"]);
    let v = Violation::from(found(&s).remove(0));
    let key = waiver_key(&v, &s).expect("waivable");
    let waiver = new_waiver(
        &key,
        &v.message,
        "single-ended here",
        "A. Designer",
        "2026-11-25",
    )
    .expect("valid");
    let mut kept = v.clone();
    kept.waiver_key = Some(key);
    let split = split_waived(vec![kept], std::slice::from_ref(&waiver));
    assert_eq!(split.waived.len(), 1);

    let mut renamed = s.clone();
    renamed.labels[0].text = "USB_D_P2".into();
    let mut moved = v;
    moved.waiver_key = waiver_key(&moved, &renamed);
    let split = split_waived(vec![moved], &[waiver]);
    assert_eq!(split.kept.len(), 1, "the finding is back");
    assert!(split.lapsed.contains(&0));
}

#[test]
fn the_rule_has_a_fixture_and_its_severity() {
    let ctx = crate::fixtures::context_for(RuleKind::DiffPairPartnerMissing);
    let hits: Vec<_> = engine::run_all(&ctx)
        .into_iter()
        .filter(|d| d.rule_kind == RuleKind::DiffPairPartnerMissing)
        .collect();
    assert_eq!(hits.len(), 1);
    assert_eq!(
        RuleKind::DiffPairPartnerMissing.default_severity(),
        Severity::Warning
    );
}
