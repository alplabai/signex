//! `view` must be a pure read of app state (#531 review).
//!
//! `LocalGitAdapter` reads now `stat` the `.snxlib` and, after an external
//! change, re-parse it under a write lock. Doing that from `view` would put
//! a syscall in every frame and a full parse in the frame that notices an
//! edit, so the library views read only the caches on [`OpenLibrary`].

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

use signex_types::theme::{ThemeId, theme_tokens};
use uuid::Uuid;

use super::state::{
    LibraryBrowserState, LibraryDisplaySettings, LibraryState, NewComponentState, OpenLibrary,
};
use super::test_adapters::CountingAdapter;

#[test]
fn library_views_trigger_no_adapter_call() {
    let library_id = Uuid::new_v4();
    let (adapter, calls) = CountingAdapter::new(library_id);
    let root = PathBuf::from("/tmp/purity/Purity.snxlib");

    let mut state = LibraryState::default();
    state.set.mount(Box::new(adapter)).expect("mount");
    state.open_libraries.push(OpenLibrary {
        root: root.clone(),
        display_name: "Purity".into(),
        library_id,
        tables: HashMap::from([("Resistors".to_string(), Vec::new())]),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        display: LibraryDisplaySettings::default(),
    });
    calls.store(0, Ordering::SeqCst);

    let tokens = theme_tokens(ThemeId::Signex);
    let browser = LibraryBrowserState::new(root.clone());
    let new_component = NewComponentState {
        library_idx: Some(0),
        ..NewComponentState::default()
    };

    // Several frames, as the runtime would draw them.
    for _ in 0..3 {
        drop(super::browser::view(&root, &state, &browser, &tokens));
        drop(super::new_component::view(
            &state,
            &new_component,
            &tokens,
            ThemeId::Signex,
            Vec::new(),
        ));
    }

    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "a library view reached the adapter; it must read OpenLibrary caches only"
    );
}
