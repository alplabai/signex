//! What the board store means (#2449): pinned while the engine still lived
//! in its tab, and held by `DocumentState::boards` since. The `boards`
//! checks pin that no board is left behind in the path-keyed map.

use std::path::{Path, PathBuf};

use super::document_files::save_async_tests::empty_board;
use crate::app::Signex;
use crate::app::handlers::menu::export::tests::app_workspace;

fn app_with_board(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), empty_board());
    (app, path)
}

/// Close the only tab of a board marked dirty, so its engine is parked.
fn park(app: &mut Signex, path: &Path) {
    app.document_state.mark_document_dirty(path);
    let _ = app.close_tab_parking_dirty(0);
    assert!(app.document_state.parked_pcb_boards.contains_key(path));
}

#[test]
fn a_layer_stack_tab_shares_the_boards_path_but_shows_no_board() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    app.open_layer_stack_tab();

    let docs = &app.document_state;
    let page = docs.active_tab;
    assert!(docs.tabs[page].kind.is_layer_stack());
    assert_eq!(
        docs.tabs[page].path, path,
        "the page shares the board's path"
    );
    assert!(app.active_pcb().is_none(), "the active page holds no board");
    assert!(!app.has_active_pcb());
    assert!(docs.tab_board_engine(page).is_none());
    assert!(
        docs.board_engine(&path).is_some(),
        "the path still finds the board"
    );
}

#[test]
fn a_parked_board_is_read_but_never_handed_out_for_a_write() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    park(&mut app, &path);

    assert!(app.document_state.board_engine(&path).is_some());
    assert!(app.document_state.pcb_board(&path).is_some());
    assert!(
        app.document_state.board_engine_mut(&path).is_none(),
        "a parked engine is read-only"
    );
}

#[test]
fn a_tab_whose_board_a_restore_dropped_does_not_fall_back_to_a_parked_one() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let docs = &mut app.document_state;
    let other = docs.new_board_engine(&path, empty_board());
    docs.park_pcb_board(path.clone(), other);
    docs.drop_pcb_board(0);

    assert!(docs.tab_board_engine(0).is_none());
    assert!(docs.board_engine(&path).is_none(), "the tab's answer wins");
    assert!(docs.pcb_board(&path).is_none());
    assert!(docs.board_engine_mut(&path).is_none());
    assert!(app.active_pcb().is_none());
}

#[test]
fn a_clean_close_drops_the_board_with_its_tab() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    assert!(!app.document_state.is_document_dirty(&path));
    let _ = app.close_tab_parking_dirty(0);

    let docs = &app.document_state;
    assert!(docs.tabs.is_empty());
    assert!(
        docs.parked_pcb_boards.is_empty(),
        "a clean board is not parked"
    );
    assert!(!docs.holds_pcb(&path));
    assert!(docs.board_engine(&path).is_none());
}

#[test]
fn a_closed_board_leaves_nothing_in_the_store() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let _ = app.close_tab_parking_dirty(0);
    assert!(app.document_state.boards.is_empty(), "a clean close");

    app.open_pcb_tab(path.clone(), "board".into(), empty_board());
    park(&mut app, &path);
    assert!(app.document_state.boards.is_empty(), "a parking close");

    let _ = app.document_state.take_parked_pcb_board(&path);
    app.open_pcb_tab(path.clone(), "board".into(), empty_board());
    app.document_state.mark_document_dirty(&path);
    let _ = app.close_tab_now(0);
    assert!(app.document_state.boards.is_empty(), "a discarding close");
    assert!(app.document_state.parked_pcb_boards.is_empty());
}

#[test]
fn closing_a_boards_layer_stack_tab_keeps_the_board() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    app.open_layer_stack_tab();
    let page = app.document_state.active_tab;
    let _ = app.close_tab_parking_dirty(page);

    assert_eq!(app.document_state.tabs.len(), 1);
    assert!(app.document_state.board_engine_mut(&path).is_some());
    assert!(app.document_state.tab_board_engine(0).is_some());
}

#[test]
fn a_rename_moves_an_open_board_to_its_new_path() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let renamed = dir.path().join("renamed.snxpcb");
    app.rekey_document(&path, &renamed);

    let docs = &mut app.document_state;
    assert!(
        docs.board_engine_mut(&renamed).is_some(),
        "the tab's board moved"
    );
    assert!(
        docs.board_engine(&path).is_none(),
        "nothing is left at the old path"
    );
    assert!(!docs.boards.contains_key(&path));
    assert!(!docs.holds_pcb(&path));
}

/// The store never overwrites a board stored at a path: a second push keeps
/// the stored board, and a rekey onto a stored board leaves both keys as they
/// were. This pins the store only; the callers' pre-check is #2971.
#[test]
fn an_open_board_is_never_replaced_at_its_path() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let uuid_at = |app: &Signex, path: &Path| app.document_state.boards[path].board().uuid;
    let first = uuid_at(&app, &path);

    let other = empty_board();
    assert_ne!(other.uuid, first);
    app.document_state.push_pcb_tab(
        "again".into(),
        path.clone(),
        None,
        signex_engine::pcb::BoardEngine::new(other, 1),
    );
    assert_eq!(uuid_at(&app, &path), first, "the open board was kept");

    let second = dir.path().join("second.snxpcb");
    app.open_pcb_tab(second.clone(), "second".into(), empty_board());
    let moved = uuid_at(&app, &second);
    app.document_state.rekey_open_pcb_board(&second, &path);
    assert_eq!(uuid_at(&app, &second), moved, "the moved board stayed");
    assert_eq!(
        uuid_at(&app, &path),
        first,
        "the board at the target is untouched"
    );
}

#[test]
fn a_rename_moves_a_parked_board_to_its_new_path() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    park(&mut app, &path);
    let renamed = dir.path().join("renamed.snxpcb");
    app.rekey_document(&path, &renamed);

    let docs = &app.document_state;
    assert!(docs.parked_pcb_boards.contains_key(&renamed));
    assert!(docs.board_engine(&renamed).is_some());
    assert!(
        docs.board_engine(&path).is_none(),
        "nothing is left at the old path"
    );
    assert!(!docs.holds_pcb(&path));
}
