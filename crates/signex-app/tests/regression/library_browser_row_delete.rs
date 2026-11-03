//! Library Browser: the per-row delete button asks the same confirmation
//! "Delete Selected" does, and a Component Preview opened from the browser
//! resolves its panes without a tab switch.

use std::path::PathBuf;

use signex_app::app::{Message, Signex};
use signex_app::library::messages::LibraryMessage;
use signex_app::library::state::EditorAddress;
use signex_library::RowId;

use crate::library_place_symbol::run;
use crate::library_read_only_mount::{
    TABLE, app_with_browser, library_with_rows, stock_like_library,
};

fn settle(app: &mut Signex, message: Message) {
    let mut pending = vec![message];
    while let Some(next) = pending.pop() {
        let task = app.update(next);
        pending.extend(run(task));
    }
}

fn row_count(app: &Signex, snxlib: &std::path::Path) -> usize {
    app.library
        .library_at(snxlib)
        .map_or(0, |lib| lib.tables.get(TABLE).map_or(0, Vec::len))
}

#[test]
fn a_row_delete_asks_first_and_confirming_deletes_only_that_row() {
    let _notes = crate::shared_locks::new_row_notes();
    let tmp = tempfile::Builder::new()
        .prefix("signex-row-del-")
        .tempdir()
        .expect("tempdir");
    let (snxlib, ids) = library_with_rows(tmp.path(), "Row Delete", 3);
    let mut app = app_with_browser(&snxlib, false);
    assert_eq!(row_count(&app, &snxlib), 3);
    let middle = ids[1];

    settle(
        &mut app,
        Message::Library(LibraryMessage::BrowserDeleteRowRequest {
            library_path: snxlib.clone(),
            table: TABLE.to_string(),
            row_id: middle,
        }),
    );
    let confirm = app.library.library_browsers[&snxlib]
        .delete_confirm
        .as_ref()
        .expect("the request opens the confirm");
    assert_eq!(confirm.row_id, middle);
    assert_eq!(
        row_count(&app, &snxlib),
        3,
        "nothing deleted before confirm"
    );

    settle(
        &mut app,
        Message::Library(LibraryMessage::BrowserDeleteRowConfirm {
            library_path: snxlib.clone(),
            table: TABLE.to_string(),
            row_id: middle,
        }),
    );
    let left: Vec<RowId> = app.library.library_at(&snxlib).expect("mounted").tables[TABLE]
        .iter()
        .map(|r| RowId::from_uuid(r.row_id))
        .collect();
    assert_eq!(left.len(), 2);
    assert!(left.contains(&ids[0]) && left.contains(&ids[2]));
    assert!(!left.contains(&middle));
    assert!(
        app.library.library_browsers[&snxlib]
            .delete_confirm
            .is_none()
    );
}

#[test]
fn a_row_delete_on_a_read_only_library_opens_no_confirm() {
    let tmp = tempfile::Builder::new()
        .prefix("signex-row-del-ro-")
        .tempdir()
        .expect("tempdir");
    let (snxlib, row_id) = stock_like_library(tmp.path());
    let mut app = app_with_browser(&snxlib, true);

    settle(
        &mut app,
        Message::Library(LibraryMessage::BrowserDeleteRowRequest {
            library_path: snxlib.clone(),
            table: TABLE.to_string(),
            row_id,
        }),
    );
    assert!(
        app.library.library_browsers[&snxlib]
            .delete_confirm
            .is_none()
    );
    assert_eq!(row_count(&app, &snxlib), 1);
}

#[test]
fn opening_a_stock_preview_resolves_symbol_and_footprint_without_a_tab_switch() {
    let stock =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library/Stock.snxlib");
    let (mut app, _boot) = Signex::new();
    app.library.stock_library_root = Some(stock.clone());
    app.library
        .open_library_read_only(stock.clone())
        .expect("mount stock");
    let (_, resistor) = app
        .library
        .all_components()
        .into_iter()
        .find(|(_, c)| c.internal_pn.as_str() == "STOCK-R")
        .expect("STOCK-R");
    let table = app
        .library
        .table_of_row(&stock, resistor.row_id)
        .expect("table");
    let row_id = RowId::from_uuid(resistor.row_id);

    settle(
        &mut app,
        Message::Library(LibraryMessage::OpenComponentRow {
            library_path: stock.clone(),
            table: table.clone(),
            row_id,
        }),
    );

    let preview = &app.library.editors[&EditorAddress::new(stock, table, row_id)];
    assert!(preview.symbol.is_some(), "symbol pane resolved on open");
    assert!(
        preview.footprint.is_some(),
        "footprint pane resolved on open"
    );
}
