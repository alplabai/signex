//! The picker list is recomputed when, and only when, its inputs change.

use super::picker_cache::counters::{ENTRY_BUILDS, FILTER_PASSES};
use super::*;

fn summary(pn: &str, mpn: &str, description: &str) -> ComponentSummary {
    ComponentSummary {
        row_id: Uuid::now_v7(),
        internal_pn: signex_library::InternalPn::new(pn),
        mpn: mpn.into(),
        state: signex_library::LifecycleState::Draft,
        description: description.into(),
    }
}

fn library(root: &str, components: Vec<ComponentSummary>) -> OpenLibrary {
    OpenLibrary {
        root: PathBuf::from(root),
        display_name: root.into(),
        library_id: Uuid::now_v7(),
        tables: HashMap::new(),
        cached_components: components,
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        pricing: Default::default(),
        alternate_supply: Default::default(),
        own_edit_stamp: None,
        display: LibraryDisplaySettings::default(),
        provenance: Default::default(),
    }
}

fn state_with_picker() -> LibraryState {
    let mut state = LibraryState::default();
    state.open_libraries.push(library(
        "/tmp/a.snxlib",
        vec![
            summary("R-10K", "RC0805", "Resistor 10k"),
            summary("C-100N", "GRM21", "Capacitor 100n"),
        ],
    ));
    state.touch_data();
    state.picker = Some(PickerState::default());
    state
}

fn builds() -> (usize, usize) {
    (
        ENTRY_BUILDS.with(|c| c.get()),
        FILTER_PASSES.with(|c| c.get()),
    )
}

fn visible_pns(state: &LibraryState) -> Vec<String> {
    state
        .picker
        .as_ref()
        .expect("picker open")
        .rows
        .visible()
        .map(|e| e.summary.internal_pn.as_str().to_string())
        .collect()
}

fn set_filter(state: &mut LibraryState, filter: &str) {
    state.picker.as_mut().expect("picker open").filter = filter.into();
}

#[test]
fn the_picker_filter_uses_the_search_fold_and_refolds_on_a_language_change() {
    let mut state = LibraryState::default();
    state.open_libraries.push(library(
        "/tmp/a.snxlib",
        vec![
            summary("A", "STRAßE", ""),
            summary("B", "ISPARTA", ""),
            summary("C", "İSTANBUL", ""),
        ],
    ));
    state.touch_data();
    state.picker = Some(PickerState::default());
    set_filter(&mut state, "strasse");
    state.sync_view_caches();
    assert_eq!(visible_pns(&state), ["A"], "`ß` folds to `ss`");
    let tr = signex_i18n::I18n::with_message_locale_for_test("tr");
    set_filter(&mut state, "ısparta");
    state.sync_view_caches_in(&tr);
    assert_eq!(visible_pns(&state), ["B"], "tr: `ı` finds `I`");
    state.sync_view_caches();
    assert!(
        visible_pns(&state).is_empty(),
        "back in en-US the keys refold"
    );
}

#[test]
fn syncing_with_unchanged_inputs_recomputes_nothing() {
    let mut state = state_with_picker();
    state.sync_view_caches();
    let after_first = builds();
    assert_eq!(visible_pns(&state), ["R-10K", "C-100N"]);

    for _ in 0..3 {
        state.sync_view_caches();
    }

    assert_eq!(builds(), after_first, "a frame's worth of syncs is free");
}

#[test]
fn a_new_query_refilters_but_does_not_rebuild_the_entries() {
    let mut state = state_with_picker();
    state.sync_view_caches();
    let (entries, filters) = builds();

    set_filter(&mut state, "  CAPACITOR ");
    state.sync_view_caches();

    assert_eq!(
        visible_pns(&state),
        ["C-100N"],
        "matches case-insensitively"
    );
    assert_eq!(builds(), (entries, filters + 1));
}

#[test]
fn a_query_that_normalises_to_the_same_needle_is_not_refiltered() {
    let mut state = state_with_picker();
    set_filter(&mut state, "rc0805");
    state.sync_view_caches();
    let after_first = builds();

    set_filter(&mut state, " RC0805 ");
    state.sync_view_caches();

    assert_eq!(builds(), after_first);
}

#[test]
fn a_change_of_the_component_set_rebuilds_the_list() {
    let mut state = state_with_picker();
    state.sync_view_caches();
    let (entries, filters) = builds();

    state.open_libraries[0]
        .cached_components
        .push(summary("L-1U", "LQ", "Inductor"));
    state.touch_data();
    state.sync_view_caches();

    assert_eq!(visible_pns(&state), ["R-10K", "C-100N", "L-1U"]);
    assert_eq!(builds(), (entries + 1, filters + 1));
}

#[test]
fn an_unrelated_change_keeps_the_list() {
    let mut state = state_with_picker();
    state.sync_view_caches();
    let after_first = builds();

    // Selecting a row, editing the panel search, a new clock tick: none of
    // these touch the component set.
    state.panel_search = "something".into();
    state.picker.as_mut().expect("picker").selected = None;
    state.sync_view_caches();

    assert_eq!(builds(), after_first);
}

#[test]
fn the_filtered_list_matches_the_old_per_frame_filter() {
    let mut state = state_with_picker();
    for (needle, expected) in [
        ("", vec!["R-10K", "C-100N"]),
        ("10k", vec!["R-10K"]),
        ("GRM", vec!["C-100N"]),
        ("zzz", vec![]),
    ] {
        set_filter(&mut state, needle);
        state.sync_view_caches();
        assert_eq!(visible_pns(&state), expected, "filter {needle:?}");
    }
}

/// A component found only by the description of its row (#1474).
#[test]
fn the_picker_filter_matches_a_row_description() {
    let mut state = state_with_picker();
    state.open_libraries[0].cached_components[0].description = "Precision shunt".into();
    state.touch_data();
    set_filter(&mut state, "shunt");
    state.sync_view_caches();
    assert_eq!(visible_pns(&state), ["R-10K"]);
}

/// The header's scope note counts every listed part and the libraries they
/// come from, whatever the query (mockup row 129).
#[test]
fn the_scope_counts_follow_the_component_set_not_the_query() {
    let mut state = state_with_picker();
    state.open_libraries.push(library(
        "/tmp/b.snxlib",
        vec![summary("L-1U", "LQ", "Inductor")],
    ));
    // An open library with no parts adds nothing to the scope.
    state
        .open_libraries
        .push(library("/tmp/empty.snxlib", Vec::new()));
    state.touch_data();
    state.sync_view_caches();
    let rows = &state.picker.as_ref().expect("picker").rows;
    assert_eq!((rows.library_count(), rows.part_count()), (2, 3));

    set_filter(&mut state, "inductor");
    state.sync_view_caches();
    let rows = &state.picker.as_ref().expect("picker").rows;
    assert_eq!(visible_pns(&state), ["L-1U"]);
    assert_eq!((rows.library_count(), rows.part_count()), (2, 3));
}

/// ↑ and ↓ walk the listed rows and stop at the ends; with nothing listed
/// selected, ↓ starts at the first row and ↑ at the last (mockup row 129).
#[test]
fn arrow_steps_walk_the_listed_rows_and_stop_at_the_ends() {
    let mut state = state_with_picker();
    state.open_libraries[0]
        .cached_components
        .push(summary("L-1U", "LQ", "Inductor"));
    state.touch_data();
    state.sync_view_caches();
    let rows = &state.picker.as_ref().expect("picker").rows;
    let pn = |entry: Option<(usize, &PickerEntry)>| {
        entry.map(|(_, e)| e.summary.internal_pn.as_str().to_string())
    };
    let at = |i: usize| {
        let e = rows.visible().nth(i).expect("row");
        Some((&*e.path, e.summary.row_id))
    };
    assert_eq!(pn(rows.stepped(None, 1)).as_deref(), Some("R-10K"));
    assert_eq!(pn(rows.stepped(None, -1)).as_deref(), Some("L-1U"));
    assert_eq!(rows.stepped(at(0), 1).map(|(i, _)| i), Some(1));
    assert_eq!(pn(rows.stepped(at(0), 1)).as_deref(), Some("C-100N"));
    assert_eq!(pn(rows.stepped(at(1), -1)).as_deref(), Some("R-10K"));
    assert_eq!(pn(rows.stepped(at(2), 1)).as_deref(), Some("L-1U"));
    assert_eq!(pn(rows.stepped(at(0), -1)).as_deref(), Some("R-10K"));

    // A row the filter hides counts as no selection; an empty list gives
    // nothing.
    let hidden = at(0).map(|(p, id)| (p.to_path_buf(), id)).expect("row");
    set_filter(&mut state, "inductor");
    state.sync_view_caches();
    let rows = &state.picker.as_ref().expect("picker").rows;
    let from = Some((hidden.0.as_path(), hidden.1));
    assert_eq!(pn(rows.stepped(from, 1)).as_deref(), Some("L-1U"));
    set_filter(&mut state, "no such part");
    state.sync_view_caches();
    let rows = &state.picker.as_ref().expect("picker").rows;
    assert!(rows.stepped(None, 1).is_none());
}
