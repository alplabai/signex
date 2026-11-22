//! m118 Library Browser, and m119 to m121, the one Library panel that took
//! over the SCH Library and Footprint Library panels (2026-11-19 decision).

use signex_app::app::{Message, OverlayMsg, TabKind};
use signex_app::library::messages::{LibraryMessage, PickerMsg};
use signex_app::library::panel::{kind_counts, shown_kind};
use signex_app::library::state::{LibraryPanelKind, LifecycleFilter};
use signex_app::panels::PanelKind;
use signex_library::RowId;

use super::support::{LibRig, TABLE, library_rig, part_number, read_only_rig, symbol_rig};

fn cell_edit(r: &mut LibRig, row_id: RowId, column: &str, value: &str) {
    r.send(LibraryMessage::BrowserCellEdit {
        library_path: r.root.clone(),
        row_id,
        column: column.to_string(),
        value: value.to_string(),
    });
}

fn cell_commit(r: &mut LibRig, row_id: RowId, column: &str) {
    r.run(LibraryMessage::BrowserCellCommit {
        library_path: r.root.clone(),
        table: TABLE.to_string(),
        row_id,
        column: column.to_string(),
    });
}

#[test]
fn opening_a_library_shows_its_browser_tab_on_its_table() {
    let r = library_rig(2);
    let tab = &r.app.document_state.tabs[r.app.document_state.active_tab];
    assert!(matches!(&tab.kind, TabKind::LibraryBrowser(path) if *path == r.root));
    assert_eq!(r.browser().active_table.as_deref(), Some(TABLE));
    assert_eq!(r.row_count(), 2);
}

#[test]
fn typing_a_search_filters_the_view_and_writes_no_library_file() {
    let mut r = library_rig(2);
    let before = r.files();

    r.send(LibraryMessage::BrowserSearchChanged {
        library_path: r.root.clone(),
        value: "R-0001".to_string(),
    });

    assert_eq!(r.browser().search, "R-0001");
    assert_eq!(r.files(), before);
}

#[test]
fn a_click_on_a_row_selects_it_for_the_preview() {
    let mut r = library_rig(2);
    let id = r.rows[1];
    r.select(id);
    assert_eq!(r.browser().selected_row, Some(id));
}

#[test]
fn changing_the_lifecycle_filter_drops_the_selection() {
    let mut r = library_rig(2);
    let id = r.rows[0];
    r.select(id);

    r.send(LibraryMessage::BrowserSetLifecycleFilter {
        library_path: r.root.clone(),
        filter: LifecycleFilter::All,
    });

    assert_eq!(r.browser().lifecycle_filter, LifecycleFilter::All);
    assert_eq!(r.browser().selected_row, None);
}

#[test]
fn a_second_click_on_a_class_clears_its_filter() {
    let mut r = library_rig(1);
    for expected in [Some("Resistor"), None] {
        r.send(LibraryMessage::BrowserClassFilterClicked {
            library_path: r.root.clone(),
            key: "Resistor".to_string(),
        });
        assert_eq!(r.browser().class_filter.as_deref(), expected);
    }
}

#[test]
fn a_column_header_click_sorts_the_grid() {
    let mut r = library_rig(2);
    assert!(r.browser().sort_by.is_none());
    r.send(LibraryMessage::BrowserSortColumn {
        library_path: r.root.clone(),
        column_key: "mpn".to_string(),
    });
    assert!(r.browser().sort_by.is_some());
}

#[test]
fn committing_a_grid_cell_writes_the_row() {
    let mut r = library_rig(2);
    let id = r.rows[0];
    let before = r.files();

    cell_edit(&mut r, id, "mpn", "RC0402FR-0710KL");
    assert_eq!(r.files(), before, "typing writes nothing");
    cell_commit(&mut r, id, "mpn");

    let row = r.row(id).expect("the row");
    assert_eq!(row.primary_mpn.mpn, "RC0402FR-0710KL");
    assert_ne!(r.files(), before, "the commit is on disk");
    assert!(r.browser().cell_edit.is_empty(), "the buffer is done");
}

#[test]
fn escape_in_a_grid_cell_leaves_the_row_and_files_unchanged() {
    let mut r = library_rig(1);
    let id = r.rows[0];
    let before = r.files();

    cell_edit(&mut r, id, "mpn", "typed");
    r.send(LibraryMessage::BrowserCellCancel {
        library_path: r.root.clone(),
        row_id: id,
        column: "mpn".to_string(),
    });

    assert!(r.browser().cell_edit.is_empty());
    assert_eq!(r.row(id).expect("row").primary_mpn.mpn, "MPN-0");
    assert_eq!(r.files(), before);
}

/// Inference from the 2026-11-19 decision: one clash rule, shared with the
/// backends' refusal, also holds for the grid's Internal PN cell.
#[test]
fn a_grid_cell_cannot_take_the_internal_pn_of_another_row() {
    let mut r = library_rig(2);
    let id = r.rows[1];

    cell_edit(&mut r, id, "internal_pn", &part_number(0));
    cell_commit(&mut r, id, "internal_pn");

    assert_eq!(
        r.row(id).expect("row").internal_pn.as_str(),
        part_number(1),
        "the stored number is kept"
    );
    assert!(
        !r.browser().cell_edit.is_empty(),
        "what was typed stays in the cell"
    );
}

#[test]
fn a_measurement_cell_refuses_an_inline_edit() {
    let mut r = library_rig(1);
    let id = r.rows[0];
    let before = r.files();
    let column = format!("parameters.{}", super::support::RATING);

    cell_edit(&mut r, id, &column, "10");
    cell_commit(&mut r, id, &column);

    assert_eq!(r.files(), before, "the stored value is kept");
}

#[test]
fn add_component_appends_a_row_and_selects_it() {
    let mut r = library_rig(1);
    r.run(LibraryMessage::BrowserAddComponent {
        library_path: r.root.clone(),
        table: Some(TABLE.to_string()),
    });

    assert_eq!(r.row_count(), 2);
    let selected = r.browser().selected_row.expect("the new row is selected");
    assert_ne!(selected, r.rows[0]);
}

#[test]
fn a_read_only_library_refuses_a_grid_edit_and_says_why() {
    let mut r = read_only_rig(1);
    let id = r.rows[0];
    let before = r.files();

    cell_edit(&mut r, id, "mpn", "typed");

    assert!(r.browser().cell_edit.is_empty());
    assert!(r.app.document_state.error_notice.is_some());
    assert_eq!(r.files(), before);
}

#[test]
fn the_panel_opens_on_parts_outside_an_editor_and_a_chip_switches_it() {
    let mut r = library_rig(3);
    let shown = |r: &LibRig| shown_kind(&r.app.library, &r.app.document_state.panel_ctx);
    assert_eq!(shown(&r), LibraryPanelKind::Parts);
    let counts = kind_counts(&r.app.library, &r.app.document_state.panel_ctx);
    assert_eq!(counts.parts, 3);

    r.send(LibraryMessage::Picker(PickerMsg::PanelKindPicked(
        LibraryPanelKind::Symbols,
    )));

    assert_eq!(shown(&r), LibraryPanelKind::Symbols);
}

#[test]
fn the_panel_opens_on_symbols_in_the_symbol_editor() {
    let r = symbol_rig();
    let ctx = &r.app.document_state.panel_ctx;
    assert_eq!(shown_kind(&r.app.library, ctx), LibraryPanelKind::Symbols);
    assert_eq!(kind_counts(&r.app.library, ctx).symbols, 1);
}

#[test]
fn the_sch_library_and_footprint_library_rows_open_the_one_library_panel() {
    for (alias, kind) in [
        (PanelKind::SchLibrary, LibraryPanelKind::Symbols),
        (PanelKind::FootprintLibrary, LibraryPanelKind::Footprints),
    ] {
        let mut r = library_rig(1);

        let _ = r.app.update(Message::Overlay(OverlayMsg::OpenPanel(alias)));

        let dock = &r.app.document_state.dock;
        assert!(dock.locate(PanelKind::Library).is_some(), "{alias:?}");
        assert!(dock.locate(alias).is_none(), "{alias:?} is never docked");
        assert_eq!(
            shown_kind(&r.app.library, &r.app.document_state.panel_ctx),
            kind
        );
    }
}

#[test]
fn the_panel_filter_is_view_state_and_writes_nothing() {
    let mut r = library_rig(2);
    let before = r.files();

    r.send(LibraryMessage::Picker(PickerMsg::PanelSearchChanged(
        "R-0001".to_string(),
    )));

    assert_eq!(r.app.library.panel_search, "R-0001");
    assert_eq!(r.files(), before);
}
