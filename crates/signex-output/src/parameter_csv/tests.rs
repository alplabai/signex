use signex_types::schematic::{SchematicSheet, Symbol};
use uuid::Uuid;

use super::*;

fn part(n: u128, reference: &str, fields: &[(&str, &str)]) -> Symbol {
    let mut symbol = Symbol {
        uuid: Uuid::from_u128(n),
        reference: reference.to_string(),
        value: "10k".to_string(),
        footprint: "R_0603".to_string(),
        ..Default::default()
    };
    for (key, value) in fields {
        symbol.fields.insert(key.to_string(), value.to_string());
    }
    symbol
}

fn sheet(symbols: Vec<Symbol>) -> SchematicSheet {
    SchematicSheet {
        symbols,
        ..Default::default()
    }
}

fn exported(sheet: &SchematicSheet) -> Vec<u8> {
    export(&listed_parts(sheet)).expect("export")
}

fn plan_of(sheet: &SchematicSheet, text: &str) -> ImportPlan {
    let csv = parse(text.as_bytes()).expect("parse");
    plan_import(&listed_parts(sheet), &csv)
}

#[test]
fn an_export_has_the_titles_then_one_line_per_listed_part() {
    let mut gnd = part(9, "#PWR1", &[("Hidden", "x")]);
    gnd.is_power = true;
    let doc = sheet(vec![
        part(1, "R1", &[("Tolerance", "1%"), ("MPN", "RC0603")]),
        gnd,
        part(2, "R2", &[("Tolerance", "5%")]),
    ]);
    let text = String::from_utf8(exported(&doc)).expect("utf-8");
    assert_eq!(
        text,
        "\u{feff}Reference,Value,Footprint,MPN,Tolerance\r\n\
         R1,10k,R_0603,RC0603,1%\r\n\
         R2,10k,R_0603,,5%\r\n"
    );
}

/// Every awkward value comes back exactly as it went out, so importing an
/// untouched export changes nothing.
#[test]
fn an_export_reads_back_to_no_change() {
    let values = [
        "a, b",
        "say \"hi\"",
        "two\r\nlines",
        "=SUM(A1)",
        "@cmd",
        "+1+1",
        "-5V",
        "+3.3V",
        "'=already quoted",
        "''=twice",
        "'plain",
        "'",
        "\tlead",
        "İstanbul µF",
        "",
    ];
    let symbols = values
        .iter()
        .enumerate()
        .map(|(i, v)| part(i as u128 + 1, &format!("R{}", i + 1), &[("Note", v)]))
        .collect();
    let doc = sheet(symbols);
    let csv = parse(&exported(&doc)).expect("parse");
    for (row, value) in csv.rows.iter().zip(values) {
        assert_eq!(row.values, vec![Some(value.to_string())], "{value:?}");
    }
    assert_eq!(
        plan_import(&listed_parts(&doc), &csv),
        ImportPlan::default()
    );
}

#[test]
fn a_formula_cell_is_written_behind_a_quote() {
    let doc = sheet(vec![part(1, "R1", &[("=HYPERLINK(1)", "=SUM(A1)")])]);
    let text = String::from_utf8(exported(&doc)).expect("utf-8");
    assert_eq!(
        text,
        "\u{feff}Reference,Value,Footprint,'=HYPERLINK(1)\r\nR1,10k,R_0603,'=SUM(A1)\r\n"
    );
    let csv = parse(text.as_bytes()).expect("parse");
    assert_eq!(csv.keys, vec!["=HYPERLINK(1)".to_string()]);
}

#[test]
fn a_changed_cell_is_one_change_and_value_and_footprint_are_never_imported() {
    let doc = sheet(vec![
        part(1, "R1", &[("Tolerance", "1%")]),
        part(2, "R2", &[("Tolerance", "5%")]),
    ]);
    let plan = plan_of(
        &doc,
        "reference , VALUE,Footprint,Tolerance\nR1,22k,R_0805,2%\nR2,10k,R_0603,5%\n",
    );
    assert_eq!(
        plan.changes,
        vec![ParameterChange {
            symbol: Uuid::from_u128(1),
            reference: "R1".to_string(),
            key: "Tolerance".to_string(),
            before: Some("1%".to_string()),
            after: "2%".to_string(),
        }]
    );
    assert_eq!(plan.part_count(), 1);
}

#[test]
fn an_empty_cell_empties_a_parameter_and_adds_none() {
    let doc = sheet(vec![part(1, "R1", &[("Tolerance", "1%")])]);
    let plan = plan_of(&doc, "Reference,Tolerance,MPN\nR1,,\n");
    assert_eq!(plan.changes.len(), 1);
    assert_eq!(plan.changes[0].key, "Tolerance");
    assert_eq!(plan.changes[0].after, "");
}

#[test]
fn a_new_column_adds_the_parameter() {
    let doc = sheet(vec![part(1, "R1", &[])]);
    let plan = plan_of(&doc, "Reference,MPN\nR1,RC0603\n");
    assert_eq!(plan.changes[0].before, None);
    assert_eq!(plan.changes[0].after, "RC0603");
}

#[test]
fn a_short_line_a_blank_title_and_a_blank_reference_change_nothing() {
    let doc = sheet(vec![part(1, "R1", &[("Tolerance", "1%"), ("MPN", "A")])]);
    let plan = plan_of(&doc, "Reference,MPN,,Tolerance\nR1,A,junk\n,B,,7%\n");
    assert_eq!(plan, ImportPlan::default());
}

#[test]
fn units_sharing_a_reference_take_their_lines_in_order() {
    let doc = sheet(vec![
        part(1, "U1", &[("Note", "a")]),
        part(2, "U1", &[("Note", "b")]),
    ]);
    let plan = plan_of(&doc, "Reference,Note\nU1,a\nU1,c\nU1,d\n");
    assert_eq!(plan.changes.len(), 1);
    assert_eq!(plan.changes[0].symbol, Uuid::from_u128(2));
    assert_eq!(plan.unmatched, vec!["U1".to_string()]);
}

#[test]
fn a_reference_not_on_the_sheet_is_listed_once() {
    let doc = sheet(vec![part(1, "R1", &[])]);
    let plan = plan_of(&doc, "Reference,MPN\nR9,x\nR9,y\nC3,z\n");
    assert!(plan.changes.is_empty());
    assert_eq!(plan.unmatched, vec!["R9".to_string(), "C3".to_string()]);
}

#[test]
fn a_power_port_is_never_a_target() {
    let mut gnd = part(1, "#PWR1", &[]);
    gnd.is_power = true;
    let doc = sheet(vec![gnd]);
    let plan = plan_of(&doc, "Reference,MPN\n#PWR1,x\n");
    assert!(plan.changes.is_empty());
    assert_eq!(plan.unmatched, vec!["#PWR1".to_string()]);
}

#[test]
fn files_that_are_not_a_parameter_table_are_refused() {
    assert_eq!(parse(b"\xff\xfe"), Err(ParameterCsvError::NotUtf8));
    assert_eq!(parse(b"\xef\xbb\xbf\r\n"), Err(ParameterCsvError::NoHeader));
    assert_eq!(
        parse(b"Value,MPN\n10k,x\n"),
        Err(ParameterCsvError::NoReferenceColumn)
    );
    assert_eq!(
        parse(b"Reference,MPN,MPN\n"),
        Err(ParameterCsvError::DuplicateColumn("MPN".to_string()))
    );
    assert_eq!(
        parse(b"Reference,reference\n"),
        Err(ParameterCsvError::DuplicateColumn("reference".to_string()))
    );
}

#[test]
fn quoted_fields_keep_commas_quotes_and_line_breaks() {
    let csv =
        parse("Reference,A,B\nR1,\"x, y\",\"say \"\"hi\"\"\nthere\"\n".as_bytes()).expect("parse");
    assert_eq!(
        csv.rows[0].values,
        vec![
            Some("x, y".to_string()),
            Some("say \"hi\"\nthere".to_string())
        ]
    );
}
