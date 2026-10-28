//! #1355 — the browser grid's scroll window is driven by its scrollable's
//! messages, and selecting a row never disturbs it.

use std::path::PathBuf;

use signex_library::{
    ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
    ParamMap, PlmReserved, PrimitiveRef, RowId,
};
use uuid::Uuid;

use crate::app::Signex;
use crate::library::messages::LibraryMessage;
use crate::library::state::{LibraryBrowserState, LibraryDisplaySettings, OpenLibrary};

const TABLE: &str = "R";

fn app_with_browser() -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    let root = PathBuf::from("/tmp/grid-window-dispatch/Lib.snxlib");
    app.library
        .library_browsers
        .insert(root.clone(), LibraryBrowserState::new(root.clone()));
    (app, root)
}

#[test]
fn a_scroll_report_moves_the_window() {
    let (mut app, root) = app_with_browser();

    let _ = app.route_library_message(LibraryMessage::BrowserGridScrolled {
        library_path: root.clone(),
        offset_y: 4_800.0,
    });
    let _ = app.route_library_message(LibraryMessage::BrowserGridResized {
        library_path: root.clone(),
        viewport_h: 640.0,
    });

    let scroll = app.library.library_browsers[&root].grid_scroll;
    assert_eq!(scroll.offset_y, 4_800.0);
    assert_eq!(scroll.viewport_h, 640.0);
}

#[test]
fn a_resize_keeps_the_offset_and_a_scroll_keeps_the_height() {
    let (mut app, root) = app_with_browser();
    let _ = app.route_library_message(LibraryMessage::BrowserGridScrolled {
        library_path: root.clone(),
        offset_y: 300.0,
    });
    let _ = app.route_library_message(LibraryMessage::BrowserGridResized {
        library_path: root.clone(),
        viewport_h: 900.0,
    });
    let _ = app.route_library_message(LibraryMessage::BrowserGridScrolled {
        library_path: root.clone(),
        offset_y: 320.0,
    });

    let scroll = app.library.library_browsers[&root].grid_scroll;
    assert_eq!((scroll.offset_y, scroll.viewport_h), (320.0, 900.0));
}

#[test]
fn a_browser_coming_to_the_front_starts_at_the_top() {
    let (mut app, root) = app_with_browser();
    app.library
        .library_browsers
        .get_mut(&root)
        .unwrap()
        .grid_scroll
        .offset_y = 4_800.0;

    app.library.note_shown_browser(Some(&root));
    assert_eq!(
        app.library.library_browsers[&root].grid_scroll.offset_y,
        0.0
    );

    // Still showing: scrolling is left alone.
    app.library
        .library_browsers
        .get_mut(&root)
        .unwrap()
        .grid_scroll
        .offset_y = 120.0;
    app.library.note_shown_browser(Some(&root));
    assert_eq!(
        app.library.library_browsers[&root].grid_scroll.offset_y,
        120.0
    );

    // Away and back resets it again.
    app.library.note_shown_browser(None);
    app.library.note_shown_browser(Some(&root));
    assert_eq!(
        app.library.library_browsers[&root].grid_scroll.offset_y,
        0.0
    );
}

/// An app whose library holds `n` rows in one table, a browser on it with
/// a measured viewport at the top, and the row ids in table order.
fn app_with_rows(n: usize) -> (Signex, PathBuf, Vec<RowId>) {
    let (mut app, root) = app_with_browser();
    let rows: Vec<ComponentRow> = (0..n)
        .map(|i| {
            let now = chrono::Utc::now();
            ComponentRow {
                alternate_symbols: Vec::new(),
                row_id: Uuid::new_v4(),
                internal_pn: InternalPn::new(format!("R{i:06}")),
                class: ComponentClass::generic(),
                datasheet: DatasheetRef::default(),
                datasheet_label: String::new(),
                more_datasheets: Vec::new(),
                state: LifecycleState::Draft,
                symbol_ref: PrimitiveRef::new(Uuid::nil(), Uuid::nil()),
                footprint_ref: None,
                sim_ref: None,
                pin_map_overrides: Vec::new(),
                primary_mpn: ManufacturerPart::draft("Acme", "MPN"),
                alternates: Vec::new(),
                supply: Vec::new(),
                parameters: ParamMap::new(),
                plm: PlmReserved::default(),
                version: "0.0.1".into(),
                released: false,
                symbol_version: String::new(),
                footprint_version: String::new(),
                sim_version: String::new(),
                created: now,
                updated: now,
                content_hash: [0u8; 32],
                server_revision: None,
            }
        })
        .collect();
    let ids = rows.iter().map(|r| RowId::from_uuid(r.row_id)).collect();
    app.library.open_libraries.push(OpenLibrary {
        root: root.clone(),
        display_name: "Window".into(),
        library_id: Uuid::new_v4(),
        tables: std::collections::HashMap::from([(TABLE.to_string(), rows)]),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        pricing: Default::default(),
        own_edit_stamp: None,
        display: LibraryDisplaySettings::default(),
    });
    app.library.touch_data();
    let browser = app.library.library_browsers.get_mut(&root).unwrap();
    browser.active_table = Some(TABLE.to_string());
    let _ = app.route_library_message(LibraryMessage::BrowserGridScrolled {
        library_path: root.clone(),
        offset_y: 0.0,
    });
    let _ = app.route_library_message(LibraryMessage::BrowserGridResized {
        library_path: root.clone(),
        viewport_h: 640.0,
    });
    (app, root, ids)
}

#[test]
fn revealing_a_new_row_far_down_a_long_table_scrolls_to_it() {
    let (mut app, root, ids) = app_with_rows(10_000);
    let target = 9_000;
    app.library
        .library_browsers
        .get_mut(&root)
        .unwrap()
        .selected_row = Some(ids[target]);

    let task = app.reveal_selected_browser_row(&root);

    assert!(task.units() > 0, "a scroll task must be issued");
    let scroll = app.library.library_browsers[&root].grid_scroll;
    assert!(scroll.offset_y > 0.0, "the window must follow the scroll");
    let range = crate::library::browser::visible_rows(scroll, 10_000);
    assert!(range.contains(&target), "{range:?} misses row {target}");
}

#[test]
fn revealing_a_row_already_in_view_does_nothing() {
    let (mut app, root, ids) = app_with_rows(10_000);
    app.library
        .library_browsers
        .get_mut(&root)
        .unwrap()
        .selected_row = Some(ids[1]);

    let task = app.reveal_selected_browser_row(&root);

    assert_eq!(task.units(), 0);
    assert_eq!(
        app.library.library_browsers[&root].grid_scroll.offset_y,
        0.0
    );
}
