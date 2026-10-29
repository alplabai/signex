//! Generation guard of the off-thread library refresh (#1374).

use super::*;
use signex_library::RowId;

use crate::library::refresh::RefreshFollowUp;
use crate::library::test_adapters::{ScriptedSymbolsAdapter, cached_summary};

/// A mounted, open library whose symbol listing the test controls.
fn mounted() -> (
    LibraryState,
    PathBuf,
    std::sync::Arc<std::sync::Mutex<Vec<PrimitiveSummary>>>,
) {
    let root = PathBuf::from("/tmp/refresh-generation.snxlib");
    let mut state = LibraryState::default();
    let symbols = open_scripted(&mut state, &root);
    (state, root, symbols)
}

/// Mount a scripted adapter and open its library at `root`.
fn open_scripted(
    state: &mut LibraryState,
    root: &Path,
) -> std::sync::Arc<std::sync::Mutex<Vec<PrimitiveSummary>>> {
    let library_id = Uuid::now_v7();
    let (adapter, symbols) = ScriptedSymbolsAdapter::new(library_id);
    state.set.mount(Box::new(adapter)).expect("mount stub");
    state.open_libraries.push(OpenLibrary {
        root: root.to_path_buf(),
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
    symbols
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
    let start = state
        .begin_refresh(&root, RefreshFollowUp::Nothing)
        .expect("begin");
    let snapshot = read(&start, &root);

    let applied = state.finish_refresh(&root, start.generation, snapshot);

    assert!(matches!(applied.applied, Some(Ok(()))));
    assert_eq!(cached_symbol_names(&state, &root), ["R_0805"]);
}

#[test]
fn a_result_superseded_by_a_newer_request_is_ignored() {
    let (mut state, root, symbols) = mounted();
    set_symbols(&symbols, &["old"]);
    let first = state
        .begin_refresh(&root, RefreshFollowUp::Nothing)
        .expect("begin first");
    let first_snapshot = read(&first, &root);
    set_symbols(&symbols, &["new"]);
    let second = state
        .begin_refresh(&root, RefreshFollowUp::Nothing)
        .expect("begin second");
    let second_snapshot = read(&second, &root);

    // The newer read lands first, the older one afterwards.
    assert!(
        state
            .finish_refresh(&root, second.generation, second_snapshot)
            .applied
            .is_some()
    );
    let late = state.finish_refresh(&root, first.generation, first_snapshot);

    assert!(late.applied.is_none(), "the older read must be dropped");
    assert_eq!(cached_symbol_names(&state, &root), ["new"]);
}

#[test]
fn a_synchronous_refresh_supersedes_an_in_flight_one() {
    let (mut state, root, symbols) = mounted();
    set_symbols(&symbols, &["old"]);
    let start = state
        .begin_refresh(&root, RefreshFollowUp::Nothing)
        .expect("begin");
    let snapshot = read(&start, &root);
    set_symbols(&symbols, &["new"]);

    state.refresh_components(&root).expect("sync refresh");
    let late = state.finish_refresh(&root, start.generation, snapshot);

    assert!(late.applied.is_none());
    assert_eq!(cached_symbol_names(&state, &root), ["new"]);
}

#[test]
fn a_result_for_a_closed_library_is_ignored() {
    let (mut state, root, symbols) = mounted();
    set_symbols(&symbols, &["R_0805"]);
    let start = state
        .begin_refresh(&root, RefreshFollowUp::Nothing)
        .expect("begin");
    let snapshot = read(&start, &root);

    state.close_library(&root);
    let late = state.finish_refresh(&root, start.generation, snapshot);

    assert!(late.applied.is_none() && late.follow_ups.is_empty());
    assert!(state.library_at(&root).is_none(), "not resurrected");
}

#[test]
fn a_refresh_of_an_unmounted_library_does_not_start() {
    let mut state = LibraryState::default();
    let result = state.begin_refresh(Path::new("/tmp/nowhere.snxlib"), RefreshFollowUp::Nothing);
    assert!(matches!(result, Err(LibraryError::NotFound(_))));
}

fn clean(row: u128) -> RefreshFollowUp {
    RefreshFollowUp::CleanEditors(vec![RowId::from_uuid(Uuid::from_u128(row))])
}

#[test]
fn a_superseded_request_hands_its_follow_up_to_the_one_that_lands() {
    let (mut state, root, _symbols) = mounted();
    let first = state.begin_refresh(&root, clean(1)).expect("first");
    let first_snapshot = read(&first, &root);
    let second = state.begin_refresh(&root, clean(2)).expect("second");
    let second_snapshot = read(&second, &root);

    let early = state.finish_refresh(&root, first.generation, first_snapshot);
    assert!(early.applied.is_none() && early.follow_ups.is_empty());
    assert!(state.refresh_in_flight(&root), "the newer read is pending");

    let landed = state.finish_refresh(&root, second.generation, second_snapshot);
    assert_eq!(landed.follow_ups, [clean(1), clean(2)]);
    assert!(!state.refresh_in_flight(&root));
}

#[test]
fn follow_ups_of_reads_a_sync_refresh_superseded_run_when_the_last_reports() {
    let (mut state, root, _symbols) = mounted();
    let start = state.begin_refresh(&root, clean(1)).expect("begin");
    let snapshot = read(&start, &root);

    state.refresh_components(&root).expect("sync refresh");
    let landed = state.finish_refresh(&root, start.generation, snapshot);

    assert!(landed.applied.is_none(), "the stale read is not applied");
    assert_eq!(landed.follow_ups, [clean(1)], "but the caches are fresh");
    assert!(!state.refresh_in_flight(&root));
}

#[test]
fn closing_the_library_forgets_pending_follow_ups() {
    let (mut state, root, _symbols) = mounted();
    state.begin_refresh(&root, clean(1)).expect("begin");

    state.close_library(&root);

    assert!(!state.refresh_in_flight(&root));
    assert!(state.pending_follow_ups.is_empty());
}

#[test]
fn a_late_read_of_a_closed_library_leaves_the_reopened_mount_alone() {
    let (mut state, root, _symbols) = mounted();
    let old = state.begin_refresh(&root, clean(1)).expect("old begin");
    let old_snapshot = read(&old, &root);
    state.close_library(&root);
    open_scripted(&mut state, &root);
    let fresh = state.begin_refresh(&root, clean(2)).expect("new begin");

    let late = state.finish_refresh(&root, old.generation, old_snapshot);

    assert!(late.applied.is_none() && late.follow_ups.is_empty());
    assert!(
        state.refresh_in_flight(&root),
        "the new mount's own read is still pending"
    );
    let snapshot = read(&fresh, &root);
    let landed = state.finish_refresh(&root, fresh.generation, snapshot);
    assert_eq!(landed.follow_ups, [clean(2)]);
    assert!(!state.refresh_in_flight(&root));
}
