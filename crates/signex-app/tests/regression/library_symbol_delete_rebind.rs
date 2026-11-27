//! #2572 — deleting an in-use library symbol counts the component rows it
//! would leave with a broken binding, and Re-bind rows… moves those rows to
//! another symbol first, returning to the confirm with the new count.

use crate::library_place_symbol::{Fixture, app_with_library, fixture};
use crate::library_symbol_missing::{open_container_editor, panel, symbol_count};
use signex_app::app::{Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::library::messages::{LibraryMessage, PrimitivePickerMsg};
use signex_app::library::state::PrimitivePickerTarget;
use signex_app::panels::{PanelMsg, SymbolDeleteRebind, SymbolDeleteWarning, SymbolUsageKey};
use signex_library::{ComponentRow, PrimitiveRef};
use std::path::PathBuf;

fn warning(app: &Signex) -> Option<SymbolDeleteWarning> {
    app.document_view
        .panel_ctx
        .symbol_editor
        .as_ref()?
        .delete_warning
        .clone()
}

fn settle(app: &mut Signex, message: Message) {
    crate::settle_support::settle(app, message);
}

fn rebind(app: &mut Signex, scope: SymbolDeleteRebind) {
    settle(
        app,
        Message::Dock(DockMessage::Panel(PanelMsg::SchLibraryDeleteRebind(scope))),
    );
}

fn pick(app: &mut Signex, symbol: PrimitiveRef) {
    settle(
        app,
        Message::Library(LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Pick(
            symbol,
        ))),
    );
}

/// The fixture's one row, as the library cache holds it; panics if it is gone.
fn the_row(app: &Signex, f: &Fixture) -> ComponentRow {
    app.library.library_at(&f.snxlib).expect("mounted").tables["ICs"]
        .iter()
        .find(|r| r.row_id == f.row_id)
        .cloned()
        .expect("the row is kept")
}

fn row_count(app: &Signex, f: &Fixture) -> usize {
    app.library.library_at(&f.snxlib).expect("mounted").tables["ICs"].len()
}

/// The fixture library with its symbol container open; the container holds
/// the row's symbol and an unused SIBLING to re-bind to.
fn asked() -> (Fixture, Signex, PathBuf, PrimitiveRef) {
    let f = fixture();
    let mut app = app_with_library(&f);
    let path = open_container_editor(&mut app, &f);
    let sibling = app.document_state.symbol_editors[&path].draft.file.symbols[1].uuid;
    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(0));
    let shown = warning(&app).expect("the row uses the symbol");
    assert_eq!(shown.rows, 1, "the confirm counts the row");
    assert_eq!(shown.items(SymbolUsageKey::ComponentRows).len(), 1);
    let sibling = PrimitiveRef::new(f.library_id, sibling);
    (f, app, path, sibling)
}

#[test]
fn re_bind_all_moves_the_rows_and_the_confirm_counts_them_down() {
    let (f, mut app, path, sibling) = asked();
    let before = the_row(&app, &f);
    let rows_before = row_count(&app, &f);

    // A cancelled pick changes nothing.
    rebind(&mut app, SymbolDeleteRebind::AllRows);
    let picker = app
        .library
        .primitive_picker
        .as_ref()
        .expect("the picker opened");
    assert_eq!(picker.kind, signex_library::PrimitiveKind::Symbol);
    let replacing = PrimitiveRef::new(f.library_id, f.symbol_id);
    assert!(
        matches!(
            &picker.target,
            PrimitivePickerTarget::BrowserRows { rows, replacing: r }
                if rows.len() == 1 && rows[0].row_id.as_uuid() == f.row_id && *r == replacing
        ),
        "{:?}",
        picker.target
    );
    settle(
        &mut app,
        Message::Library(LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Cancel)),
    );
    assert_eq!(the_row(&app, &f), before, "nothing written");
    assert_eq!(warning(&app).expect("back to the confirm").rows, 1);

    rebind(&mut app, SymbolDeleteRebind::AllRows);
    pick(&mut app, sibling);

    let shown = warning(&app).expect("back to the confirm with the new count");
    assert_eq!(shown.rows, 0);
    assert!(shown.is_clear(), "{:?}", shown.entries);
    assert_eq!(symbol_count(&app, &path), 2, "no delete without the answer");
    let after = the_row(&app, &f);
    assert_eq!(after.symbol_ref, sibling);
    assert_ne!(before.symbol_version, sibling_version(&app, &path));
    assert_eq!(
        after.symbol_version,
        sibling_version(&app, &path),
        "pinned at the picked symbol's current version"
    );
    assert_eq!(after.internal_pn, before.internal_pn);
    assert_eq!(after.primary_mpn, before.primary_mpn);
    assert_eq!(after.class, before.class);
    assert_eq!(row_count(&app, &f), rows_before, "no row lost");

    panel(&mut app, PanelMsg::SchLibraryDeleteConfirmed(0));
    assert_eq!(symbol_count(&app, &path), 1, "now deleted normally");
    assert!(warning(&app).is_none());
    assert_eq!(
        the_row(&app, &f).symbol_ref,
        sibling,
        "the row keeps its new symbol"
    );
}

/// The SIBLING symbol's current version, as its container file holds it.
fn sibling_version(app: &Signex, path: &PathBuf) -> String {
    app.document_state.symbol_editors[path].draft.file.symbols[1]
        .version
        .clone()
}

/// A Library Browser symbol pick pins the row at the picked symbol's
/// current version too (Caner, 2026-11-27); before, the old pin was kept.
#[test]
fn a_browser_row_symbol_pick_pins_the_symbols_current_version() {
    let f = fixture();
    let mut app = app_with_library(&f);
    let path = open_container_editor(&mut app, &f);
    let sibling = PrimitiveRef::new(
        f.library_id,
        app.document_state.symbol_editors[&path].draft.file.symbols[1].uuid,
    );
    let before = the_row(&app, &f);
    settle(
        &mut app,
        Message::Library(LibraryMessage::OpenPrimitivePicker {
            kind: signex_library::PrimitiveKind::Symbol,
            target: PrimitivePickerTarget::BrowserRow(
                signex_app::library::state::EditorAddress::new(
                    f.snxlib.clone(),
                    "ICs".into(),
                    signex_library::RowId::from_uuid(f.row_id),
                ),
            ),
        }),
    );
    pick(&mut app, sibling);

    let after = the_row(&app, &f);
    assert_eq!(after.symbol_ref, sibling);
    assert_ne!(before.symbol_version, sibling_version(&app, &path));
    assert_eq!(after.symbol_version, sibling_version(&app, &path));
}

#[test]
fn a_rows_own_re_bind_moves_that_row() {
    let (f, mut app, path, sibling) = asked();
    rebind(&mut app, SymbolDeleteRebind::Row(0));
    pick(&mut app, sibling);

    assert_eq!(the_row(&app, &f).symbol_ref, sibling);
    let shown = warning(&app).expect("back to the confirm");
    assert_eq!(shown.rows, 0);
    assert!(shown.items(SymbolUsageKey::ComponentRows).is_empty());
    assert_eq!(symbol_count(&app, &path), 2);
}
