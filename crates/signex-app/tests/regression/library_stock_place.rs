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
