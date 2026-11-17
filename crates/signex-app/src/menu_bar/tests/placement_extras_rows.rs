//! #1667: Place ▸ Design Block... and Place ▸ Import Graphics... run
//! `place_design_block` and `import_graphics`: live on a sheet, greyed
//! without one, and a click asks for the file.

use super::*;

const ROWS: [&str; 2] = ["Place/Design Block...", "Place/Import Graphics..."];

#[test]
fn the_file_rows_are_live_on_a_sheet_and_ask_for_the_file() {
    for path in ROWS {
        let mut app = one_symbol_selected();
        assert!(row_is_live(&app, path), "{path} is greyed on a sheet");
        let task = click(&mut app, path);
        assert!(has_work(&task), "{path}: no file dialog was asked for");
        assert_eq!(doc(&app).symbols.len(), 1, "{path}: nothing placed yet");
    }
}

#[test]
fn the_file_rows_are_greyed_without_a_sheet() {
    let app = bare_app();
    for path in ROWS {
        assert!(!row_is_live(&app, path), "{path} is live with no schematic");
    }
}
