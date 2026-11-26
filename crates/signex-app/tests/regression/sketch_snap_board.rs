//! #2518, Graphite mockup row 179: a board sketch snaps to the board under it
//! (point only), and its Snap ▾ board rows turn that off without an undo
//! step or a dirty board sketch. Driven through `Signex::update`.

use signex_app::library::editor::footprint::snap::{SnapKind, snap_cursor};
use signex_app::library::editor::footprint::snap_board::BoardFeatureKind;
use signex_app::library::editor::footprint::snap_menu::SketchSnapMsg;
use signex_app::library::messages::FootprintEditorMsg;

use crate::board_sketch::{Fixture, edit, in_sketch, open_board, send};
use crate::menu_support::menu_row;

/// What the cursor at `raw` snaps to in the board's sketch editor.
fn snap_at(f: &Fixture, raw: (f64, f64)) -> SnapKind {
    let session = f
        .app
        .interaction_state
        .board_sketch
        .as_ref()
        .expect("sketch mode");
    snap_cursor(raw, None, &session.editor.state, None).kind
}

/// The undo depth and dirty flag of the board's sketch editor.
fn history(f: &Fixture) -> (usize, bool) {
    let session = f
        .app
        .interaction_state
        .board_sketch
        .as_ref()
        .expect("sketch mode");
    (session.editor.history.undo_len(), session.editor.is_dirty())
}

#[test]
fn a_board_sketch_snaps_to_a_track_end_until_snap_turns_it_off() {
    let mut f = open_board();
    send(&mut f, menu_row("board_sketch"));
    assert!(in_sketch(&f));
    // The fixture board's track runs from (5, 15) to (25, 15).
    let track_end = SnapKind::BoardFeature(BoardFeatureKind::TrackEnd);
    assert_eq!(snap_at(&f, (5.05, 15.0)), track_end);
    assert_eq!(snap_at(&f, (24.95, 15.05)), track_end);

    let before = history(&f);
    edit(
        &mut f,
        FootprintEditorMsg::SketchSnap(SketchSnapMsg::BoardFeatureKindToggled(
            BoardFeatureKind::TrackEnd,
        )),
    );
    assert_ne!(snap_at(&f, (5.05, 15.0)), track_end, "track ends off");
    assert_eq!(history(&f), before, "a snap switch is no edit");

    edit(
        &mut f,
        FootprintEditorMsg::SketchSnap(SketchSnapMsg::BoardFeatureKindToggled(
            BoardFeatureKind::TrackEnd,
        )),
    );
    assert_eq!(snap_at(&f, (5.05, 15.0)), track_end, "back on");
    edit(
        &mut f,
        FootprintEditorMsg::SketchSnap(SketchSnapMsg::BoardFeaturesToggled),
    );
    assert!(
        !matches!(snap_at(&f, (5.05, 15.0)), SnapKind::BoardFeature(_)),
        "the switch for all turns every kind off"
    );
    assert!(in_sketch(&f), "the board rows never leave the mode");
}
