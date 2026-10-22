//! `refresh_primitive_cache_for` runs immediately after a primitive
//! save, on caches that are known-good. #599: it overwrote all three
//! with `unwrap_or_default()`, so a listing failure erased them — the
//! user saved a symbol and watched every symbol, footprint and sim
//! vanish from that library's picker while the file sat fine on disk.

use std::path::PathBuf;

use uuid::Uuid;

use crate::app::Signex;
use crate::library::state::{LibraryDisplaySettings, OpenLibrary};
use crate::library::test_adapters::{FailingListingAdapter, cached_summary};

/// A mounted library rooted at `<tmp>/<unique>/lib.snxlib` whose three
/// primitive caches already hold one entry each, plus the path of a
/// primitive just saved inside it.
fn app_with_cached_library() -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    let library_id = Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("snx-editor-cache-{}", Uuid::new_v4().simple()));
    let root = dir.join("lib.snxlib");
    app.library
        .set
        .mount(Box::new(FailingListingAdapter::new(library_id)))
        .expect("mount stub adapter");
    app.library.open_libraries.push(OpenLibrary {
        root: root.clone(),
        display_name: "FailingLib".into(),
        library_id,
        tables: std::collections::HashMap::new(),
        cached_components: Vec::new(),
        cached_symbols: vec![cached_summary("R_0805")],
        cached_footprints: vec![cached_summary("SOIC-8")],
        cached_sims: vec![cached_summary("BC547.lib")],
        display: LibraryDisplaySettings::default(),
    });
    let saved = dir.join("symbols").join("just-saved.snxsym");
    (app, saved)
}

#[test]
fn a_post_save_refresh_that_fails_keeps_the_pickers_entries() {
    let (mut app, saved) = app_with_cached_library();

    app.refresh_primitive_cache_for(&saved);

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
    let marker = format!("FailingLib-{}", Uuid::new_v4().simple());
    app.library
        .open_libraries
        .first_mut()
        .expect("library still open")
        .display_name = marker.clone();

    app.refresh_primitive_cache_for(&saved);

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
