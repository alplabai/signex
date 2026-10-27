//! Generation guard of the off-thread library refresh (#1374).

use super::*;

use crate::library::test_adapters::{ScriptedSymbolsAdapter, cached_summary};

/// A mounted, open library whose symbol listing the test controls.
fn mounted() -> (
    LibraryState,
    PathBuf,
    std::sync::Arc<std::sync::Mutex<Vec<PrimitiveSummary>>>,
) {
    let root = PathBuf::from("/tmp/refresh-generation.snxlib");
    let library_id = Uuid::new_v4();
    let (adapter, symbols) = ScriptedSymbolsAdapter::new(library_id);
    let mut state = LibraryState::default();
    state.set.mount(Box::new(adapter)).expect("mount stub");
    state.open_libraries.push(OpenLibrary {
        root: root.clone(),
        display_name: "Scripted".into(),
        library_id,
        tables: HashMap::new(),
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
    (state, root, symbols)
}

fn set_symbols(handle: &std::sync::Mutex<Vec<PrimitiveSummary>>, names: &[&str]) {
    *handle.lock().expect("symbols lock") = names.iter().map(|n| cached_summary(n)).collect();
}

fn cached_symbol_names(state: &LibraryState, root: &Path) -> Vec<String> {
    state
        .library_at(root)
        .expect("library open")
        .cached_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect()
}

/// Reads through the handle `begin_refresh` returned, as the task does.
fn read(start: &RefreshStart, root: &Path) -> Result<LibrarySnapshot, LibraryError> {
    LibrarySnapshot::read(start.adapter.as_ref(), root)
}

#[test]
fn the_latest_refresh_result_is_applied() {
    let (mut state, root, symbols) = mounted();
    set_symbols(&symbols, &["R_0805"]);
    let start = state.begin_refresh(&root).expect("begin");
    let snapshot = read(&start, &root);

    let applied = state.finish_refresh(&root, start.generation, snapshot);

    assert!(matches!(applied, Some(Ok(()))));
    assert_eq!(cached_symbol_names(&state, &root), ["R_0805"]);
}

#[test]
fn a_result_superseded_by_a_newer_request_is_ignored() {
    let (mut state, root, symbols) = mounted();
    set_symbols(&symbols, &["old"]);
    let first = state.begin_refresh(&root).expect("begin first");
    let first_snapshot = read(&first, &root);
    set_symbols(&symbols, &["new"]);
    let second = state.begin_refresh(&root).expect("begin second");
    let second_snapshot = read(&second, &root);

    // The newer read lands first, the older one afterwards.
    assert!(
        state
            .finish_refresh(&root, second.generation, second_snapshot)
            .is_some()
    );
    let late = state.finish_refresh(&root, first.generation, first_snapshot);

    assert!(late.is_none(), "the older read must be dropped");
    assert_eq!(cached_symbol_names(&state, &root), ["new"]);
}

#[test]
fn a_synchronous_refresh_supersedes_an_in_flight_one() {
    let (mut state, root, symbols) = mounted();
    set_symbols(&symbols, &["old"]);
    let start = state.begin_refresh(&root).expect("begin");
    let snapshot = read(&start, &root);
    set_symbols(&symbols, &["new"]);

    state.refresh_components(&root).expect("sync refresh");
    let late = state.finish_refresh(&root, start.generation, snapshot);

    assert!(late.is_none());
    assert_eq!(cached_symbol_names(&state, &root), ["new"]);
}

#[test]
fn a_result_for_a_closed_library_is_ignored() {
    let (mut state, root, symbols) = mounted();
    set_symbols(&symbols, &["R_0805"]);
    let start = state.begin_refresh(&root).expect("begin");
    let snapshot = read(&start, &root);

    state.close_library(&root);
    let late = state.finish_refresh(&root, start.generation, snapshot);

    assert!(late.is_none());
    assert!(state.library_at(&root).is_none(), "not resurrected");
}

#[test]
fn a_refresh_of_an_unmounted_library_does_not_start() {
    let mut state = LibraryState::default();
    let result = state.begin_refresh(Path::new("/tmp/nowhere.snxlib"));
    assert!(matches!(result, Err(LibraryError::NotFound(_))));
}
