//! The grid's derived data is recomputed when, and only when, its inputs
//! change.

use std::collections::HashMap;
use std::path::PathBuf;

use signex_library::{
    ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
    ParamMap, PlmReserved, PrimitiveRef,
};
use signex_types::theme::{ThemeId, theme_tokens};
use uuid::Uuid;

use super::counters::BUILDS;
use crate::library::state::{BrowserSort, LibraryBrowserState, LibraryDisplaySettings};
use crate::library::state::{LibraryState, OpenLibrary};

const TABLE: &str = "Resistors";

fn row(pn: &str) -> ComponentRow {
    let now = chrono::Utc::now();
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::new_v4(),
        internal_pn: InternalPn::new(pn),
        class: ComponentClass::generic(),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        description: String::new(),
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
}

fn fixture() -> (LibraryState, PathBuf) {
    let root = PathBuf::from("/tmp/view-cache/Cache.snxlib");
    let mut state = LibraryState::default();
    state.open_libraries.push(OpenLibrary {
        root: root.clone(),
        display_name: "Cache".into(),
        library_id: Uuid::new_v4(),
        tables: HashMap::from([(TABLE.to_string(), vec![row("R3"), row("R1"), row("R2")])]),
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
    state.touch_data();
    let mut browser = LibraryBrowserState::new(root.clone());
    browser.active_table = Some(TABLE.to_string());
    state.library_browsers.insert(root.clone(), browser);
    (state, root)
}

fn builds() -> usize {
    BUILDS.with(|c| c.get())
}

fn browser<'a>(state: &'a LibraryState, root: &PathBuf) -> &'a LibraryBrowserState {
    &state.library_browsers[root]
}

fn visible(state: &LibraryState, root: &PathBuf) -> Vec<usize> {
    browser(state, root)
        .view_cache
        .as_ref()
        .expect("synced")
        .visible
        .clone()
}

fn browser_mut<'a>(state: &'a mut LibraryState, root: &PathBuf) -> &'a mut LibraryBrowserState {
    state.library_browsers.get_mut(root).expect("browser")
}

#[test]
fn syncing_with_unchanged_inputs_recomputes_nothing() {
    let (mut state, root) = fixture();
    state.sync_view_caches();
    let after_first = builds();
    assert_eq!(visible(&state, &root), [0, 1, 2]);

    for _ in 0..3 {
        state.sync_view_caches();
    }

    assert_eq!(builds(), after_first);
}

#[test]
fn drawing_frames_with_a_synced_cache_computes_nothing() {
    let (mut state, root) = fixture();
    state.sync_view_caches();
    let after_sync = builds();
    let tokens = theme_tokens(ThemeId::Signex);

    for _ in 0..3 {
        drop(crate::library::browser::view(
            &root,
            &state,
            browser(&state, &root),
            &tokens,
        ));
    }

    assert_eq!(builds(), after_sync, "view reused the cache");
}

#[test]
fn a_view_without_a_current_cache_computes_for_that_frame_only() {
    let (state, root) = fixture();
    let before = builds();
    let tokens = theme_tokens(ThemeId::Signex);

    drop(crate::library::browser::view(
        &root,
        &state,
        browser(&state, &root),
        &tokens,
    ));

    assert_eq!(builds(), before + 1, "a miss recomputes, and never panics");
    assert!(
        browser(&state, &root).view_cache.is_none(),
        "view stores nothing"
    );
}

#[test]
fn a_filter_change_rebuilds_the_visible_rows() {
    let (mut state, root) = fixture();
    state.sync_view_caches();
    let before = builds();

    browser_mut(&mut state, &root).search = "r1".into();
    state.sync_view_caches();

    assert_eq!(builds(), before + 1);
    assert_eq!(visible(&state, &root), [1]);
}

#[test]
fn a_sort_change_rebuilds_and_orders_the_rows() {
    let (mut state, root) = fixture();
    state.sync_view_caches();
    let before = builds();

    browser_mut(&mut state, &root).sort_by = Some(BrowserSort {
        key: "internal_pn".into(),
        descending: false,
    });
    state.sync_view_caches();

    assert_eq!(builds(), before + 1);
    assert_eq!(visible(&state, &root), [1, 2, 0], "R1, R2, R3");

    browser_mut(&mut state, &root).toggle_sort("internal_pn".into());
    state.sync_view_caches();
    assert_eq!(builds(), before + 2);
    assert_eq!(visible(&state, &root), [0, 2, 1], "R3, R2, R1");
}

#[test]
fn a_change_of_the_cached_rows_rebuilds() {
    let (mut state, root) = fixture();
    state.sync_view_caches();
    let before = builds();

    state.open_libraries[0]
        .tables
        .get_mut(TABLE)
        .expect("table")
        .push(row("R4"));
    state.touch_data();
    state.sync_view_caches();

    assert_eq!(builds(), before + 1);
    assert_eq!(visible(&state, &root), [0, 1, 2, 3]);
}

#[test]
fn a_new_clock_tick_rebuilds_because_price_ages_depend_on_it() {
    let (mut state, root) = fixture();
    state.sync_view_caches();
    let before = builds();

    state.clock += chrono::Duration::seconds(2);
    state.sync_view_caches();

    assert_eq!(builds(), before + 1);
    assert_eq!(visible(&state, &root), [0, 1, 2]);
}

#[test]
fn selecting_a_row_or_editing_a_cell_does_not_rebuild() {
    let (mut state, root) = fixture();
    state.sync_view_caches();
    let before = builds();

    let browser = browser_mut(&mut state, &root);
    browser.selected_row = Some(signex_library::RowId::from_uuid(Uuid::new_v4()));
    browser.cell_edit.insert(
        (
            signex_library::RowId::from_uuid(Uuid::new_v4()),
            "mpn".into(),
        ),
        "x".into(),
    );
    state.sync_view_caches();

    assert_eq!(builds(), before);
}
