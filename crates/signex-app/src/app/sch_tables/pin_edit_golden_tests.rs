//! SCH Tables & Docs exit criterion: a pin table on a sheet follows a pin
//! edit of its part. The edit reaches the sheet the way a library pin edit
//! does, as the part's new body (`Command::ReplaceLibSymbol`, the Update
//! from library step); the next refresh rebuilds the table, and the drawn
//! columns before and after match `testdata/pin_table_follows_pin_edit.tsv`.

use signex_types::schematic::{LibSymbol, PinDirection, SchDrawing, SchTable, SchematicSheet};

use super::tests::{app_with_sheet, pin, pin_table, sheet_with_pin_table, undo};
use crate::app::handlers::canvas::placement_tests::doc;

/// The sheet of `sheet_with_pin_table` with the Net column hidden (its name
/// is the sheet's own connectivity's, not this test's subject) and Unit and
/// Pin length shown.
fn sheet() -> SchematicSheet {
    let mut sheet = sheet_with_pin_table();
    for drawing in &mut sheet.drawings {
        if let SchDrawing::Table(table) = drawing {
            table.set_column_shown("net", false);
            table.set_column_shown("unit", true);
            table.set_column_shown("pin_length", true);
        }
    }
    sheet
}

/// U's body after the pin edit: pin 1 becomes an input 5.08 mm long, pin 2
/// is renamed VCC, pin 3 EN is added, and a second unit gets pin 4.
fn edited_body() -> LibSymbol {
    let mut body: LibSymbol = serde_json::from_str(r#"{"id":"U"}"#).expect("body");
    let mut boot = pin("1", "BOOT", PinDirection::Input);
    boot.pin.length = 5.08;
    let mut out = pin("4", "OUT", PinDirection::Output);
    out.unit = 2;
    body.pins = vec![
        boot,
        pin("2", "VCC", PinDirection::PowerInput),
        pin("3", "EN", PinDirection::Input),
        out,
    ];
    body
}

/// The drawn columns of `table`, a tab between cells, the titles first.
fn drawn(table: &SchTable) -> String {
    let shown = table.shown_columns();
    let line = |cells: Vec<&str>| cells.join("\t") + "\n";
    let mut out = line(
        shown
            .iter()
            .map(|&c| table.columns[c].title.as_str())
            .collect(),
    );
    for row in 0..table.rows.len() {
        out.push_str(&line(shown.iter().map(|&c| table.cell(row, c)).collect()));
    }
    out
}

/// Compare with `testdata/<name>`; `UPDATE_GOLDEN=1` writes it. A diff is a
/// change to what the table shows, not a snapshot to refresh.
fn check_golden(name: &str, actual: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/app/sch_tables/testdata")
        .join(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, actual).expect("golden written");
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}; run with UPDATE_GOLDEN=1", path.display()))
        .replace("\r\n", "\n");
    assert!(
        expected == actual,
        "{} differs:\n{actual}\nregenerate with UPDATE_GOLDEN=1 cargo test -p signex-app \
         pin_edit_golden if the change is meant",
        path.display()
    );
}

#[test]
fn a_pin_table_follows_a_pin_edit_of_its_part() {
    let mut app = app_with_sheet(sheet());
    app.refresh_bound_tables(None);
    let before = drawn(&pin_table(&app));

    let replaced = app.apply_engine_command(
        signex_engine::Command::ReplaceLibSymbol {
            lib_symbol: edited_body(),
            library_versions: Vec::new(),
            only: None,
        },
        true,
        true,
    );
    assert!(replaced, "the part takes its new body");
    assert_eq!(
        drawn(&pin_table(&app)),
        before,
        "the table waits for the refresh"
    );
    app.refresh_bound_tables(None);
    let after = drawn(&pin_table(&app));
    check_golden(
        "pin_table_follows_pin_edit.tsv",
        &format!("# before the pin edit\n{before}# after the pin edit\n{after}"),
    );

    undo(&mut app);
    assert_eq!(
        drawn(&pin_table(&app)),
        before,
        "one undo takes the refresh back"
    );
    let body = doc(&app)
        .symbols
        .first()
        .and_then(|s| doc(&app).body_of(s))
        .expect("U1's body");
    assert_eq!(body.pins.len(), 4, "and leaves the edited part");
}
