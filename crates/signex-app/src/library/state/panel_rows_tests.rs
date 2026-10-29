//! The Components Panel row list is recomputed when, and only when, its
//! inputs change (#1353).

use super::panel_rows::counters::{FILTER_PASSES, KEY_BUILDS};
use super::tests::fixture_row;
use super::*;

const ROOT: &str = "/tmp/a.snxlib";

fn library(name: &str, tables: Vec<(&str, Vec<ComponentRow>)>) -> OpenLibrary {
    OpenLibrary {
        root: PathBuf::from(ROOT),
        display_name: name.into(),
        library_id: Uuid::now_v7(),
        tables: tables.into_iter().map(|(n, r)| (n.to_owned(), r)).collect(),
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
    }
}

fn row(pn: &str, mpn: &str, mfr: &str) -> ComponentRow {
    let mut r = fixture_row(pn);
    r.primary_mpn = signex_library::ManufacturerPart::draft(mfr, mpn);
    r
}

fn state() -> LibraryState {
    let mut state = LibraryState::default();
    state.open_libraries.push(library(
        "passives",
        vec![
            ("res", vec![row("R-10K", "ERA-3AED", "Panasonic")]),
            ("cap", vec![row("C-100N", "GRM21", "Murata")]),
        ],
    ));
    state.touch_data();
    state
}

fn shown(state: &LibraryState) -> Vec<(String, usize)> {
    state
        .components_panel
        .rows
        .visible_rows(Path::new(ROOT), usize::MAX)
        .map(|(t, i)| (t.to_owned(), i))
        .collect()
}

fn set_filter(state: &mut LibraryState, filter: &str) {
    state.components_panel.filter = filter.into();
    state.sync_view_caches();
}

#[test]
fn empty_filter_lists_every_row_in_table_name_order() {
    let mut s = state();
    s.sync_view_caches();
    assert_eq!(shown(&s), vec![("cap".into(), 0), ("res".into(), 0)]);
}

#[test]
fn filter_matches_mpn_manufacturer_and_part_number_case_insensitively() {
    let mut s = state();
    for (query, expect) in [
        ("  ERA ", "res"),
        ("murata", "cap"),
        ("r-10", "res"),
        ("grm", "cap"),
    ] {
        set_filter(&mut s, query);
        assert_eq!(shown(&s), vec![(expect.into(), 0)], "query {query:?}");
    }
    set_filter(&mut s, "zzz");
    assert!(shown(&s).is_empty());
}

#[test]
fn library_name_hit_lists_every_row() {
    let mut s = state();
    set_filter(&mut s, "PASSIVES");
    assert_eq!(shown(&s).len(), 2);
}

#[test]
fn keys_rebuild_only_when_data_changes() {
    let mut s = state();
    s.sync_view_caches();
    let (keys, passes) = (
        KEY_BUILDS.with(|c| c.get()),
        FILTER_PASSES.with(|c| c.get()),
    );

    s.sync_view_caches();
    assert_eq!(KEY_BUILDS.with(|c| c.get()), keys, "nothing changed");
    assert_eq!(FILTER_PASSES.with(|c| c.get()), passes, "nothing changed");

    set_filter(&mut s, "era");
    assert_eq!(
        KEY_BUILDS.with(|c| c.get()),
        keys,
        "a query is not a data change"
    );
    assert_eq!(FILTER_PASSES.with(|c| c.get()), passes + 1);
}

/// A refreshed library must not keep answering from the old keys.
#[test]
fn changed_library_data_invalidates_the_cache() {
    let mut s = state();
    set_filter(&mut s, "newpart");
    assert!(shown(&s).is_empty());

    s.open_libraries[0]
        .tables
        .get_mut("res")
        .unwrap()
        .push(row("R-NEW", "NEWPART-1", "Vishay"));
    s.touch_data();
    s.sync_view_caches();
    assert_eq!(shown(&s), vec![("res".into(), 1)]);

    s.open_libraries.clear();
    s.touch_data();
    s.sync_view_caches();
    assert_eq!(s.components_panel.rows.visible_count(Path::new(ROOT)), 0);
}

#[test]
fn renaming_a_library_refilters_without_a_data_change() {
    let mut s = state();
    set_filter(&mut s, "zzz");
    assert!(shown(&s).is_empty());
    s.open_libraries[0].display_name = "ZZZ lib".into();
    s.sync_view_caches();
    assert_eq!(shown(&s).len(), 2);
}
