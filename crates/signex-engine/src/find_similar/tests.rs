use super::*;
use crate::test_support::test_sheet;
use signex_types::schematic::{Label, LabelType, Point, Symbol, Wire};
use uuid::Uuid;

fn part(reference: &str, value: &str, footprint: &str, lib: &str) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: lib.to_owned(),
        reference: reference.to_owned(),
        value: value.to_owned(),
        footprint: footprint.to_owned(),
        position: Point::new(0.0, 0.0),
        ..Default::default()
    }
}

fn label(text: &str, label_type: LabelType) -> Label {
    Label {
        uuid: Uuid::now_v7(),
        text: text.to_owned(),
        position: Point::new(0.0, 0.0),
        rotation: 0.0,
        label_type,
        shape: String::new(),
        font_size: 0.0,
        font_family: None,
        justify: Default::default(),
        justify_v: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn sym(s: &Symbol) -> SelectedItem {
    SelectedItem::new(s.uuid, SelectedKind::Symbol)
}

/// R1 10k 0603, R2 10k 0402, R3 4k7 0603, C1 10k-valued 0603 (a trap for
/// Value-only matches), and a wire.
fn sheet() -> (SchematicSheet, [Symbol; 4]) {
    let parts = [
        part("R1", "10k", "R_0603", "Device:R"),
        part("R2", "10k", "R_0402", "Device:R"),
        part("R3", "4k7", "R_0603", "Device:R"),
        part("C1", "10k", "C_0603", "Device:C"),
    ];
    let mut s = test_sheet();
    s.symbols = parts.to_vec();
    s.wires.push(Wire {
        uuid: Uuid::now_v7(),
        start: Point::new(0.0, 0.0),
        end: Point::new(1.0, 0.0),
        stroke_width: 0.0,
        unknown_columns: Default::default(),
    });
    (s, parts)
}

fn criteria(
    sheet: &SchematicSheet,
    seed: &SelectedItem,
    same: &[SimilarProperty],
) -> Vec<SimilarValue> {
    properties_of(sheet, seed)
        .into_iter()
        .filter(|v| same.contains(&v.property))
        .collect()
}

fn found(sheet: &SchematicSheet, seed: &Symbol, same: &[SimilarProperty]) -> Vec<Uuid> {
    let c = criteria(sheet, &sym(seed), same);
    find_similar(sheet, &c).iter().map(|i| i.uuid).collect()
}

#[test]
fn kind_alone_matches_every_object_of_that_kind() {
    let (s, p) = sheet();
    let all: Vec<Uuid> = p.iter().map(|x| x.uuid).collect();
    assert_eq!(found(&s, &p[0], &[SimilarProperty::Kind]), all);
}

#[test]
fn same_value_matches_across_designator_prefixes() {
    let (s, p) = sheet();
    let got = found(&s, &p[0], &[SimilarProperty::Kind, SimilarProperty::Value]);
    assert_eq!(got, vec![p[0].uuid, p[1].uuid, p[3].uuid]);
}

#[test]
fn same_footprint_and_library_part_and_prefix_each_narrow_the_match() {
    let (s, p) = sheet();
    assert_eq!(
        found(&s, &p[0], &[SimilarProperty::Footprint]),
        vec![p[0].uuid, p[2].uuid]
    );
    assert_eq!(
        found(&s, &p[0], &[SimilarProperty::LibraryPart]),
        vec![p[0].uuid, p[1].uuid, p[2].uuid]
    );
    assert_eq!(
        found(&s, &p[3], &[SimilarProperty::DesignatorPrefix]),
        vec![p[3].uuid]
    );
    assert_eq!(
        found(
            &s,
            &p[0],
            &[SimilarProperty::Value, SimilarProperty::Footprint]
        ),
        vec![p[0].uuid]
    );
}

#[test]
fn a_property_other_kinds_lack_never_matches_them() {
    let (s, p) = sheet();
    // Kind Any, Value Same: the wire has no Value, so it is not a match.
    let got = found(&s, &p[0], &[SimilarProperty::Value]);
    assert_eq!(got.len(), 3);
    assert!(!got.contains(&s.wires[0].uuid));
}

#[test]
fn labels_match_by_net_name_and_type() {
    let mut s = test_sheet();
    let a = label("VCC", LabelType::Net);
    let b = label("VCC", LabelType::Global);
    let c = label("GND", LabelType::Net);
    let ids = (a.uuid, b.uuid, c.uuid);
    s.labels = vec![a, b, c];
    let seed = SelectedItem::new(ids.0, SelectedKind::Label);

    let by_name = criteria(&s, &seed, &[SimilarProperty::NetName]);
    let got: Vec<Uuid> = find_similar(&s, &by_name).iter().map(|i| i.uuid).collect();
    assert_eq!(got, vec![ids.0, ids.1]);

    let by_both = criteria(
        &s,
        &seed,
        &[SimilarProperty::NetName, SimilarProperty::LabelType],
    );
    let got: Vec<Uuid> = find_similar(&s, &by_both).iter().map(|i| i.uuid).collect();
    assert_eq!(got, vec![ids.0]);
}

#[test]
fn the_ref_and_value_field_kinds_stand_for_their_symbol() {
    let (s, p) = sheet();
    let field = SelectedItem::new(p[0].uuid, SelectedKind::SymbolRefField);
    let props = properties_of(&s, &field);
    assert_eq!(props[0].value, "Symbol");
    assert!(
        props
            .iter()
            .any(|v| v.property == SimilarProperty::Value && v.value == "10k")
    );
}

#[test]
fn an_object_not_on_the_sheet_has_no_properties() {
    let (s, _) = sheet();
    for kind in [
        SelectedKind::Symbol,
        SelectedKind::Wire,
        SelectedKind::Label,
    ] {
        assert!(properties_of(&s, &SelectedItem::new(Uuid::now_v7(), kind)).is_empty());
    }
}

#[test]
fn criteria_from_another_sheet_apply_by_value() {
    let (s1, p) = sheet();
    let mut s2 = test_sheet();
    s2.symbols = vec![
        part("R9", "10k", "R_0805", "Device:R"),
        part("R8", "1k", "R_0805", "Device:R"),
    ];
    let c = criteria(
        &s1,
        &sym(&p[0]),
        &[SimilarProperty::Kind, SimilarProperty::Value],
    );
    assert_eq!(find_similar(&s2, &c).len(), 1);
}

#[test]
fn describe_names_the_designator_text_or_kind() {
    let (s, p) = sheet();
    assert_eq!(describe(&s, &sym(&p[0])), "Symbol R1");
    let wire = SelectedItem::new(s.wires[0].uuid, SelectedKind::Wire);
    assert_eq!(describe(&s, &wire), "Wire");
}
