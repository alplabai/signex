//! #1668 / #1671: the cursor and redraw commands run from an undocked window
//! act on the sheet and canvas that window shows.

use super::*;

fn run_in_window(f: &mut Fixture, id: &str) {
    let command = crate::keymap::AppCommandId::new(id).unwrap();
    let _ = f.app.dispatch_command(
        &command,
        crate::app::command::CommandArgs::none(),
        Some(f.window),
    );
}

#[test]
fn select_node_from_a_window_selects_the_item_under_its_cursor() {
    let mut f = fixture();
    let _ = f.app.update(Message::CanvasEventInWindow {
        window_id: f.window,
        event: crate::canvas::CanvasEvent::CursorAt {
            x: 15.24,
            y: 10.16,
            zoom_pct: 100.0,
        },
    });

    run_in_window(&mut f, "select_node_or_connection_item");

    assert_eq!(
        f.app.interaction_state.canvases[&f.window].selected,
        vec![SelectedItem::new(wire_id(), SelectedKind::Wire)]
    );
    assert!(f.app.interaction_state.canvas.selected.is_empty());
}

#[test]
fn refresh_view_from_a_window_clears_that_windows_canvas_caches() {
    let mut f = fixture();
    let window_before = f.app.interaction_state.canvases[&f.window].content_cache_clears;
    let main_before = f.app.interaction_state.canvas.content_cache_clears;

    run_in_window(&mut f, "refresh_view");

    assert!(f.app.interaction_state.canvases[&f.window].content_cache_clears > window_before);
    assert_eq!(
        f.app.interaction_state.canvas.content_cache_clears, main_before,
        "the main window's canvas was not asked to redraw"
    );
}
