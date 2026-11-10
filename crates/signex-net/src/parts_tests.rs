use signex_types::schematic::Symbol;

use super::*;
use crate::fixtures::empty_sheet;

/// The private `parts_of` that `link_by_designator` used before
/// `design_parts` replaced it, kept verbatim as the oracle.
fn old_parts_of(
    sheets: &[(InstancePath, &SchematicSheet)],
) -> Vec<(InstancePath, String, Vec<Uuid>)> {
    let mut parts = Vec::new();
    for (path, sheet) in sheets {
        let placed: Vec<_> = sheet.symbols.iter().filter(|s| !s.is_power).collect();
        let keys: Vec<PartKey<'_>> = placed
            .iter()
            .map(|s| PartKey {
                reference: s.designator_in(path).trim(),
                ..s.part_key()
            })
            .collect();
        for group in group_into_parts(keys.iter().copied()) {
            let designator = keys[group[0]].reference;
            if designator.is_empty() || designator.contains('?') {
                continue;
            }
            parts.push((
                path.clone(),
                designator.to_string(),
                group.iter().map(|&i| placed[i].uuid).collect(),
            ));
        }
    }
    parts
}

fn symbol(reference: &str, lib_id: &str, unit: u32) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: lib_id.to_string(),
        reference: reference.to_string(),
        unit,
        ..Default::default()
    }
}

fn sheet(symbols: Vec<Symbol>) -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet.symbols = symbols;
    sheet
}

fn fixture() -> (SchematicSheet, SchematicSheet) {
    let mut gnd = symbol("#PWR01", "GND", 1);
    gnd.is_power = true;
    let root = sheet(vec![
        symbol(" R1 ", "R", 1),
        symbol("U1", "OPAMP", 2),
        symbol("U1", "OPAMP", 1),
        symbol("R?", "R", 1),
        symbol("", "C", 1),
        gnd,
        // Unit 1 twice under one designator: two parts.
        symbol("R5", "R", 1),
        symbol("R5", "R", 1),
    ]);
    let child = sheet(vec![symbol("R1", "R", 1), symbol("U?", "OPAMP", 1)]);
    (root, child)
}

#[test]
fn design_parts_matches_the_old_parts_of_on_its_annotated_parts() {
    let (root, child) = fixture();
    let child_path = InstancePath::from_chain(vec![Uuid::now_v7()]);
    let sheets = [(InstancePath::root(), &root), (child_path, &child)];
    let annotated: Vec<_> = design_parts(&sheets)
        .into_iter()
        .filter(|p| !p.needs_annotation)
        .map(|p| (p.sheet, p.designator, p.units))
        .collect();
    assert_eq!(annotated, old_parts_of(&sheets));
    assert_eq!(annotated.len(), 5, "R1, U1, R5 twice, child R1");
}

#[test]
fn unannotated_parts_are_kept_and_flagged() {
    let (root, child) = fixture();
    let sheets = [
        (InstancePath::root(), &root),
        (InstancePath::root(), &child),
    ];
    let flagged: Vec<_> = design_parts(&sheets)
        .into_iter()
        .filter(|p| p.needs_annotation)
        .map(|p| p.designator)
        .collect();
    assert_eq!(flagged, ["R?", "", "U?"]);
}

#[test]
fn a_multi_unit_part_anchors_on_its_smallest_unit() {
    let (root, _) = fixture();
    let parts = design_parts(&[(InstancePath::root(), &root)]);
    let u1 = parts.iter().find(|p| p.designator == "U1").unwrap();
    assert_eq!(u1.units.len(), 2);
    let anchor = u1.anchor().unwrap();
    assert_eq!(anchor.sheet, InstancePath::root());
    assert_eq!(Some(&anchor.part), u1.units.iter().min());
    assert!(
        parts.iter().all(|p| p.designator != "#PWR01"),
        "power is no part"
    );
}
