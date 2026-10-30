//! `refresh_primitive_cache_for` runs right after a primitive
//! save (its Task is run to completion here), on caches that are known-good. #599: it overwrote all three
//! with `unwrap_or_default()`, so a listing failure erased them — the
//! user saved a symbol and watched every symbol, footprint and sim
//! vanish from that library's picker while the file sat fine on disk.

use std::path::PathBuf;

use uuid::Uuid;

use crate::app::Signex;
use crate::library::state::{LibraryDisplaySettings, OpenLibrary};
use crate::library::test_adapters::{
    CountingAdapter, FailingListingAdapter, ScriptedSymbolsAdapter, cached_summary,
};

/// A mounted library rooted at `<tmp>/<unique>/lib.snxlib` whose three
/// primitive caches already hold one entry each, plus the path of a
/// primitive just saved inside it.
fn app_with_cached_library() -> (Signex, PathBuf) {
    let library_id = Uuid::now_v7();
    app_with_cached_library_over(Box::new(FailingListingAdapter::new(library_id)), library_id)
}

/// [`app_with_cached_library`] over the adapter the test chooses.
fn app_with_cached_library_over(
    adapter: Box<dyn signex_library::LibraryAdapter>,
    library_id: Uuid,
) -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    let dir = std::env::temp_dir().join(format!("snx-editor-cache-{}", Uuid::now_v7().simple()));
    let root = dir.join("lib.snxlib");
    // Bootstrap mounted the bundled stock library; these tests address
    // `open_libraries.first()`, so start from an empty set.
    app.library.open_libraries.clear();
    app.library.set.mount(adapter).expect("mount stub adapter");
    app.library.open_libraries.push(OpenLibrary {
        root: root.clone(),
        display_name: "FailingLib".into(),
        library_id,
        tables: std::collections::HashMap::new(),
        cached_components: Vec::new(),
        cached_symbols: vec![cached_summary("R_0805")],
        cached_footprints: vec![cached_summary("SOIC-8")],
        cached_sims: vec![cached_summary("BC547.lib")],
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        pricing: Default::default(),
        own_edit_stamp: None,
        display: LibraryDisplaySettings::default(),
    });
    let saved = dir.join("symbols").join("just-saved.snxsym");
    (app, saved)
}

#[test]
fn a_post_save_refresh_that_fails_keeps_the_pickers_entries() {
    let (mut app, saved) = app_with_cached_library();

    let task = app.refresh_primitive_cache_for(&saved);
    crate::app::path_ops::settle_task(&mut app, task);

    let lib = app
        .library
        .open_libraries
        .first()
        .expect("library still open");
    assert_eq!(
        lib.cached_symbols.len(),
        1,
        "a failed refresh must not erase the symbols the picker was showing"
    );
    assert_eq!(lib.cached_symbols[0].name, "R_0805");
    assert_eq!(
        lib.cached_footprints.len(),
        1,
        "the save was a symbol — the footprint cache has no business being touched"
    );
    assert_eq!(lib.cached_sims.len(), 1, "same for sims");
}

#[test]
fn a_post_save_refresh_that_fails_reaches_the_messages_panel() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, saved) = app_with_cached_library();
    // Unique display name — the diagnostics ring is global and shared
    // with every other test in this binary.
    let marker = format!("FailingLib-{}", Uuid::now_v7().simple());
    app.library
        .open_libraries
        .first_mut()
        .expect("library still open")
        .display_name = marker.clone();

    let task = app.refresh_primitive_cache_for(&saved);
    crate::app::path_ops::settle_task(&mut app, task);

    let records: Vec<_> = crate::diagnostics::recent_entries()
        .into_iter()
        .filter(|entry| entry.message.contains(&marker))
        .collect();
    assert_eq!(
        records.len(),
        3,
        "the save looked complete and the picker went stale — say so, once per listing"
    );
    assert!(
        records
            .iter()
            .all(|entry| entry.level == crate::diagnostics::DiagnosticLevel::Warning),
        "the file is on disk and the previous cache stands: degraded, not lost"
    );
}

#[test]
fn a_post_save_refresh_lists_nothing_inside_update() {
    let library_id = Uuid::now_v7();
    let (adapter, calls) = CountingAdapter::new(library_id);
    let (mut app, saved) = app_with_cached_library_over(Box::new(adapter), library_id);

    let task = app.refresh_primitive_cache_for(&saved);

    assert_eq!(
        calls.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "update only plans the refresh; the listings belong to the worker"
    );
    crate::app::path_ops::settle_task(&mut app, task);
    assert!(
        calls.load(std::sync::atomic::Ordering::SeqCst) > 0,
        "the worker lists the library"
    );
}

#[test]
fn a_post_save_refresh_lands_the_new_listing_when_the_task_has_run() {
    let library_id = Uuid::now_v7();
    let (adapter, symbols) = ScriptedSymbolsAdapter::new(library_id);
    let (mut app, saved) = app_with_cached_library_over(Box::new(adapter), library_id);
    *symbols.lock().expect("symbols lock") = vec![cached_summary("JustSaved")];

    let task = app.refresh_primitive_cache_for(&saved);
    let before: Vec<_> = app.library.open_libraries[0]
        .cached_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    crate::app::path_ops::settle_task(&mut app, task);
    let after: Vec<_> = app.library.open_libraries[0]
        .cached_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();

    assert_eq!(
        before,
        ["R_0805"],
        "nothing is applied before the read lands"
    );
    assert_eq!(after, ["JustSaved"]);
}

#[test]
fn a_post_save_refresh_superseded_before_it_lands_is_dropped() {
    let library_id = Uuid::now_v7();
    let (adapter, symbols) = ScriptedSymbolsAdapter::new(library_id);
    let (mut app, saved) = app_with_cached_library_over(Box::new(adapter), library_id);
    *symbols.lock().expect("symbols lock") = vec![cached_summary("Stale")];
    let task = app.refresh_primitive_cache_for(&saved);
    let root = app.library.open_libraries[0].root.clone();
    // A newer refresh of the same library (here a synchronous one) won.
    app.library.supersede_refreshes(&root);

    crate::app::path_ops::settle_task(&mut app, task);

    let names: Vec<_> = app.library.open_libraries[0]
        .cached_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert_eq!(names, ["R_0805"], "a superseded read changes nothing");
}

#[test]
fn a_post_save_refresh_for_a_library_closed_before_it_lands_is_dropped() {
    let library_id = Uuid::now_v7();
    let (adapter, _symbols) = ScriptedSymbolsAdapter::new(library_id);
    let (mut app, saved) = app_with_cached_library_over(Box::new(adapter), library_id);
    let task = app.refresh_primitive_cache_for(&saved);
    app.library.open_libraries.clear();

    crate::app::path_ops::settle_task(&mut app, task);

    assert!(
        app.library.open_libraries.is_empty(),
        "nothing is re-opened"
    );
}

#[test]
fn a_post_save_refresh_asks_for_the_drift_check_even_when_the_read_fails() {
    let (mut app, saved) = app_with_cached_library();
    app.library.drift.check_pending = false;

    let task = app.refresh_primitive_cache_for(&saved);

    assert!(
        app.library.drift.check_pending,
        "a saved primitive may be the symbol an open sheet embeds"
    );
    crate::app::path_ops::settle_task(&mut app, task);
}

#[test]
fn a_save_outside_every_library_starts_no_refresh() {
    let (mut app, _saved) = app_with_cached_library();
    app.library.drift.check_pending = false;

    let task = app.refresh_primitive_cache_for(std::path::Path::new("/elsewhere/a.snxsym"));
    crate::app::path_ops::settle_task(&mut app, task);

    assert!(!app.library.drift.check_pending);
}
