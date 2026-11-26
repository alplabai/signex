use std::collections::BTreeMap;

use super::*;
use crate::{
    BomContext, BomEngineOptions, BomGrouping, BomMetadata, BomPrice, build_table_with_members,
};

fn part(reference: &str, value: &str) -> BomComponent {
    BomComponent {
        reference: reference.to_string(),
        name: value.to_string(),
        value: value.to_string(),
        footprint: "R_0603".to_string(),
        lib_ref: "Device:R".to_string(),
        description: String::new(),
        dnp: false,
        in_bom: true,
        on_board: true,
        variant_fitted: None,
        base_fitted: None,
        custom: BTreeMap::new(),
        unit_conflicts: Vec::new(),
        library_row: None,
    }
}

fn vars(sheet: &[(&str, &str)], project: &[(&str, &str)]) -> MemberVariables {
    let table = |pairs: &[(&str, &str)]| {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    };
    MemberVariables {
        sheet: table(sheet),
        project: table(project),
    }
}

fn power() -> ComputedColumn {
    ComputedColumn::new("Power", "= VCC^2 / Value").unwrap()
}

/// The table of `components` under `grouping`, with `columns` evaluated.
fn evaluated(
    components: Vec<BomComponent>,
    variables: &[MemberVariables],
    grouping: BomGrouping,
    columns: &[ComputedColumn],
) -> BomTable {
    let ctx = BomContext {
        components,
        metadata: BomMetadata::default(),
    };
    let opts = BomEngineOptions {
        grouping,
        ..BomEngineOptions::default()
    };
    let (mut table, members) = build_table_with_members(&ctx, &opts);
    apply_computed_columns(&mut table, &members, &ctx.components, variables, columns);
    table
}

#[test]
fn a_row_column_reads_the_value_and_a_project_variable_with_its_unit() {
    let table = evaluated(
        vec![part("R1", "10kΩ")],
        &[vars(&[], &[("VCC", "5V")])],
        BomGrouping::Ungrouped,
        &[power()],
    );
    assert_eq!(
        table.rows[0].computed.get("Power"),
        Some(&ComputedCell::Value {
            text: "2.5mW".into()
        })
    );
    assert!(computed_issues(&table, &[power()]).is_empty());
}

#[test]
fn a_sheet_variable_shadows_the_project_variable() {
    let table = evaluated(
        vec![part("R1", "10kΩ")],
        &[vars(&[("VCC", "10V")], &[("VCC", "5V")])],
        BomGrouping::Ungrouped,
        &[power()],
    );
    assert_eq!(
        table.rows[0].computed.get("Power"),
        Some(&ComputedCell::Value {
            text: "10mW".into()
        })
    );
}

#[test]
fn a_grouped_row_whose_members_agree_shows_the_shared_result() {
    let five = vars(&[("VCC", "5V")], &[]);
    let table = evaluated(
        vec![part("R1", "10kΩ"), part("R2", "10kΩ")],
        &[five.clone(), five],
        BomGrouping::Grouped,
        &[power()],
    );
    assert_eq!(table.rows.len(), 1);
    assert_eq!(table.rows[0].references, ["R1", "R2"]);
    assert_eq!(
        table.rows[0].computed.get("Power"),
        Some(&ComputedCell::Value {
            text: "2.5mW".into()
        })
    );
}

#[test]
fn a_grouped_row_whose_members_differ_says_so() {
    // Two equal parts on sheets with different rails: one line, two results.
    let table = evaluated(
        vec![part("R1", "10kΩ"), part("R2", "10kΩ")],
        &[vars(&[("VCC", "5V")], &[]), vars(&[("VCC", "3.3V")], &[])],
        BomGrouping::Grouped,
        &[power()],
    );
    assert_eq!(table.rows.len(), 1);
    assert_eq!(
        table.rows[0].computed.get("Power"),
        Some(&ComputedCell::Differs)
    );
    assert!(computed_issues(&table, &[power()]).is_empty());
}

#[test]
fn an_error_row_holds_the_error_and_is_reported_once_naming_the_row() {
    let table = evaluated(
        vec![part("R1", "10kΩ"), part("C1", "100nF")],
        &[vars(&[], &[("VCC", "5V")]), vars(&[], &[])],
        BomGrouping::Ungrouped,
        &[power()],
    );
    assert_eq!(
        table.rows[0].computed.get("Power"),
        Some(&ComputedCell::Error {
            member: None,
            error: "unknown parameter 'VCC'".into()
        })
    );
    let issues = computed_issues(&table, &[power()]);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].line, 1);
    assert_eq!(issues[0].references, ["C1"]);
    assert_eq!(
        issues[0].to_string(),
        "BOM line 1 (C1): column Power: unknown parameter 'VCC'"
    );
}

#[test]
fn a_grouped_error_names_the_part_it_failed_for() {
    let table = evaluated(
        vec![part("R1", "10kΩ"), part("R2", "10kΩ")],
        &[vars(&[("VCC", "5V")], &[]), vars(&[], &[])],
        BomGrouping::Grouped,
        &[power()],
    );
    let issues = computed_issues(&table, &[power()]);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].member.as_deref(), Some("R2"));
    assert_eq!(
        issues[0].to_string(),
        "BOM line 1 (R1, R2): column Power for R2: unknown parameter 'VCC'"
    );
}

#[test]
fn quantity_and_the_cached_unit_price_are_line_names() {
    let cost = ComputedColumn::new("Cost", "= Quantity * UnitPrice").unwrap();
    let ctx = BomContext {
        components: vec![part("R1", "10k"), part("R2", "10k")],
        metadata: BomMetadata::default(),
    };
    let (mut table, members) = build_table_with_members(&ctx, &BomEngineOptions::default());
    table.rows[0].price = Some(BomPrice {
        distributor: "Stock".into(),
        currency: "EUR".into(),
        unit_price: 0.25,
        extended_price: 0.5,
        buy_qty: 2,
        stock: Some(100),
        age: "today".into(),
    });
    apply_computed_columns(
        &mut table,
        &members,
        &ctx.components,
        &[],
        std::slice::from_ref(&cost),
    );
    assert_eq!(
        table.rows[0].computed.get("Cost"),
        Some(&ComputedCell::Value { text: "0.5".into() })
    );
}

#[test]
fn an_unpriced_line_cannot_read_the_unit_price() {
    let cost = ComputedColumn::new("Cost", "= Quantity * UnitPrice").unwrap();
    let table = evaluated(
        vec![part("R1", "10k")],
        &[],
        BomGrouping::Ungrouped,
        &[cost],
    );
    assert!(matches!(
        table.rows[0].computed.get("Cost"),
        Some(ComputedCell::Error { .. })
    ));
}

#[test]
fn a_user_field_is_read_by_name() {
    let mut r1 = part("R1", "10k");
    r1.custom.insert("Rated".into(), "250mW".into());
    let derate = ComputedColumn::new("Half", "= rated / 2").unwrap();
    let table = evaluated(vec![r1], &[], BomGrouping::Ungrouped, &[derate]);
    assert_eq!(
        table.rows[0].computed.get("Half"),
        Some(&ComputedCell::Value {
            text: "125mW".into()
        })
    );
}

#[test]
fn a_column_needs_a_name_and_an_expression_that_parses() {
    assert!(ComputedColumn::new("", "= 1").is_err());
    assert!(ComputedColumn::new("Power", "VCC^2").is_err());
    assert!(ComputedColumn::new("Power", "= VCC^").is_err());
    let column = ComputedColumn::new("Power", "= VCC^2 / Value").unwrap();
    assert_eq!(column.name(), "Power");
    assert_eq!(column.expression(), "= VCC^2 / Value");
}

#[test]
fn members_name_the_components_of_each_row() {
    let ctx = BomContext {
        components: vec![part("R2", "10k"), part("C1", "1u"), part("R1", "10k")],
        metadata: BomMetadata::default(),
    };
    let (table, members) = build_table_with_members(&ctx, &BomEngineOptions::default());
    assert_eq!(table.rows[0].references, ["C1"]);
    assert_eq!(members, vec![vec![1], vec![2, 0]]);
    assert_eq!(
        crate::build_table(&ctx, &BomEngineOptions::default())
            .rows
            .len(),
        table.rows.len()
    );
}
