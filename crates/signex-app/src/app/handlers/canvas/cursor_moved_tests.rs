//! #1993 - a cursor move redraws the overlay, never the sheet background.

use crate::app::Signex;
use crate::canvas::CanvasEvent;

#[test]
fn a_cursor_move_keeps_the_background_cache() {
    let (mut app, _boot) = Signex::new();
    let before = app.interaction_state.active_canvas().bg_cache_clears;
    let overlay_before = app.interaction_state.active_canvas().overlay_cache_clears;

    let _ = app.handle_canvas_interaction_event(CanvasEvent::CursorMoved);

    let canvas = app.interaction_state.active_canvas();
    assert_eq!(
        canvas.bg_cache_clears, before,
        "the paper and grid were re-tessellated for a cursor move"
    );
    assert!(
        canvas.overlay_cache_clears > overlay_before,
        "the overlay must still follow the cursor"
    );
}

#[test]
fn a_draft_grid_style_drops_the_background_cache() {
    use crate::app::contracts::{Message, PreferencesMsg};
    let (mut app, _boot) = Signex::new();
    let before = app.interaction_state.active_canvas().bg_cache_clears;
    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(
        crate::preferences::PrefMsg::DraftGridStyle(crate::render_config::GridStyle::Lines),
    )));
    assert!(
        app.interaction_state.active_canvas().bg_cache_clears > before,
        "the grid-style preview stays stale until the camera moves"
    );
}
