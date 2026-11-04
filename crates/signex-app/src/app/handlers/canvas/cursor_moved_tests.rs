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

#[test]
fn a_draft_grid_style_and_its_discard_reach_the_symbol_preview_grid() {
    use crate::app::contracts::{Message, PreferencesMsg};
    use crate::preferences::PrefMsg;
    use crate::render_config::GridStyle;
    let (mut app, _boot) = Signex::new();
    let committed = app.sheet_grid().style;
    let draft = if committed == GridStyle::Lines {
        GridStyle::Dots
    } else {
        GridStyle::Lines
    };
    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(
        PrefMsg::DraftGridStyle(draft),
    )));
    assert_eq!(
        app.sheet_grid().style,
        draft,
        "the symbol preview keeps the old grid style while Preferences previews a new one"
    );
    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(
        PrefMsg::DiscardAndClose,
    )));
    assert_eq!(
        app.sheet_grid().style,
        committed,
        "the symbol preview keeps the discarded grid style"
    );
}

#[test]
fn the_symbol_preview_grid_follows_the_grid_toggle_and_size() {
    let (mut app, _boot) = Signex::new();
    app.ui_state.grid_visible = false;
    app.ui_state.visible_grid_mm = 5.08;
    let grid = app.sheet_grid();
    assert!(!grid.visible);
    assert_eq!(grid.pitch_mm, 5.08);
    app.ui_state.grid_visible = true;
    assert!(app.sheet_grid().visible);
}
