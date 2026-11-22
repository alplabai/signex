//! DECISIONS 2026-11-22 (#2840): a recorded board edit is refused once its
//! target changed, and nothing runs.

use std::path::{Path, PathBuf};

use signex_engine::pcb::{BoardCommand, BoardItem};
use signex_types::pcb::Segment;

use super::fixtures::*;
use crate::app::Signex;

const FIRST: u128 = 11;
const SECOND: u128 = 12;

fn new_track(id: u128, y: f64) -> Segment {
    serde_json::from_value(serde_json::json!({
        "uuid": uuid::Uuid::from_u128(id),
        "start": {"x": 0.0, "y": y},
        "end": {"x": 5.0, "y": y},
        "width": 0.25,
        "layer": "TopCopper",
    }))
    .expect("a track")
}

fn app_with_tracks(dir: &Path) -> (Signex, PathBuf) {
    let mut app =
        crate::app::handlers::menu::export::tests::app_workspace(&dir.to_string_lossy(), &[]);
    let path = dir.join("board.snxpcb");
    let mut board = board_with_two_parts();
    board.segments = vec![new_track(FIRST, 0.0), new_track(SECOND, 5.0)];
    app.open_pcb_tab(path.clone(), "board".into(), board);
    (app, path)
}

fn track(app: &Signex, path: &Path, id: u128) -> Segment {
    (app.document_state.pcb_board(path).expect("the board"))
        .segments
        .iter()
        .find(|s| s.uuid == uuid::Uuid::from_u128(id))
        .expect("the track")
        .clone()
}

fn replace(app: &mut Signex, path: &Path, tracks: Vec<Segment>) {
    let items = tracks.into_iter().map(BoardItem::Segment).collect();
    let command = BoardCommand::ReplaceItems { items };
    app.execute_board_commands(path, vec![command], None)
        .expect("the edit runs");
}

/// A Properties panel width edit of `ids`: one row.
fn widen(app: &mut Signex, path: &Path, ids: &[u128]) -> u64 {
    let tracks = ids
        .iter()
        .map(|id| Segment {
            width: 0.5,
            ..track(app, path, *id)
        })
        .collect();
    app.ui_state.command_replay.mark_board_properties_edit();
    replace(app, path, tracks);
    let row = newest(app);
    assert_eq!(row.command, "edit_object_properties");
    row.seq
}

fn move_track(app: &mut Signex, path: &Path, id: u128) -> Segment {
    let mut moved = track(app, path, id);
    moved.start.x += 2.0;
    moved.end.x += 2.0;
    replace(app, path, vec![moved.clone()]);
    moved
}

#[test]
fn a_track_width_replay_is_refused_after_a_later_move() {
    let dir = tempfile::tempdir().expect("a folder");
    let (mut app, path) = app_with_tracks(dir.path());
    let seq = widen(&mut app, &path, &[FIRST]);
    let moved = move_track(&mut app, &path, FIRST);

    replay(&mut app, seq);

    assert_eq!(track(&app, &path, FIRST), moved, "the move stays");
    assert!(failure(&newest(&app)).contains("changed after"));
}

#[test]
fn a_track_width_replays_after_its_edit_was_undone() {
    let dir = tempfile::tempdir().expect("a folder");
    let (mut app, path) = app_with_tracks(dir.path());
    let seq = widen(&mut app, &path, &[FIRST]);
    assert!(app.step_board_history(&path, true));
    assert_eq!(track(&app, &path, FIRST).width, 0.25);

    replay(&mut app, seq);

    assert_eq!(track(&app, &path, FIRST).width, 0.5);
}

#[test]
fn a_row_with_one_changed_target_changes_nothing() {
    let dir = tempfile::tempdir().expect("a folder");
    let (mut app, path) = app_with_tracks(dir.path());
    let seq = widen(&mut app, &path, &[FIRST, SECOND]);
    assert!(app.step_board_history(&path, true));
    let moved = move_track(&mut app, &path, SECOND);

    replay(&mut app, seq);

    assert_eq!(track(&app, &path, FIRST).width, 0.25, "the free one too");
    assert_eq!(track(&app, &path, SECOND), moved);
    assert!(failure(&newest(&app)).contains("changed after"));
}
