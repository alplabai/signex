//! #1653 — a new user places a part straight from the bundled Stock
//! library. The stock library used to ship an empty component table, so
//! the Components panel offered nothing to place.
//!
//! Real `Stock.snxlib` from `assets/stock-library`, mounted read-only the
//! way the app mounts it, and the real `ComponentsPanelPlace` message; the
//! tasks it spawns are run to completion and fed back as the iced runtime
//! would.

use std::path::PathBuf;

use signex_app::app::{Message, Signex};
use signex_app::library::LibraryMessage;

use crate::library_place_symbol::{PLACE_AT, app_with_empty_sheet, click, doc, run};

fn stock_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library/Stock.snxlib")
}

fn app_with_stock() -> Signex {
    let mut app = app_with_empty_sheet();
    let stock = stock_file();
    app.library.stock_library_root = Some(stock.clone());
    app.library
        .open_library_read_only(stock)
        .expect("mount stock");
    app
}

fn dispatch_to_completion(app: &mut Signex, message: Message) {
    let mut pending = vec![message];
    while let Some(next) = pending.pop() {
        let task = app.update(next);
        pending.extend(run(task));
    }
}

#[test]
fn the_stock_library_mounts_read_only_and_lists_placeable_rows() {
    let app = app_with_stock();
    assert!(app.library.is_read_only(&stock_file()));
    let rows = app.library.all_components();
    assert_eq!(rows.len(), 34, "Stock must not show (0)");
}

#[test]
fn placing_the_stock_resistor_puts_a_symbol_on_the_sheet() {
    let mut app = app_with_stock();
    let (_, resistor) = app
        .library
        .all_components()
        .into_iter()
        .find(|(_, c)| c.internal_pn.as_str() == "STOCK-R")
        .expect("Stock lists the generic resistor");
    let table = app
        .library
        .table_of_row(&stock_file(), resistor.row_id)
        .expect("the resistor's table");

    dispatch_to_completion(
        &mut app,
        Message::Library(LibraryMessage::ComponentsPanelPlace {
            library_path: stock_file(),
            table,
            row_id: signex_library::RowId(resistor.row_id),
        }),
    );
    click(&mut app, PLACE_AT);

    assert!(app.document_state.error_notice.is_none());
    assert_eq!(doc(&app).symbols.len(), 1);
    assert_eq!(doc(&app).symbols[0].row_id, Some(resistor.row_id));
    assert!(
        doc(&app).symbols[0].reference.starts_with('R'),
        "the stock R takes an R designator"
    );
}

/// #1798 — a part placed from Stock starts with its reference and value
/// clear of the part. The fixed offset it used to get from the origin sat
/// inside the generic IC's body, on top of its pins.
#[test]
fn a_placed_stock_part_has_its_fields_clear_of_its_body() {
    let mut app = app_with_stock();
    let (_, ic) = app
        .library
        .all_components()
        .into_iter()
        .find(|(_, c)| c.internal_pn.as_str() == "STOCK-IC_Generic")
        .expect("Stock lists the generic IC");
    let table = app
        .library
        .table_of_row(&stock_file(), ic.row_id)
        .expect("the IC's table");

    dispatch_to_completion(
        &mut app,
        Message::Library(LibraryMessage::ComponentsPanelPlace {
            library_path: stock_file(),
            table,
            row_id: signex_library::RowId(ic.row_id),
        }),
    );
    click(&mut app, PLACE_AT);

    let sheet = doc(&app);
    let sym = &sheet.symbols[0];
    let lib = sheet.lib_symbols.get(&sym.lib_id).expect("embedded body");
    let parts = signex_types::schematic::lower_placed_symbol(Some(lib), sym);
    let body = signex_types::schematic::primitives_bounds(&parts).expect("drawn body");
    for (name, prop) in [("reference", &sym.ref_text), ("value", &sym.val_text)] {
        let at = prop.as_ref().expect(name).position;
        let inside =
            at.x > body.min_x && at.x < body.max_x && at.y > body.min_y && at.y < body.max_y;
        assert!(
            !inside,
            "{name} anchor {at:?} sits inside the part {body:?}"
        );
    }
}
