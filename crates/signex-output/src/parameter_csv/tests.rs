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

/// `doc` as the one listed sheet `name`, each part named by its reference.
fn listed<'a>(name: &str, doc: &'a SchematicSheet) -> ListedSheet<'a> {
    ListedSheet {
        name: name.to_string(),
        parts: listed_parts(doc)
            .into_iter()
            .map(ListedPart::stored)
            .collect(),
    }
}

fn exported(sheet: &SchematicSheet) -> Vec<u8> {
    export(&[listed("power.snxsch", sheet)], false).expect("export")
}

fn plan_of(sheet: &SchematicSheet, text: &str) -> ImportPlan {
    let csv = parse(text.as_bytes()).expect("parse");
    plan_import(&[listed("power.snxsch", sheet)], &csv)
}

fn plan_across(sheets: &[ListedSheet<'_>], text: &str) -> ImportPlan {
    let csv = parse(text.as_bytes()).expect("parse");
    plan_import(sheets, &csv)
}

/// Where each change goes: its sheet's index, its part and the new value.
fn targets(plan: &ImportPlan) -> Vec<(usize, Uuid, &str)> {
    (plan.changes.iter())
        .map(|c| (c.sheet, c.symbol, c.after.as_str()))
        .collect()
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
        "\u{feff}Unique Id,Reference,Value,Footprint,MPN,Tolerance\r\n\
         00000000-0000-0000-0000-000000000001,R1,10k,R_0603,RC0603,1%\r\n\
         00000000-0000-0000-0000-000000000002,R2,10k,R_0603,,5%\r\n"
    );
}

/// A line goes to the part its Unique Id names, whatever its reference now
/// says, so a re-annotated sheet still takes its edited export.
#[test]
fn a_line_goes_to_the_part_its_unique_id_names() {
    let doc = sheet(vec![
        part(1, "R1", &[("Tolerance", "1%")]),
        part(2, "R2", &[("Tolerance", "5%")]),
    ]);
    let plan = plan_of(
        &doc,
        "Unique Id,Reference,Tolerance\n\
         00000000-0000-0000-0000-000000000002,R1,2%\n\
         00000000-0000-0000-0000-000000000001,R2,1%\n",
    );
    assert_eq!(
        plan.changes,
        vec![ParameterChange {
            sheet: 0,
            symbol: Uuid::from_u128(2),
            reference: "R2".to_string(),
            key: "Tolerance".to_string(),
            before: Some("5%".to_string()),
            after: "2%".to_string(),
        }]
    );
    assert_eq!(plan.unmatched_lines, 0);
}

/// An id no listed part has, an id a line already took and an id that is
/// not a uuid are unmatched lines; a reference never stands in for them.
#[test]
fn an_unknown_or_repeated_unique_id_is_unmatched() {
    let doc = sheet(vec![part(1, "R1", &[("Tolerance", "1%")])]);
    let plan = plan_of(
        &doc,
        "Unique Id,Reference,Tolerance\n\
         00000000-0000-0000-0000-000000000009,R1,2%\n\
         00000000-0000-0000-0000-000000000001,R1,3%\n\
         00000000-0000-0000-0000-000000000001,R1,4%\n\
         not-an-id,,5%\n",
    );
    assert_eq!(plan.changes.len(), 1);
    assert_eq!(plan.changes[0].after, "3%");
    assert_eq!(plan.unmatched_lines, 3);
    assert_eq!(
        plan.unmatched,
        vec!["R1".to_string(), "not-an-id".to_string()]
    );
}

/// A line without an id goes by reference, but never to a part a Unique Id
/// in the file already names.
#[test]
fn a_line_without_an_id_goes_by_reference_to_a_part_no_id_names() {
    let doc = sheet(vec![
        part(1, "U1", &[("Note", "a")]),
        part(2, "U1", &[("Note", "b")]),
    ]);
    let plan = plan_of(
        &doc,
        "Unique Id,Reference,Note\n\
         00000000-0000-0000-0000-000000000001,U1,x\n\
         ,U1,y\n",
    );
    let targets: Vec<(Uuid, &str)> = plan
        .changes
        .iter()
        .map(|c| (c.symbol, c.after.as_str()))
        .collect();
    assert_eq!(
        targets,
        vec![(Uuid::from_u128(1), "x"), (Uuid::from_u128(2), "y")]
    );
}

#[test]
fn added_and_changed_parameters_are_counted_apart() {
    let doc = sheet(vec![part(1, "R1", &[("Tolerance", "1%")])]);
    let plan = plan_of(&doc, "Reference,Tolerance,MPN\nR1,2%,RC0603\nR7,1%,\n");
    assert_eq!(plan.changed_count(), 1);
    assert_eq!(plan.added_count(), 1);
    assert!(plan.changes[1].is_added());
    assert_eq!(plan.unmatched_lines, 1);
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
        plan_import(&[listed("power.snxsch", &doc)], &csv),
        ImportPlan::default()
    );
}

#[test]
fn a_formula_cell_is_written_behind_a_quote() {
    let doc = sheet(vec![part(1, "R1", &[("=HYPERLINK(1)", "=SUM(A1)")])]);
    let text = String::from_utf8(exported(&doc)).expect("utf-8");
    assert_eq!(
        text,
        "\u{feff}Unique Id,Reference,Value,Footprint,'=HYPERLINK(1)\r\n\
         00000000-0000-0000-0000-000000000001,R1,10k,R_0603,'=SUM(A1)\r\n"
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
            sheet: 0,
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
        Err(ParameterCsvError::NoKeyColumn)
    );
    assert_eq!(
        parse(b"Unique Id,unique id\n"),
        Err(ParameterCsvError::DuplicateColumn("unique id".to_string()))
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
fn a_file_keyed_by_unique_id_alone_is_read() {
    let doc = sheet(vec![part(1, "R1", &[("Tolerance", "1%")])]);
    let plan = plan_of(
        &doc,
        " UNIQUE ID ,Tolerance\n00000000-0000-0000-0000-000000000001,2%\n",
    );
    assert_eq!(plan.changes.len(), 1);
    assert_eq!(plan.changes[0].reference, "R1");
}

/// #1784: an All sheets export has a Sheet column after Reference and every
/// listed sheet's parts, sheet by sheet; a part's Reference is the
/// designator the table names it by, not its stored reference.
#[test]
fn an_all_sheets_export_names_each_parts_sheet() {
    let a = sheet(vec![part(1, "R1", &[("Tolerance", "1%")])]);
    let b = sheet(vec![part(2, "R1", &[("MPN", "RC0603")])]);
    let mut on_b = listed("ch/b.snxsch", &b);
    on_b.parts[0].reference = "R101".into();
    let text = String::from_utf8(export(&[listed("a.snxsch", &a), on_b], true).expect("export"))
        .expect("utf-8");
    assert_eq!(
        text,
        "\u{feff}Unique Id,Reference,Sheet,Value,Footprint,MPN,Tolerance\r\n\
         00000000-0000-0000-0000-000000000001,R1,a.snxsch,10k,R_0603,,1%\r\n\
         00000000-0000-0000-0000-000000000002,R101,ch/b.snxsch,10k,R_0603,RC0603,\r\n"
    );
}

#[test]
fn an_all_sheets_export_reads_back_to_no_change() {
    let a = sheet(vec![part(1, "R1", &[("Tolerance", "1%")])]);
    let b = sheet(vec![part(1, "R1", &[("Tolerance", "5%")])]);
    let sheets = [listed("a.snxsch", &a), listed("b.snxsch", &b)];
    let csv = parse(&export(&sheets, true).expect("export")).expect("parse");
    assert_eq!(csv.rows[1].sheet, "b.snxsch");
    assert_eq!(plan_import(&sheets, &csv), ImportPlan::default());
}

/// A sheet file copied keeps its parts' uuids: the Sheet cell says which
/// copy a line is for.
#[test]
fn a_sheet_cell_sends_its_line_to_that_sheet() {
    let a = sheet(vec![part(1, "R1", &[("Tolerance", "1%")])]);
    let b = sheet(vec![part(1, "R1", &[("Tolerance", "1%")])]);
    let sheets = [listed("a.snxsch", &a), listed("b.snxsch", &b)];
    let plan = plan_across(
        &sheets,
        "Unique Id,Reference,sheet ,Tolerance\n\
         00000000-0000-0000-0000-000000000001,R1,b.snxsch,2%\n\
         ,R1,a.snxsch,3%\n",
    );
    assert_eq!(
        targets(&plan),
        vec![(1, Uuid::from_u128(1), "2%"), (0, Uuid::from_u128(1), "3%")]
    );
    assert_eq!(plan.part_count(), 2, "one part on each sheet");
}

/// Without a Sheet cell an id two listed sheets carry names neither, so no
/// line is written to a guessed sheet; an id one sheet carries still goes
/// there, as a file exported from one sheet does.
#[test]
fn an_id_on_two_listed_sheets_needs_a_sheet_cell() {
    let a = sheet(vec![
        part(1, "R1", &[("Tolerance", "1%")]),
        part(2, "R2", &[]),
    ]);
    let b = sheet(vec![part(1, "R1", &[("Tolerance", "1%")])]);
    let sheets = [listed("a.snxsch", &a), listed("b.snxsch", &b)];
    let plan = plan_across(
        &sheets,
        "Unique Id,Reference,Tolerance\n\
         00000000-0000-0000-0000-000000000001,R1,2%\n\
         00000000-0000-0000-0000-000000000002,R2,5%\n",
    );
    assert_eq!(targets(&plan), vec![(0, Uuid::from_u128(2), "5%")]);
    assert_eq!(plan.unmatched, vec!["R1".to_string()]);
}

#[test]
fn a_line_without_a_sheet_cell_goes_to_any_listed_sheet() {
    let a = sheet(vec![part(1, "R1", &[])]);
    let b = sheet(vec![part(2, "R2", &[])]);
    let sheets = [listed("a.snxsch", &a), listed("b.snxsch", &b)];
    let plan = plan_across(&sheets, "Reference,Sheet,MPN\nR2,,X\nR1,c.snxsch,Y\n");
    assert_eq!(targets(&plan), vec![(1, Uuid::from_u128(2), "X")]);
    assert_eq!(
        plan.unmatched,
        vec!["R1".to_string()],
        "no listed sheet is named c.snxsch"
    );
}

/// A line keyed by reference matches the designator the table shows (the
/// placement's, #1151), as the exported Reference cell reads.
#[test]
fn a_reference_line_matches_the_shown_designator() {
    let doc = sheet(vec![part(1, "R1", &[])]);
    let mut shown = listed("child.snxsch", &doc);
    shown.parts[0].reference = "R101".into();
    let plan = plan_across(&[shown], "Reference,MPN\nR101,X\nR1,Y\n");
    assert_eq!(targets(&plan), vec![(0, Uuid::from_u128(1), "X")]);
    assert_eq!(plan.changes[0].reference, "R101");
    assert_eq!(plan.unmatched, vec!["R1".to_string()]);
}

#[test]
fn two_sheet_columns_are_refused() {
    assert_eq!(
        parse(b"Reference,Sheet,SHEET\n"),
        Err(ParameterCsvError::DuplicateColumn("SHEET".to_string()))
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
