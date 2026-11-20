//! The hoisted fit resolver (#1293) and the variant matrix built on it.

use signex_types::property::SchematicProperty;
use signex_types::schematic::{InstancePath, SchematicSheet, Symbol};
use uuid::Uuid;

use super::*;
use crate::{ExportContext, ProjectMetadata, SheetSnapshot};

fn part(reference: &str, value: &str, fields: &[(&str, &str)]) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: value.to_string(),
        footprint: "R_0603".to_string(),
        fields: fields
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
        ..Default::default()
    }
}

fn context(symbols: Vec<Symbol>) -> ExportContext {
    ExportContext {
        sheets: vec![SheetSnapshot {
            path: "main.snxsch".into(),
            schematic: SchematicSheet {
                symbols,
                ..Default::default()
            },
            sheet_name: "Main".to_string(),
            sheet_number: 1,
            sheet_count: 1,
            placement: InstancePath::root(),
        }],
        metadata: ProjectMetadata::default(),
        netlist: None,
        netlist_sheets: Vec::new(),
        netlist_placements: Vec::new(),
        current_sheet: None,
    }
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| name.to_string()).collect()
}

#[test]
fn base_fits_a_part_unless_it_is_marked_dnp_or_not_fitted() {
    assert!(is_fitted(&part("R1", "10k", &[]), None));
    let mut dnp = part("R2", "10k", &[]);
    dnp.dnp = true;
    assert!(!is_fitted(&dnp, None));
    assert!(!is_fitted(&part("R3", "10k", &[("Fitted", "no")]), None));
    // Base's own name is Base.
    assert!(!is_fitted(
        &part("R3", "10k", &[("Fitted", "no")]),
        Some("base")
    ));
}

#[test]
fn a_variant_field_overrides_base_for_that_variant_only() {
    let lite_off = part("R5", "31.6k", &[("Fitted@Lite", "no")]);
    assert!(is_fitted(&lite_off, None));
    assert!(!is_fitted(&lite_off, Some("Lite")));
    assert!(
        !is_fitted(&lite_off, Some("LITE")),
        "names ignore ASCII case"
    );
    assert!(is_fitted(&lite_off, Some("Pro")));

    let dnp_in_lite = part("R6", "1k", &[("DNP@Lite", "yes")]);
    assert!(is_fitted(&dnp_in_lite, None));
    assert!(!is_fitted(&dnp_in_lite, Some("Lite")));

    // A variant override brings a DNP part back in that variant.
    let mut back = part("R7", "1k", &[("Fitted@Pro", "yes")]);
    back.dnp = true;
    assert!(!is_fitted(&back, None));
    assert!(is_fitted(&back, Some("Pro")));
}

#[test]
fn fitted_outranks_dnp_when_a_variant_sets_both() {
    for _ in 0..32 {
        // Fitted@Lite=yes says fitted, DNP@Lite=yes says not: Fitted wins.
        let both = part("R8", "1k", &[("Fitted@Lite", "yes"), ("DNP@Lite", "yes")]);
        assert!(is_fitted(&both, Some("Lite")));
    }
}

#[test]
fn a_property_override_is_read_before_the_fields() {
    let mut symbol = part("R9", "1k", &[("Fitted@Lite", "yes")]);
    let mut property = SchematicProperty {
        key: "Fitted".to_string(),
        value: "yes".to_string(),
        ..Default::default()
    };
    property
        .variant_overrides
        .insert("Lite".to_string(), "no".to_string());
    symbol.custom_properties.push(property);
    assert!(!is_fitted(&symbol, Some("Lite")));
    assert!(is_fitted(&symbol, None));
}

#[test]
fn one_unit_not_fitted_makes_its_part_not_fitted() {
    let mut a = part("U1", "LM358", &[]);
    a.lib_id = "Amp:LM358".to_string();
    a.unit = 1;
    let mut b = part("U1", "LM358", &[("Fitted@Lite", "no")]);
    b.lib_id = "Amp:LM358".to_string();
    b.unit = 2;
    assert!(part_is_fitted(&[&a, &b], None));
    assert!(!part_is_fitted(&[&a, &b], Some("Lite")));
}

#[test]
fn the_matrix_has_a_row_per_part_and_a_column_per_variant() {
    let mut power = part("#PWR01", "GND", &[]);
    power.is_power = true;
    let mut u_a = part("U1", "LM358", &[]);
    u_a.lib_id = "Amp:LM358".to_string();
    u_a.unit = 1;
    let mut u_b = part("U1", "", &[("Fitted@Lite", "no")]);
    u_b.lib_id = "Amp:LM358".to_string();
    u_b.unit = 2;
    let ctx = context(vec![
        part("R10", "1k", &[]),
        part("R2", "10k", &[("DNP@Pro", "yes")]),
        power,
        u_a,
        u_b,
    ]);
    let matrix = variant_matrix(&ctx, &names(&["Lite", "Pro"]));
    let designators: Vec<&str> = matrix.rows.iter().map(|r| r.designator.as_str()).collect();
    assert_eq!(
        designators,
        ["R2", "R10", "U1"],
        "power ports are not parts"
    );
    let u1 = &matrix.rows[2];
    assert_eq!(u1.value, "LM358");
    assert_eq!(u1.footprint, "R_0603");
    assert_eq!(u1.fitted, [true, false, true]);
    assert_eq!(matrix.rows[0].fitted, [true, true, false]);

    assert_eq!(matrix.column(None), Some(0));
    assert_eq!(matrix.column(Some("Base")), Some(0));
    assert_eq!(matrix.column(Some("pro")), Some(2));
    assert_eq!(matrix.column(Some("Mini")), None);
    assert_eq!(matrix.fitted_count(0), 3);
    assert_eq!(matrix.fitted_count(1), 2);
    assert_eq!(matrix.changes(1), 1);
    assert_eq!(matrix.changes(2), 1);
    assert_eq!(matrix.differing_rows(), 2);
    assert_eq!(matrix.differing_cells(), 2);
}

#[test]
fn rows_that_match_base_everywhere_do_not_differ() {
    let ctx = context(vec![
        part("C1", "100n", &[]),
        part("C2", "22u", &[("Fitted@Lite", "no")]),
    ]);
    let matrix = variant_matrix(&ctx, &names(&["Lite"]));
    let differing: Vec<&str> = matrix
        .rows
        .iter()
        .filter(|row| row.differs())
        .map(|row| row.designator.as_str())
        .collect();
    assert_eq!(differing, ["C2"]);
    assert!(
        !matrix.rows[0].differs_at(0),
        "Base never differs from itself"
    );
}

/// Maintainer decision 2026-11-20 (Caner): the matrix's Value is the one the
/// BOM lists, an `=` value evaluated on its sheet rather than its source.
#[test]
fn an_expression_value_shows_evaluated_in_the_matrix_as_in_the_bom() {
    let mut ctx = context(vec![
        part("R1", "= Rbase * 2", &[("Rbase", "10kΩ")]),
        part("R2", "= VCC / 10mA", &[]),
        part("R3", "= R9.Value", &[]),
        part("R4", "4k7", &[]),
    ]);
    let params = std::collections::BTreeMap::from([("VCC".to_string(), "5V".to_string())]);
    ctx.metadata = ProjectMetadata::from_values(&params);
    ctx.metadata.project_parameters = Some(params);

    let matrix = variant_matrix(&ctx, &names(&["Lite"]));
    let shown: Vec<(&str, &str)> = matrix
        .rows
        .iter()
        .map(|row| (row.designator.as_str(), row.value.as_str()))
        .collect();
    assert_eq!(
        shown,
        [
            ("R1", "20kΩ"),
            ("R2", "500Ω"),
            ("R3", "= R9.Value"),
            ("R4", "4k7"),
        ],
        "evaluated values; one that does not evaluate keeps its source"
    );

    let table = crate::bom::rollup(&ctx, &crate::bom::BomOptions::default());
    let mut listed: Vec<(String, String)> = table
        .rows
        .iter()
        .map(|row| (row.references.join(","), row.value.clone()))
        .collect();
    listed.sort();
    let mut in_matrix: Vec<(String, String)> = shown
        .iter()
        .map(|(designator, value)| (designator.to_string(), value.to_string()))
        .collect();
    in_matrix.sort();
    assert_eq!(in_matrix, listed, "the matrix shows the BOM's values");
}

#[test]
fn a_project_with_base_only_has_one_column() {
    let matrix = variant_matrix(&context(vec![part("R1", "1k", &[])]), &[]);
    assert_eq!(matrix.rows[0].fitted, [true]);
    assert_eq!(matrix.differing_rows(), 0);
    assert_eq!(matrix.differing_cells(), 0);
}

#[test]
fn the_filter_matches_designators_and_values_with_wildcards() {
    let row = VariantRow {
        designator: "U12".to_string(),
        value: "10k".to_string(),
        footprint: String::new(),
        fitted: vec![true],
    };
    assert!(row.matches(""));
    assert!(row.matches("  , "));
    assert!(row.matches("u*"));
    assert!(row.matches("U1*"));
    assert!(row.matches("*2"));
    assert!(row.matches("u*2"));
    assert!(!row.matches("R*"));
    assert!(!row.matches("*1"));
    assert!(
        row.matches("10K"),
        "a plain term matches anywhere, any case"
    );
    assert!(row.matches("R5, U*"), "any term may match");
    assert!(row.matches("R5 10k"));
    assert!(!row.matches("R5, C*"));
}
