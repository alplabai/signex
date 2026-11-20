//! Block A tests 3 and 4: a grouped undo's non-active members. Test 3 pins
//! when a parked sheet goes back to disk (dropped) and when it is kept; test 4
//! characterises whether a window or tile pane showing a non-active member
//! repaints after a grouped undo and after Replace All (maintainer question
//! M3 of the slice 2 plan).

use std::path::Path;

use super::{
    after_replace, dirty, drawn_label, engine_label, fresh_cache, replace_in_project,
    with_saved_cached_child,
};
use crate::app::Signex;
use crate::app::state::WindowKind;
use crate::canvas::CanvasSlot;

/// Show the sheet at `path` in a new undocked window, drawn as its engine
/// holds it now. The window's id.
fn undock(app: &mut Signex, path: &Path) -> iced::window::Id {
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        WindowKind::UndockedTab {
            path: path.to_path_buf(),
            title: "child".to_string(),
        },
    );
    let mut slot = CanvasSlot::default();
    slot.set_render_cache(Some(fresh_cache(app, path)));
    app.interaction_state.canvases.insert(window, slot);
    window
}

/// Show the sheet at `path` in an unfocused tile pane, drawn as its engine
/// holds it now.
fn tile(app: &mut Signex, path: &Path) {
    let mut slot = CanvasSlot::default();
    slot.set_render_cache(Some(fresh_cache(app, path)));
    app.interaction_state
        .tile_panes
        .schematic
        .insert(path.to_path_buf(), slot);
}

/// Bring the window's and the pane's drawing level with the engine again.
fn redraw(app: &mut Signex, window: iced::window::Id, path: &Path) {
    let cache = fresh_cache(app, path);
    app.interaction_state
        .canvases
        .get_mut(&window)
        .unwrap()
        .set_render_cache(Some(cache.clone()));
    app.interaction_state
        .tile_panes
        .schematic
        .get_mut(path)
        .unwrap()
        .set_render_cache(Some(cache));
}

fn window_label(app: &Signex, window: iced::window::Id) -> String {
    drawn_label(&app.interaction_state.canvases[&window])
}

fn pane_label(app: &Signex, path: &Path) -> String {
    drawn_label(&app.interaction_state.tile_panes.schematic[path])
}

// ---- Test 3.

#[test]
fn a_grouped_undo_takes_a_parked_sheet_back_to_disk_clean_and_drops_its_engine() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = after_replace(&dir);
    app.handle_undo_requested();
    assert!(
        !app.document_state.engines.contains_key(&child),
        "the clean parked engine is dropped"
    );
    assert!(!dirty(&app, &child));
}

/// Kept, and still dirty today: the parked engine the replace loaded has no
/// save point at its loaded state, so back at that state it reads unsaved
/// and closing asks to save text equal to the file.
#[test]
fn a_grouped_undo_keeps_a_parked_sheet_an_undocked_window_shows_and_leaves_it_dirty_today() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = after_replace(&dir);
    undock(&mut app, &child);
    app.handle_undo_requested();
    assert!(
        app.document_state.engines.contains_key(&child),
        "a sheet a window shows keeps its engine"
    );
    assert_eq!(engine_label(&app, &child), "CHILD_NET", "it was undone");
    assert!(!app.document_state.engines[&child].is_at_save_point());
    assert!(
        dirty(&app, &child),
        "kept dirty although back at its loaded state"
    );
}

/// Today's "held open" is tabs and undocked windows only: a tile pane alone
/// does not keep a parked sheet's engine.
#[test]
fn a_grouped_undo_drops_a_parked_sheet_only_a_tile_pane_shows() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = after_replace(&dir);
    tile(&mut app, &child);
    app.handle_undo_requested();
    assert!(!app.document_state.engines.contains_key(&child));
    assert!(!dirty(&app, &child));
}

// ---- Test 4 (c): repaint of non-active members, for M3.

/// `top` active; `child` a saved cached engine shown by an undocked window
/// and a tile pane, both drawing its `CHILD_NET` label.
fn child_shown_twice(dir: &tempfile::TempDir) -> (Signex, iced::window::Id, std::path::PathBuf) {
    let (mut app, _top, child) = with_saved_cached_child(dir);
    let window = undock(&mut app, &child);
    tile(&mut app, &child);
    assert_eq!(window_label(&app, window), "CHILD_NET");
    assert_eq!(pane_label(&app, &child), "CHILD_NET");
    (app, window, child)
}

/// Replace All across `top` (active) and `child` (shown twice): one group.
fn replace_on_both(app: &mut Signex, child: &Path) {
    replace_in_project(app, "net", "X");
    assert_eq!(
        engine_label(app, child),
        "CHILD_X",
        "the child was replaced"
    );
}

#[test]
fn replace_all_leaves_a_window_and_a_tile_pane_on_another_sheet_stale_today() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, window, child) = child_shown_twice(&dir);
    replace_on_both(&mut app, &child);
    assert_eq!(
        window_label(&app, window),
        "CHILD_NET",
        "the undocked window still draws the old text"
    );
    assert_eq!(
        pane_label(&app, &child),
        "CHILD_NET",
        "the tile pane still draws the old text"
    );
}

#[test]
fn a_grouped_undo_leaves_a_window_and_a_tile_pane_on_another_sheet_stale_today() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, window, child) = child_shown_twice(&dir);
    replace_on_both(&mut app, &child);
    redraw(&mut app, window, &child);
    app.handle_undo_requested();
    assert_eq!(
        engine_label(&app, &child),
        "CHILD_NET",
        "the child was undone"
    );
    assert_eq!(
        window_label(&app, window),
        "CHILD_X",
        "the undocked window still draws the replaced text"
    );
    assert_eq!(
        pane_label(&app, &child),
        "CHILD_X",
        "the tile pane still draws the replaced text"
    );
}
